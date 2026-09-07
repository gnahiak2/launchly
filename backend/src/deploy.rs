use crate::{
    detector,
    models::{DeployRequest, DeploymentPlan, DeploymentRecord},
};
use std::{
    collections::HashMap,
    path::PathBuf,
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
const BUILD_TIMEOUT: Duration = Duration::from_secs(15 * 60);
const RUN_TIMEOUT: Duration = Duration::from_secs(30);
const HEALTH_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Clone, Default)]
pub struct DeploymentService {
    deployments: Arc<RwLock<HashMap<String, DeploymentRecord>>>,
}

pub async fn inspect_repository(repository_url: &str) -> Result<DeploymentPlan, String> {
    let repository_url = detector::parse_repository_url(repository_url.trim())?.to_owned();
    let preview_id = format!("preview-{}", Uuid::new_v4());
    let workspace = workspace_dir(&preview_id);
    tokio::fs::create_dir_all(&workspace)
        .await
        .map_err(|error| format!("failed to create inspection workspace: {error}"))?;

    let result = async {
        let clone = run_command(
            "git",
            vec![
                "clone".into(),
                "--depth".into(),
                "1".into(),
                repository_url.clone(),
                workspace.to_string_lossy().into_owned(),
            ],
            CLONE_TIMEOUT,
        )
        .await?;
        if !clone.success {
            return Err(format!("git clone failed: {}", clone.stderr));
        }

        let plan = detector::detect_plan_from_workspace(&repository_url, &workspace)?;
        Ok(plan)
    }
    .await;

    let _ = tokio::fs::remove_dir_all(&workspace).await;
    result
}

impl DeploymentService {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn create(&self, request: DeployRequest) -> Result<DeploymentRecord, String> {
        let repository_url =
            detector::parse_repository_url(request.repository_url.trim())?.to_owned();
        let domain = request
            .domain
            .map(|value| value.trim().to_ascii_lowercase())
            .filter(|value| !value.is_empty());
        if let Some(domain) = &domain {
            validate_domain(domain)?;
        }

        let id = Uuid::new_v4().to_string();
        let now = unix_timestamp();
        let record = DeploymentRecord {
            id: id.clone(),
            repository_url: repository_url.clone(),
            domain,
            state: "queued".into(),
            phase: "queued".into(),
            message: "Deployment queued".into(),
            plan: None,
            image: None,
            container: None,
            host_port: None,
            created_at: now,
            updated_at: now,
        };
        self.deployments
            .write()
            .await
            .insert(id.clone(), record.clone());
        let service = self.clone();
        tokio::spawn(async move {
            if let Err(error) = service.run(id.clone(), repository_url).await {
                service.fail(&id, error).await;
            }
        });
        Ok(record)
    }

    pub async fn get(&self, id: &str) -> Option<DeploymentRecord> {
        self.deployments.read().await.get(id).cloned()
    }

    async fn run(&self, id: String, repository_url: String) -> Result<(), String> {
        let result = self.run_inner(id.clone(), repository_url).await;
        if result.is_err() {
            self.cleanup(&id).await;
        }
        result
    }

