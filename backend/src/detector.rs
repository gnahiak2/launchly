use crate::models::DeploymentPlan;

pub fn create_plan(repository_url: &str) -> Result<DeploymentPlan, String> {
    let trimmed = repository_url.trim();
    let parsed = parse_repository_url(trimmed)?;
    let name = parsed
        .rsplit('/')
        .next()
        .unwrap_or_default()
        .trim_end_matches(".git")
        .to_ascii_lowercase();

    let plan = if has_any(&name, &["rust", "axum", "cargo"]) {
        DeploymentPlan {
            repository_url: trimmed.to_owned(),
            framework: "Rust application".into(),
            language: "Rust".into(),
            package_manager: "Cargo".into(),
            build_command: "cargo build --release".into(),
            start_command: "./target/release/app".into(),
            port: 8080,
            confidence: "medium".into(),
            rationale: vec![
                "Repository name suggests a Rust application.".into(),
                "A Cargo-based build was selected as the safe default.".into(),
            ],
        }
    } else if has_any(&name, &["python", "django", "flask", "fastapi"]) {
        DeploymentPlan {
            repository_url: trimmed.to_owned(),
            framework: "Python application".into(),
            language: "Python".into(),
            package_manager: "pip".into(),
            build_command: "pip install -r requirements.txt".into(),
            start_command: "python -m app".into(),
            port: 8000,
            confidence: "medium".into(),
            rationale: vec![
                "Repository name suggests a Python application.".into(),
                "A requirements.txt-based install was selected as the safe default.".into(),
            ],
        }
    } else if has_any(
        &name,
        &["node", "javascript", "typescript", "next", "react", "vite"],
    ) {
        DeploymentPlan {
            repository_url: trimmed.to_owned(),
            framework: "Node.js application".into(),
            language: "JavaScript/TypeScript".into(),
            package_manager: "npm".into(),
            build_command: "npm run build".into(),
            start_command: "npm start".into(),
            port: 3000,
            confidence: "medium".into(),
            rationale: vec![
                "Repository name suggests a Node.js application.".into(),
                "npm was selected as the broadly compatible package manager default.".into(),
            ],
        }
    } else {
        DeploymentPlan {
            repository_url: trimmed.to_owned(),
            framework: "Unknown application".into(),
            language: "Unknown".into(),
            package_manager: "Unknown".into(),
            build_command: "Not detected".into(),
            start_command: "Not detected".into(),
            port: 8080,
            confidence: "low".into(),
            rationale: vec![
                "No strong framework signal was available from the repository URL.".into(),
                "Inspect the repository or provide an override before building.".into(),
            ],
        }
    };
    Ok(plan)
}

pub fn parse_repository_url(value: &str) -> Result<&str, String> {
    if value.is_empty() || value.len() > 2048 {
        return Err("repository_url must be between 1 and 2048 characters".into());
    }
    let valid_scheme = value.starts_with("https://") || value.starts_with("http://");
    if !valid_scheme || value.contains([' ', '\n', '\r', '\t']) {
        return Err("repository_url must be a valid HTTP(S) Git URL".into());
    }
    let rest = value
        .split_once("://")
        .map(|(_, rest)| rest)
        .unwrap_or_default();
    if !rest.contains('/') || rest.starts_with('/') || rest.contains("..") {
        return Err("repository_url must include a host and repository path".into());
    }
    Ok(value)
}

fn has_any(value: &str, signals: &[&str]) -> bool {
    signals.iter().any(|signal| value.contains(signal))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_non_http_urls() {
        assert!(parse_repository_url("git@github.com:org/app.git").is_err());
    }
    #[test]
    fn detects_node_plan() {
        assert_eq!(
            create_plan("https://github.com/acme/node-dashboard.git")
                .unwrap()
                .port,
            3000
        );
    }
    #[test]
    fn returns_unknown_plan_without_guessing_commands() {
        assert_eq!(
            create_plan("https://github.com/acme/inventory.git")
                .unwrap()
                .confidence,
            "low"
        );
    }
}
