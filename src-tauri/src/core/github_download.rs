//! Download a GitHub directory via the Contents API, bypassing git clone entirely.
//! This is much faster than cloning large repos when only a subdirectory is needed.

use std::path::Path;

use anyhow::{Context, Result};
use reqwest::blocking::Client;
use serde::Deserialize;

use super::cancel_token::CancelToken;

#[derive(Debug, Deserialize)]
struct GithubContent {
    name: String,
    #[serde(rename = "type")]
    content_type: String,
    download_url: Option<String>,
    path: String,
}

/// Download a directory from a GitHub repo using the Contents API.
///
/// `owner`/`repo`: repository coordinates
/// `branch`: branch or ref (e.g. "main")
/// `path`: directory path within the repo (e.g. "skills/user/foo")
/// `dest`: local directory to write files into (will be created)
/// `cancel`: optional cancellation token
pub fn download_github_directory(
    owner: &str,
    repo: &str,
    branch: &str,
    path: &str,
    dest: &Path,
    cancel: Option<&CancelToken>,
    token: Option<&str>,
) -> Result<()> {
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .context("build HTTP client")?;

    std::fs::create_dir_all(dest).with_context(|| format!("create directory {:?}", dest))?;

    download_dir_recursive(&client, owner, repo, branch, path, dest, cancel, token)
}

#[allow(clippy::too_many_arguments)]
fn download_dir_recursive(
    client: &Client,
    owner: &str,
    repo: &str,
    branch: &str,
    path: &str,
    dest: &Path,
    cancel: Option<&CancelToken>,
    token: Option<&str>,
) -> Result<()> {
    if cancel.is_some_and(|c| c.is_cancelled()) {
        anyhow::bail!("CANCELLED|操作已被用户取消。");
    }

    let url = format!(
        "https://api.github.com/repos/{}/{}/contents/{}?ref={}",
        owner, repo, path, branch
    );

    let mut req = client
        .get(&url)
        .header("User-Agent", "skills-hub")
        .header("Accept", "application/vnd.github.v3+json");
    if let Some(t) = token {
        req = req.header("Authorization", format!("Bearer {}", t));
    }
    let resp = req
        .send()
        .with_context(|| format!("request GitHub contents: {}", url))?
        .error_for_status()
        .with_context(|| format!("GitHub API error for: {}", url))?;

    let items: Vec<GithubContent> = resp
        .json()
        .with_context(|| format!("parse GitHub contents response: {}", url))?;

    for item in items {
        if cancel.is_some_and(|c| c.is_cancelled()) {
            anyhow::bail!("CANCELLED|操作已被用户取消。");
        }

        let local_path = dest.join(&item.name);

        match item.content_type.as_str() {
            "file" => {
                if let Some(download_url) = &item.download_url {
                    if let Some(parent) = local_path.parent() {
                        std::fs::create_dir_all(parent)
                            .with_context(|| format!("create parent dir {:?}", parent))?;
                    }
                    let mut file_req = client.get(download_url).header("User-Agent", "skills-hub");
                    if let Some(t) = token {
                        file_req = file_req.header("Authorization", format!("Bearer {}", t));
                    }
                    let bytes = file_req
                        .send()
                        .with_context(|| format!("download file: {}", item.path))?
                        .error_for_status()
                        .with_context(|| format!("download file HTTP error: {}", item.path))?
                        .bytes()
                        .with_context(|| format!("read file bytes: {}", item.path))?;

                    std::fs::write(&local_path, &bytes)
                        .with_context(|| format!("write file {:?}", local_path))?;
                }
            }
            "dir" => {
                download_dir_recursive(
                    client,
                    owner,
                    repo,
                    branch,
                    &item.path,
                    &local_path,
                    cancel,
                    token,
                )?;
            }
            _ => {
                // Skip symlinks, submodules, etc.
            }
        }
    }

    Ok(())
}

/// Check if a GitHub URL with subpath can use the fast API download path.
/// Returns Some((owner, repo, branch, subpath)) if applicable.
pub fn parse_github_api_params(
    clone_url: &str,
    branch: Option<&str>,
    subpath: Option<&str>,
) -> Option<(String, String, String, String)> {
    // Only for GitHub URLs with a subpath
    let subpath = subpath?;
    if subpath.is_empty() {
        return None;
    }

    // Extract owner/repo from clone_url like https://github.com/owner/repo.git
    let url = clone_url.trim_end_matches('/').trim_end_matches(".git");
    let prefix = "https://github.com/";
    if !url.starts_with(prefix) {
        return None;
    }
    let rest = &url[prefix.len()..];
    let parts: Vec<&str> = rest.split('/').collect();
    if parts.len() < 2 {
        return None;
    }

    Some((
        parts[0].to_string(),
        parts[1].to_string(),
        branch.unwrap_or("main").to_string(),
        subpath.to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_github_api_params_extracts_correctly() {
        let result = parse_github_api_params(
            "https://github.com/openclaw/skills.git",
            Some("main"),
            Some("skills/user/foo"),
        );
        assert_eq!(
            result,
            Some((
                "openclaw".to_string(),
                "skills".to_string(),
                "main".to_string(),
                "skills/user/foo".to_string(),
            ))
        );
    }

    #[test]
    fn parse_github_api_params_returns_none_without_subpath() {
        let result =
            parse_github_api_params("https://github.com/openclaw/skills.git", Some("main"), None);
        assert_eq!(result, None);
    }

    #[test]
    fn parse_github_api_params_returns_none_for_non_github() {
        let result = parse_github_api_params(
            "https://gitlab.com/user/repo.git",
            Some("main"),
            Some("path"),
        );
        assert_eq!(result, None);
    }
}
