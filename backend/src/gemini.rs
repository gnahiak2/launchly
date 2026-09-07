use crate::models::DeploymentPlan;
use reqwest::Client;
use serde::Deserialize;
use std::{fs, path::Path, time::Duration};
use tokio::time::timeout;

#[derive(Debug, Deserialize)]
struct GeminiResponse {
    candidates: Option<Vec<Candidate>>,
}

#[derive(Debug, Deserialize)]
struct Candidate {
    content: Option<Content>,
}

#[derive(Debug, Deserialize)]
struct Content {
    parts: Option<Vec<Part>>,
}

#[derive(Debug, Deserialize)]
struct Part {
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SuggestedPlan {
    framework: String,
    language: String,
    package_manager: String,
    build_command: String,
    start_command: String,
    port: u16,
    rationale: Vec<String>,
}

pub async fn infer_plan(
    repository_url: &str,
    workspace: &Path,
) -> Result<Option<DeploymentPlan>, String> {
    let api_key = match std::env::var("GEMINI_API_KEY") {
        Ok(value) if !value.trim().is_empty() => value,
        _ => return Ok(None),
    };

    let snapshot = build_snapshot(workspace)?;
    if snapshot.is_empty() {
        return Ok(None);
    }

    let prompt = format!(
        "You are Launchly's repository deployment detector. Analyze only this bounded repository snapshot. Do not invent dependencies or commands. Return JSON only with exactly these fields: framework (string), language (string), package_manager (string), build_command (string), start_command (string), port (integer 1-65535), rationale (array of 1-3 short strings). If evidence is insufficient, use framework='Unknown application', language='Unknown', package_manager='Unknown', build_command='Not detected', start_command='Not detected', port=8080. Never include secrets. Repository URL: {repository_url}\n\nSnapshot:\n{snapshot}"
    );

    let endpoint = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.0-flash:generateContent?key={api_key}"
    );
    let body = serde_json::json!({
        "contents": [{ "parts": [{ "text": prompt }] }],
        "generationConfig": { "temperature": 0.0, "responseMimeType": "application/json" }
    });

    let response = timeout(
        Duration::from_secs(20),
        Client::new().post(endpoint).json(&body).send(),
    )
    .await
    .map_err(|_| "Gemini detection timed out".to_owned())?
    .map_err(|error| format!("Gemini request failed: {error}"))?;

    if !response.status().is_success() {
        return Err(format!("Gemini returned HTTP {}", response.status()));
    }
    let payload: GeminiResponse = response
        .json()
        .await
        .map_err(|error| format!("invalid Gemini response: {error}"))?;
    let text = payload
        .candidates
        .and_then(|items| items.into_iter().next())
        .and_then(|candidate| candidate.content)
        .and_then(|content| content.parts)
        .and_then(|parts| parts.into_iter().find_map(|part| part.text))
        .ok_or_else(|| "Gemini returned no plan".to_owned())?;
    let suggested: SuggestedPlan = serde_json::from_str(strip_code_fence(&text))
        .map_err(|error| format!("Gemini returned invalid plan JSON: {error}"))?;
    validate_suggestion(&suggested)?;

    Ok(Some(DeploymentPlan {
        repository_url: repository_url.to_owned(),
        framework: suggested.framework,
        language: suggested.language,
        package_manager: suggested.package_manager,
        build_command: suggested.build_command,
        start_command: suggested.start_command,
        port: suggested.port,
        confidence: "medium (Gemini-assisted)".into(),
        rationale: suggested
            .rationale
            .into_iter()
            .chain(std::iter::once(
                "Plan inferred from a bounded repository snapshot by Gemini; review before deployment.".into(),
            ))
            .take(3)
            .collect(),
    }))
}

fn build_snapshot(workspace: &Path) -> Result<String, String> {
    let mut files = Vec::new();
    collect_files(workspace, workspace, &mut files)?;
    files.sort();
    let mut snapshot = String::new();
    for relative in files.into_iter().take(80) {
        let path = workspace.join(&relative);
        snapshot.push_str("FILE: ");
        snapshot.push_str(&relative);
        snapshot.push('\n');
        if is_sensitive_name(&relative) {
            snapshot.push_str("[content omitted: sensitive filename]\n\n");
            continue;
        }
        if let Ok(content) = fs::read_to_string(&path) {
            snapshot.push_str(&content.chars().take(4_000).collect::<String>());
            snapshot.push_str("\n\n");
        }
        if snapshot.len() > 80_000 {
            break;
        }
    }
    Ok(snapshot)
}

fn collect_files(root: &Path, current: &Path, result: &mut Vec<String>) -> Result<(), String> {
    let entries =
        fs::read_dir(current).map_err(|error| format!("failed to inspect repository: {error}"))?;
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if name == ".git"
            || name == "node_modules"
            || name == "target"
            || name == "dist"
            || name == "build"
        {
            continue;
        }
        if path.is_dir() {
            collect_files(root, &path, result)?;
        } else if path.is_file() {
            if let Ok(relative) = path.strip_prefix(root) {
                result.push(relative.to_string_lossy().replace('\\', "/"));
            }
        }
    }
    Ok(())
}

fn is_sensitive_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.contains(".env")
        || lower.contains("secret")
        || lower.contains("credential")
        || lower.contains("token")
        || lower.ends_with(".pem")
        || lower.ends_with(".key")
}

fn strip_code_fence(value: &str) -> &str {
    let trimmed = value.trim();
    trimmed
        .strip_prefix("```json")
        .and_then(|body| body.strip_suffix("```"))
        .or_else(|| {
            trimmed
                .strip_prefix("```")
                .and_then(|body| body.strip_suffix("```"))
        })
        .unwrap_or(trimmed)
        .trim()
}

fn validate_suggestion(plan: &SuggestedPlan) -> Result<(), String> {
    if plan.port == 0
        || plan.rationale.is_empty()
        || plan.rationale.len() > 10
        || plan.framework.len() > 120
        || plan.build_command.len() > 500
        || plan.start_command.len() > 500
    {
        return Err("Gemini plan failed safety validation".into());
    }
    Ok(())
}
