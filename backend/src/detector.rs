use crate::models::DeploymentPlan;
use std::path::Path;

pub fn detect_plan_from_workspace(
    repository_url: &str,
    workspace: &Path,
) -> Result<DeploymentPlan, String> {
    parse_repository_url(repository_url)?;

    if !workspace.join("index.html").is_file() {
        return Err("repository must contain index.html to deploy as a website".into());
    }

    Ok(DeploymentPlan {
        repository_url: repository_url.to_owned(),
        framework: "Vanilla HTML".into(),
        language: "HTML/CSS/JavaScript".into(),
        package_manager: "None".into(),
        build_command: "No build required".into(),
        start_command: "nginx -g 'daemon off;'".into(),
        port: 8080,
        confidence: "high".into(),
        rationale: vec![
            "Detected index.html in the repository root.".into(),
            "Launchly will serve the repository as a static website with unprivileged Nginx."
                .into(),
        ],
    })
}

pub fn parse_repository_url(value: &str) -> Result<&str, String> {
    if value.is_empty() || value.len() > 2048 {
        return Err("repository_url must be between 1 and 2048 characters".into());
    }
    let valid_scheme = value.starts_with("https://") || value.starts_with("http://");
    if !valid_scheme || value.contains([' ', '\n', '\r', '\t', '?', '#']) {
        return Err("repository_url must be a valid HTTP(S) Git URL".into());
    }
    let rest = value
        .split_once("://")
        .map(|(_, rest)| rest)
        .unwrap_or_default();
    let (host, path) = rest.split_once('/').unwrap_or((rest, ""));
    if host.is_empty()
        || host.contains('@')
        || host.eq_ignore_ascii_case("localhost")
        || host == "127.0.0.1"
        || host == "[::1]"
        || path.is_empty()
        || path.contains("..")
    {
        return Err("repository_url must include a public host and repository path".into());
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn detects_static_website() {
        let workspace = temp_workspace();
        fs::write(workspace.join("index.html"), "<!doctype html>").unwrap();
        let plan =
            detect_plan_from_workspace("https://github.com/acme/site.git", &workspace).unwrap();
        assert_eq!(plan.framework, "Vanilla HTML");
        assert_eq!(plan.port, 8080);
        let _ = fs::remove_dir_all(workspace);
    }

    #[test]
    fn rejects_repositories_without_index_html() {
        let workspace = temp_workspace();
        let error =
            detect_plan_from_workspace("https://github.com/acme/app.git", &workspace).unwrap_err();
        assert!(error.contains("index.html"));
        let _ = fs::remove_dir_all(workspace);
    }

    #[test]
    fn rejects_credentials_and_local_hosts() {
        assert!(parse_repository_url("https://user:pass@github.com/acme/site.git").is_err());
        assert!(parse_repository_url("http://localhost/acme/site.git").is_err());
    }

    fn temp_workspace() -> std::path::PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let workspace = std::env::temp_dir().join(format!("launchly-static-test-{unique}"));
        fs::create_dir_all(&workspace).unwrap();
        workspace
    }
}
