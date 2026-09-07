use crate::models::DeploymentPlan;
use std::{fs, path::Path};

#[allow(dead_code)]
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

pub fn detect_plan_from_workspace(
    repository_url: &str,
    workspace: &Path,
) -> Result<DeploymentPlan, String> {
    let repository_url = parse_repository_url(repository_url)?.to_owned();
    if has_file(workspace, "Cargo.toml") {
        return Ok(DeploymentPlan {
            repository_url,
            framework: "Rust application".into(),
            language: "Rust".into(),
            package_manager: "Cargo".into(),
            build_command: "cargo build --release".into(),
            start_command: "./target/release/app".into(),
            port: 8080,
            confidence: "high".into(),
            rationale: vec![
                "Detected Cargo.toml in the repository root.".into(),
                "Rust projects are built with Cargo and commonly listen on port 8080 by default."
                    .into(),
            ],
        });
    }

    if has_file(workspace, "package.json") {
        let package_manager = if has_file(workspace, "pnpm-lock.yaml") {
            "pnpm"
        } else if has_file(workspace, "yarn.lock") {
            "yarn"
        } else {
            "npm"
        };
        return Ok(DeploymentPlan {
            repository_url,
            framework: "Node.js application".into(),
            language: "JavaScript/TypeScript".into(),
            package_manager: package_manager.into(),
            build_command: format!("{package_manager} run build"),
            start_command: format!("{package_manager} start"),
            port: 3000,
            confidence: "high".into(),
            rationale: vec![
                "Detected package.json in the repository root.".into(),
                format!(
                    "{} was selected based on the available lockfile signals.",
                    package_manager
                ),
            ],
        });
    }

    if has_file(workspace, "pyproject.toml") || has_file(workspace, "requirements.txt") {
        let package_manager = if has_file(workspace, "poetry.lock") {
            "poetry"
        } else {
            "pip"
        };
        return Ok(DeploymentPlan {
            repository_url,
            framework: "Python application".into(),
            language: "Python".into(),
            package_manager: package_manager.into(),
            build_command: match package_manager {
                "poetry" => "poetry install".into(),
                _ => "pip install -r requirements.txt".into(),
            },
            start_command: "python -m app".into(),
            port: 8000,
            confidence: "high".into(),
            rationale: vec![
                "Detected Python packaging files in the repository root.".into(),
                "Python deployments are planned from explicit manifest signals rather than repo name guesses.".into(),
            ],
        });
    }

    if has_file(workspace, "index.html") {
        return Ok(DeploymentPlan {
            repository_url,
            framework: "Vanilla HTML".into(),
            language: "HTML/CSS/JavaScript".into(),
            package_manager: "None".into(),
            build_command: "No build required".into(),
            start_command: "nginx -g 'daemon off;'".into(),
            port: 8080,
            confidence: "high".into(),
            rationale: vec![
                "Detected index.html without a framework manifest.".into(),
                "This repository can be served as a static site without a package manager.".into(),
            ],
        });
    }

    if has_file(workspace, "Dockerfile") || has_file(workspace, "Containerfile") {
        let dockerfile_path = if has_file(workspace, "Dockerfile") {
            workspace.join("Dockerfile")
        } else {
            workspace.join("Containerfile")
        };
        let dockerfile = fs::read_to_string(dockerfile_path).unwrap_or_default();
        let lower = dockerfile.to_ascii_lowercase();
        let (framework, language, package_manager, build_command, start_command, port, rationale) =
            if lower.contains("from node") || lower.contains("npm run") {
                (
                    "Node.js application",
                    "JavaScript/TypeScript",
                    "npm",
                    "npm run build",
                    "npm start",
                    3000,
                    vec![
                        "Detected a Dockerfile that appears to target Node.js.".into(),
                        "Launchly prefers explicit repository signals over opaque heuristics."
                            .into(),
                    ],
                )
            } else if lower.contains("from python") || lower.contains("pip install") {
                (
                    "Python application",
                    "Python",
                    "pip",
                    "pip install -r requirements.txt",
                    "python -m app",
                    8000,
                    vec![
                        "Detected a Dockerfile that appears to target Python.".into(),
                        "Launchly prefers explicit repository signals over opaque heuristics."
                            .into(),
                    ],
                )
            } else if lower.contains("from rust") || lower.contains("cargo build") {
                (
                    "Rust application",
                    "Rust",
                    "Cargo",
                    "cargo build --release",
                    "./target/release/app",
                    8080,
                    vec![
                        "Detected a Dockerfile that appears to target Rust.".into(),
                        "Launchly prefers explicit repository signals over opaque heuristics."
                            .into(),
                    ],
                )
            } else {
                (
                    "Containerized application",
                    "Unknown",
                    "Dockerfile",
                    "docker build .",
                    "docker run <image>",
                    8080,
                    vec![
                        "Detected a Dockerfile without a more specific language signal.".into(),
                        "A generic container plan is the safest fallback here.".into(),
                    ],
                )
            };

        return Ok(DeploymentPlan {
            repository_url,
            framework: framework.into(),
            language: language.into(),
            package_manager: package_manager.into(),
            build_command: build_command.into(),
            start_command: start_command.into(),
            port,
            confidence: "medium".into(),
            rationale,
        });
    }

    Ok(DeploymentPlan {
        repository_url,
        framework: "Unknown application".into(),
        language: "Unknown".into(),
        package_manager: "Unknown".into(),
        build_command: "Not detected".into(),
        start_command: "Not detected".into(),
        port: 8080,
        confidence: "low".into(),
        rationale: vec![
            "No strong manifest or Dockerfile signal was found in the repository root.".into(),
            "Launchly needs an explicit repository signal before it can build and run safely."
                .into(),
        ],
    })
}

