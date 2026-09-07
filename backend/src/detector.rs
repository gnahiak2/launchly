use crate::models::DeploymentPlan;
use std::path::Path;

pub fn validate_repository_url(value: &str) -> Result<String, String> {
    let value = value.trim();
    if value.is_empty() || value.len() > 2048 || value.contains([' ', '\n', '\r', '\t', '?', '#']) {
        return Err("repository_url must be a clean HTTP(S) URL".into());
    }
    let rest = value
        .strip_prefix("https://")
        .or_else(|| value.strip_prefix("http://"))
        .ok_or("repository_url must start with http:// or https://")?;
    let (host, path) = rest
        .split_once('/')
        .ok_or("repository_url must include a repository path")?;
    if host.is_empty() || host.contains('@') || host.contains(':') || path.is_empty() {
        return Err("repository_url must include a public host and repository path".into());
    }
    let lower = host.to_ascii_lowercase();
    if lower == "localhost" || lower == "127.0.0.1" || lower == "[::1]" || lower.starts_with("127.")
    {
        return Err("local repository hosts are not allowed".into());
    }
    if path.split('/').any(|part| part == "..") {
        return Err("repository path cannot contain '..'".into());
    }
    Ok(value.to_owned())
}

pub fn detect_plan(repository_url: &str, root: &Path) -> Result<DeploymentPlan, String> {
    if !root.join("index.html").is_file() {
        return Err("repository must contain index.html in its root".into());
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
            "The repository will be served directly as a static website.".into(),
        ],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn accepts_static_site() {
        let root = temp_dir();
        fs::write(root.join("index.html"), "<h1>Hello</h1>").unwrap();
        assert_eq!(
            detect_plan("https://github.com/example/site.git", &root)
                .unwrap()
                .port,
            8080
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn requires_root_index() {
        let root = temp_dir();
        assert!(detect_plan("https://github.com/example/site.git", &root).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_unsafe_urls() {
        assert!(validate_repository_url("https://user:pass@github.com/a/b").is_err());
        assert!(validate_repository_url("http://localhost/a/b").is_err());
        assert!(validate_repository_url("https://github.com/a/../b").is_err());
    }

    fn temp_dir() -> std::path::PathBuf {
        let n = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("launchly-test-{n}"));
        fs::create_dir_all(&path).unwrap();
        path
    }
}