    async fn run_inner(&self, id: String, repository_url: String) -> Result<(), String> {
        let workspace = workspace_dir(&id);
        let image = format!("localhost/launchly-deployment-{id}:latest");
        let container = format!("launchly-deployment-{id}");
        self.update(&id, "discovering", "cloning repository", None)
            .await;
        if workspace.exists() {
            tokio::fs::remove_dir_all(&workspace)
                .await
                .map_err(|e| format!("failed to clean workspace: {e}"))?;
        }
        tokio::fs::create_dir_all(&workspace)
            .await
            .map_err(|e| format!("failed to create workspace: {e}"))?;

        let clone = run_command(
            "git",
            vec![
                "clone".into(),
                "--depth".into(),
                "1".into(),
                repository_url.clone(),
                workspace.to_string_lossy().into_owned(),
            ],
            CLONE_TIMEOUT,
        )
        .await?;
        if !clone.success {
            return Err(format!("git clone failed: {}", clone.stderr));
        }

        self.update(
            &id,
            "planning",
            "detecting build plan from repository files",
            None,
        )
        .await;
        let plan = detector::detect_plan_from_workspace(&repository_url, &workspace)?;
        self.set_plan(&id, plan.clone()).await;

        {
            tokio::fs::write(
                workspace.join("Containerfile"),
                r#"FROM nginxinc/nginx-unprivileged:alpine
COPY --chown=101:101 . /usr/share/nginx/html
EXPOSE 8080
"#,
            )
            .await
            .map_err(|error| format!("failed to create static-site Containerfile: {error}"))?;
            tokio::fs::write(
                workspace.join(".containerignore"),
                r#".git
.env
.env.*
*.pem
*.key
*.secret
node_modules
target
dist
build
"#,
            )
            .await
            .map_err(|error| format!("failed to create static-site ignore file: {error}"))?;
        }

        self.update(
            &id,
            "building",
            "building application image with Podman",
            None,
        )
        .await;
        let build = run_command(
            "podman",
            vec![
                "build".into(),
                "--pull=missing".into(),
                "--tag".into(),
                image.clone(),
                workspace.to_string_lossy().into_owned(),
            ],
            BUILD_TIMEOUT,
        )
        .await?;
        if !build.success {
            return Err(format!("Podman build failed: {}", build.stderr));
        }

        let host_port = host_port_for(&id);
        self.update_runtime(&id, &image, &container, host_port)
            .await;
        self.update(
            &id,
            "starting",
            "starting isolated application container",
            None,
        )
        .await;
        let run = run_command(
            "podman",
            vec![
                "run".into(),
                "--detach".into(),
                "--name".into(),
                container.clone(),
                "--network".into(),
                "bridge".into(),
                "--read-only".into(),
                "--tmpfs".into(),
                "/var/run:rw,noexec,nosuid,size=16m".into(),
                "--tmpfs".into(),
                "/var/cache/nginx:rw,noexec,nosuid,size=32m".into(),
                "--tmpfs".into(),
                "/tmp:rw,noexec,nosuid,size=16m".into(),
                "--cap-drop=ALL".into(),
                "--security-opt=no-new-privileges".into(),
                "--pids-limit=256".into(),
                "--memory=512m".into(),
                "--cpus=1".into(),
                "--publish".into(),
                format!("127.0.0.1:{host_port}:{}", plan.port),
                image.clone(),
            ],
            RUN_TIMEOUT,
        )
        .await?;
        if !run.success {
            return Err(format!("Podman run failed: {}", run.stderr));
        }

        self.update(
            &id,
            "verifying",
            "waiting for application health check",
            None,
        )
        .await;
        if let Err(error) = wait_for_health(host_port).await {
            let logs = run_command(
                "podman",
                vec![
                    "logs".into(),
                    "--tail".into(),
                    "100".into(),
                    container.clone(),
                ],
                RUN_TIMEOUT,
            )
            .await
            .map(|output| truncate(&output.stdout, 8_192))
            .unwrap_or_default();
            let _ = run_command(
                "podman",
                vec!["rm".into(), "--force".into(), container.clone()],
                RUN_TIMEOUT,
            )
            .await;
            let diagnostic = if logs.trim().is_empty() {
                String::new()
            } else {
                format!(" Container logs: {logs}")
            };
            return Err(format!("health verification failed: {error}.{diagnostic}"));
        }

        self.update(&id, "live", "application is healthy and live", None)
            .await;
        Ok(())
    }

    async fn set_plan(&self, id: &str, plan: DeploymentPlan) {
        let mut deployments = self.deployments.write().await;
        if let Some(record) = deployments.get_mut(id) {
            record.plan = Some(plan);
            record.updated_at = unix_timestamp();
        }
    }

    async fn update_runtime(&self, id: &str, image: &str, container: &str, host_port: u16) {
        let mut deployments = self.deployments.write().await;
        if let Some(record) = deployments.get_mut(id) {
            record.image = Some(image.into());
            record.container = Some(container.into());
            record.host_port = Some(host_port);
            record.updated_at = unix_timestamp();
        }
    }