pub fn parse_repository_url(value: &str) -> Result<&str, String> {
    if value.is_empty() || value.len() > 2048 {
        return Err("repository_url must be between 1 and 2048 characters".into());
    }
    let valid_scheme = value.starts_with("https://") || value.starts_with("http://");
    if !valid_scheme || value.contains([' ', '\n', '\r', '\t']) || value.contains(['?', '#']) {
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

fn has_file(workspace: &Path, name: &str) -> bool {
    workspace.join(name).is_file()
}

#[allow(dead_code)]
fn has_any(value: &str, signals: &[&str]) -> bool {
    signals.iter().any(|signal| value.contains(signal))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

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
    fn detects_workspace_cargo_project() {
        let workspace = temp_workspace();
        fs::write(
            workspace.join("Cargo.toml"),
            "[package]\nname = 'app'\nversion = '0.1.0'\n",
        )
        .unwrap();
        let plan =
            detect_plan_from_workspace("https://github.com/acme/app.git", &workspace).unwrap();
        assert_eq!(plan.language, "Rust");
        assert_eq!(plan.confidence, "high");
        let _ = fs::remove_dir_all(&workspace);
    }

    #[test]
    fn detects_workspace_package_json() {
        let workspace = temp_workspace();
        fs::write(workspace.join("package.json"), "{}\n").unwrap();
        fs::write(workspace.join("pnpm-lock.yaml"), "lockfileVersion: 5.4\n").unwrap();
        let plan =
            detect_plan_from_workspace("https://github.com/acme/app.git", &workspace).unwrap();
        assert_eq!(plan.package_manager, "pnpm");
        assert_eq!(plan.port, 3000);
        let _ = fs::remove_dir_all(&workspace);
    }

    #[test]
    fn detects_vanilla_html_site() {
        let workspace = temp_workspace();
        fs::write(
            workspace.join("index.html"),
            "<!doctype html><h1>Hello</h1>",
        )
        .unwrap();
        let plan =
            detect_plan_from_workspace("https://github.com/acme/site.git", &workspace).unwrap();
        assert_eq!(plan.framework, "Vanilla HTML");
        assert_eq!(plan.package_manager, "None");
        assert_eq!(plan.confidence, "high");
        let _ = fs::remove_dir_all(&workspace);
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

    fn temp_workspace() -> std::path::PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let workspace = std::env::temp_dir().join(format!("launchly-test-{unique}"));
        fs::create_dir_all(&workspace).unwrap();
        workspace
    }
}
