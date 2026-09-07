use crate::{
    detector,
    models::{DeployRequest, DeploymentPlan, DeploymentRecord},
};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    process::Stdio,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    process::Command,
    sync::RwLock,
    time::{sleep, timeout},
};
use uuid::Uuid;

const CLONE_TIMEOUT: Duration = Duration::from_secs(5 * 60);
const COMMAND_TIMEOUT: Duration = Duration::from_secs(30);
const HEALTH_TIMEOUT: Duration = Duration::from_secs(45);
const IMAGE: &str = "docker.io/nginxinc/nginx-unprivileged:alpine";

#[derive(Clone, Default)]
pub struct DeploymentService {
    records: Arc<RwLock<HashMap<String, DeploymentRecord>>>,
}

pub async fn inspect_repository(url: &str) -> Result<DeploymentPlan, String> {
    let url = detector::validate_repository_url(url)?;
    let id = format!("preview-{}", Uuid::new_v4());
    let root = workspace_root(&id);
    clone_repository(&url, &root).await?;
    let result = detector::detect_plan(&url, &root);
    let _ = tokio::fs::remove_dir_all(root).await;
    result
}

impl DeploymentService {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn create(&self, request: DeployRequest) -> Result<DeploymentRecord, String> {
        let url = detector::validate_repository_url(&request.repository_url)?;
        let domain = request
            .domain
            .map(|d| d.trim().to_ascii_lowercase())
            .filter(|d| !d.is_empty());
        if let Some(domain) = &domain {
            validate_domain(domain)?;
        }
        let id = Uuid::new_v4().to_string();
        let now = timestamp();
        let record = DeploymentRecord {
            id: id.clone(),
            repository_url: url.clone(),
            domain,
            state: "queued".into(),
            phase: "queued".into(),
            message: "Deployment queued".into(),
            plan: None,
            container: None,
            host_port: None,
            created_at: now,
            updated_at: now,
        };
        self.records
            .write()
            .await
            .insert(id.clone(), record.clone());
        let service = self.clone();
        tokio::spawn(async move {
            if let Err(error) = service.run(id.clone(), url).await {
                service.fail(&id, error).await;
            }
        });
        Ok(record)
    }

    pub async fn get(&self, id: &str) -> Option<DeploymentRecord> {
        self.records.read().await.get(id).cloned()
    }

    async fn run(&self, id: String, url: String) -> Result<(), String> {
        let result = self.run_inner(&id, &url).await;
        if result.is_err() {
            self.cleanup(&id).await;
        }
        result
    }

    async fn run_inner(&self, id: &str, url: &str) -> Result<(), String> {
        let base = workspace_root(id);
        let site = base.join("site");
        let container = format!("launchly-site-{id}");
        let port = host_port(id);
        self.update(id, "discovering", "cloning repository").await;
        clone_repository(url, &site).await?;
        self.update(id, "planning", "checking for root index.html")
            .await;
        let plan = detector::detect_plan(url, &site)?;
        self.set_plan(id, plan).await;
        self.update_runtime(id, &container, port).await;
        self.update(id, "starting", "starting unprivileged Nginx")
            .await;
        let run = command(
            "podman",
            vec![
                "run",
                "--detach",
                "--pull=missing",
                "--name",
                &container,
                "--network",
                "bridge",
                "--read-only",
                "--tmpfs",
                "/tmp:rw,noexec,nosuid,size=16m",
                "--tmpfs",
                "/var/run:rw,noexec,nosuid,size=16m",
                "--tmpfs",
                "/var/cache/nginx:rw,noexec,nosuid,size=32m",
                "--cap-drop=ALL",
                "--security-opt=no-new-privileges",
                "--pids-limit=256",
                "--memory=512m",
                "--cpus=1",
                "--publish",
                &format!("127.0.0.1:{port}:8080"),
                "--volume",
                &format!("{}:/usr/share/nginx/html:ro,Z", site.display()),
                IMAGE,
            ],
            COMMAND_TIMEOUT,
        )
        .await?;
        if !run.success {
            return Err(format!("Podman run failed: {}", clean_output(&run.stderr)));
        }
        self.update(id, "verifying", "waiting for HTTP 200 from the website")
            .await;
        if let Err(error) = wait_for_health(port).await {
            let logs = command(
                "podman",
                vec!["logs", "--tail", "100", &container],
                COMMAND_TIMEOUT,
            )
            .await
            .map(|o| truncate(&o.stdout, 4096))
            .unwrap_or_default();
            let _ = command("podman", vec!["rm", "--force", &container], COMMAND_TIMEOUT).await;
            let detail = if logs.trim().is_empty() {
                String::new()
            } else {
                format!(" Container logs: {logs}")
            };
            return Err(format!("health verification failed: {error}.{detail}"));
        }
        self.update(id, "live", "website is live").await;
        Ok(())
    }