    async fn update(&self, id: &str, state: &str, message: &str, plan: Option<DeploymentPlan>) {
        let mut deployments = self.deployments.write().await;
        if let Some(record) = deployments.get_mut(id) {
            record.state = state.into();
            record.phase = state.into();
            record.message = message.into();
            if plan.is_some() {
                record.plan = plan;
            }
            record.updated_at = unix_timestamp();
        }
    }

    async fn cleanup(&self, id: &str) {
        let workspace = workspace_dir(id);
        let image = format!("localhost/launchly-deployment-{id}:latest");
        let container = format!("launchly-deployment-{id}");
        let _ = run_command(
            "podman",
            vec!["rm".into(), "--force".into(), container],
            RUN_TIMEOUT,
        )
        .await;
        let _ = run_command(
            "podman",
            vec!["rmi".into(), "--force".into(), image],
            RUN_TIMEOUT,
        )
        .await;
        let _ = tokio::fs::remove_dir_all(workspace).await;
    }

    async fn fail(&self, id: &str, message: String) {
        let mut deployments = self.deployments.write().await;
        if let Some(record) = deployments.get_mut(id) {
            record.state = "failed".into();
            record.phase = "deployment".into();
            record.message = message;
            record.updated_at = unix_timestamp();
        }
    }
}

struct CommandOutput {
    success: bool,
    stdout: String,
    stderr: String,
}

async fn run_command(
    program: &str,
    args: Vec<String>,
    duration: Duration,
) -> Result<CommandOutput, String> {
    let mut command = Command::new(program);
    command
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let output = timeout(duration, command.output())
        .await
        .map_err(|_| format!("{program} timed out after {duration:?}"))?
        .map_err(|e| format!("failed to run {program}: {e}"))?;
    Ok(CommandOutput {
        success: output.status.success(),
        stdout: truncate(&String::from_utf8_lossy(&output.stdout), 32_768),
        stderr: truncate(&String::from_utf8_lossy(&output.stderr), 32_768),
    })
}

async fn wait_for_health(port: u16) -> Result<(), String> {
    let deadline = tokio::time::Instant::now() + HEALTH_TIMEOUT;
    loop {
        if tokio::time::Instant::now() >= deadline {
            return Err("timed out waiting for HTTP response".into());
        }
        if let Ok(Ok(mut stream)) = timeout(
            Duration::from_secs(2),
            TcpStream::connect(("127.0.0.1", port)),
        )
        .await
        {
            let _ = stream
                .write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
                .await;
            let mut response = [0_u8; 128];
            if let Ok(Ok(size)) = timeout(Duration::from_secs(2), stream.read(&mut response)).await
            {
                let text = String::from_utf8_lossy(&response[..size]);
                if text.starts_with("HTTP/1.1 200") || text.starts_with("HTTP/1.0 200") {
                    return Ok(());
                }
            }
        }
        sleep(Duration::from_secs(1)).await;
    }
}

fn workspace_dir(id: &str) -> PathBuf {
    std::env::temp_dir()
        .join("launchly")
        .join("deployments")
        .join(id)
}
fn host_port_for(id: &str) -> u16 {
    10_000
        + (id
            .as_bytes()
            .iter()
            .fold(0_u16, |sum, byte| sum.wrapping_add(*byte as u16))
            % 2_000)
}
fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
fn truncate(value: &str, limit: usize) -> String {
    if value.len() <= limit {
        value.into()
    } else {
        format!("{}…", &value[..limit])
    }
}

fn validate_domain(domain: &str) -> Result<(), String> {
    if domain.len() > 253
        || domain.starts_with('.')
        || domain.ends_with('.')
        || domain.contains("..")
        || !domain
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
    {
        return Err("domain must be a valid hostname".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn creates_queued_deployment() {
        let service = DeploymentService::new();
        let record = service
            .create(DeployRequest {
                repository_url: "https://github.com/acme/app.git".into(),
                domain: None,
            })
            .await
            .unwrap();
        assert_eq!(record.state, "queued");
        assert!(service.get(&record.id).await.is_some());
    }
    #[test]
    fn validates_domains() {
        assert!(validate_domain("app.example.com").is_ok());
        assert!(validate_domain("bad/domain").is_err());
    }
}