    async fn cleanup(&self, id: &str) {
        if let Some(record) = self.get(id).await {
            if let Some(container) = record.container {
                let _ = command("podman", vec!["rm", "--force", &container], COMMAND_TIMEOUT).await;
            }
        }
        let _ = tokio::fs::remove_dir_all(workspace_root(id)).await;
    }
    async fn fail(&self, id: &str, error: String) {
        self.update(id, "failed", &error).await;
    }
    async fn update(&self, id: &str, state: &str, message: &str) {
        if let Some(r) = self.records.write().await.get_mut(id) {
            r.state = state.into();
            r.phase = state.into();
            r.message = message.into();
            r.updated_at = timestamp();
        }
    }
    async fn set_plan(&self, id: &str, plan: DeploymentPlan) {
        if let Some(r) = self.records.write().await.get_mut(id) {
            r.plan = Some(plan);
            r.updated_at = timestamp();
        }
    }
    async fn update_runtime(&self, id: &str, container: &str, port: u16) {
        if let Some(r) = self.records.write().await.get_mut(id) {
            r.container = Some(container.into());
            r.host_port = Some(port);
            r.updated_at = timestamp();
        }
    }
}

struct Output {
    success: bool,
    stdout: String,
    stderr: String,
}
async fn command(program: &str, args: Vec<&str>, limit: Duration) -> Result<Output, String> {
    let child = Command::new(program)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("failed to start {program}: {e}"))?;
    let result = timeout(limit, child.wait_with_output())
        .await
        .map_err(|_| format!("{program} timed out"))?
        .map_err(|e| format!("{program} failed: {e}"))?;
    Ok(Output {
        success: result.status.success(),
        stdout: String::from_utf8_lossy(&result.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&result.stderr).into_owned(),
    })
}

async fn clone_repository(url: &str, destination: &Path) -> Result<(), String> {
    if let Some(parent) = destination.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("failed to create workspace: {e}"))?;
    }
    let output = command(
        "git",
        vec![
            "clone",
            "--depth",
            "1",
            "--",
            url,
            &destination.to_string_lossy(),
        ],
        CLONE_TIMEOUT,
    )
    .await?;
    if output.success {
        Ok(())
    } else {
        Err(format!(
            "git clone failed: {}",
            clean_output(&output.stderr)
        ))
    }
}

async fn wait_for_health(port: u16) -> Result<(), String> {
    let start = tokio::time::Instant::now();
    while start.elapsed() < HEALTH_TIMEOUT {
        if let Ok(Ok(mut stream)) = timeout(
            Duration::from_secs(2),
            TcpStream::connect(("127.0.0.1", port)),
        )
        .await
        {
            let _ = stream
                .write_all(b"GET / HTTP/1.0\r\nHost: localhost\r\nConnection: close\r\n\r\n")
                .await;
            let mut bytes = [0u8; 128];
            if let Ok(Ok(size)) = timeout(Duration::from_secs(2), stream.read(&mut bytes)).await {
                let response = String::from_utf8_lossy(&bytes[..size]);
                if response.starts_with("HTTP/")
                    && response.split_whitespace().nth(1) == Some("200")
                {
                    return Ok(());
                }
            }
        }
        sleep(Duration::from_millis(500)).await;
    }
    Err("timed out waiting for HTTP 200".into())
}

fn workspace_root(id: &str) -> PathBuf {
    std::env::var_os("LAUNCHLY_WORKDIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp/launchly/deployments"))
        .join(id)
}
fn host_port(id: &str) -> u16 {
    let value = id
        .as_bytes()
        .iter()
        .fold(0u16, |a, b| a.wrapping_add(*b as u16));
    20000 + value % 20000
}
fn timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
fn truncate(value: &str, max: usize) -> String {
    value.chars().take(max).collect()
}
fn clean_output(value: &str) -> String {
    let clean = value.trim();
    if clean.is_empty() {
        "no diagnostic output".into()
    } else {
        truncate(clean, 4096)
    }
}
fn validate_domain(value: &str) -> Result<(), String> {
    if value.len() > 253
        || value.contains(['/', ':', '@', ' '])
        || !value.contains('.')
        || value.split('.').any(|part| {
            part.is_empty() || !part.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        })
    {
        return Err("domain must be a simple hostname".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn port_is_in_safe_range() {
        let port = host_port("12345678-1234-1234-1234-123456789abc");
        assert!((20000..40000).contains(&port));
    }
}
