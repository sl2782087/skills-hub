//! Download a GitHub directory via the Contents API, bypassing git clone entirely.
//! This is much faster than cloning large repos when only a subdirectory is needed.

use std::path::Path;

use anyhow::{Context, Result};
use reqwest::blocking::Client;
use serde::Deserialize;
use serde_json::Value;

use super::cancel_token::CancelToken;

const MAX_SYMLINK_DEPTH: u32 = 64;

#[derive(Debug, Deserialize)]
struct GithubContent {
    name: String,
    #[serde(rename = "type")]
    content_type: String,
    download_url: Option<String>,
    path: String,
    #[serde(default)]
    target: Option<String>,
}

/// Resolve a Git-relative symlink target against the parent directory of `symlink_api_path`
/// (GitHub API path of the symlink entry, e.g. `.claude/skills/foo/data`).
pub fn resolve_repo_relative_symlink(symlink_api_path: &str, target: &str) -> Result<String> {
    if target.starts_with('/') {
        anyhow::bail!("absolute symlink targets are not supported: {}", target);
    }
    let parent_dir = symlink_api_path
        .rsplit_once('/')
        .map(|(p, _)| p)
        .filter(|p| !p.is_empty())
        .unwrap_or("");
    let mut stack: Vec<&str> = parent_dir.split('/').filter(|s| !s.is_empty()).collect();
    for comp in target.split('/') {
        match comp {
            "" | "." => {}
            ".." => {
                stack
                    .pop()
                    .ok_or_else(|| anyhow::anyhow!("symlink escapes repository root"))?;
            }
            c => stack.push(c),
        }
    }
    if stack.is_empty() {
        anyhow::bail!("resolved symlink path is empty");
    }
    Ok(stack.join("/"))
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

    download_dir_recursive(&client, owner, repo, branch, path, dest, cancel, token, 0)
}

fn github_contents_url(owner: &str, repo: &str, path: &str, branch: &str) -> String {
    format!(
        "https://api.github.com/repos/{}/{}/contents/{}?ref={}",
        owner, repo, path, branch
    )
}

fn fetch_contents_response(
    client: &Client,
    owner: &str,
    repo: &str,
    branch: &str,
    path: &str,
    token: Option<&str>,
    context: &str,
) -> Result<Value> {
    let url = github_contents_url(owner, repo, path, branch);
    let mut req = client
        .get(&url)
        .header("User-Agent", "skillverse")
        .header("Accept", "application/vnd.github.v3+json");
    if let Some(t) = token {
        req = req.header("Authorization", format!("Bearer {}", t));
    }
    let resp = req
        .send()
        .with_context(|| format!("request GitHub contents: {}", url))?;
    let resp = check_github_response(resp, context)?;
    let v: Value = resp
        .json()
        .with_context(|| format!("parse GitHub contents JSON: {}", url))?;
    Ok(v)
}

enum ContentsBody {
    Directory(Vec<GithubContent>),
    Single(GithubContent),
}

fn parse_contents_body(v: Value, context: &str) -> Result<ContentsBody> {
    if v.is_array() {
        let items: Vec<GithubContent> = serde_json::from_value(v)
            .with_context(|| format!("parse directory listing {}", context))?;
        return Ok(ContentsBody::Directory(items));
    }
    if v.is_object() {
        let item: GithubContent =
            serde_json::from_value(v).with_context(|| format!("parse single item {}", context))?;
        return Ok(ContentsBody::Single(item));
    }
    anyhow::bail!("unexpected GitHub contents JSON for {}", context);
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
    symlink_depth: u32,
) -> Result<()> {
    if cancel.is_some_and(|c| c.is_cancelled()) {
        anyhow::bail!("CANCELLED|操作已被用户取消。");
    }

    let v = fetch_contents_response(client, owner, repo, branch, path, token, path)?;

    match parse_contents_body(v, path)? {
        ContentsBody::Directory(items) => {
            for item in items {
                if cancel.is_some_and(|c| c.is_cancelled()) {
                    anyhow::bail!("CANCELLED|操作已被用户取消。");
                }
                process_content_entry(
                    client,
                    owner,
                    repo,
                    branch,
                    &item,
                    dest,
                    cancel,
                    token,
                    symlink_depth,
                )?;
            }
        }
        ContentsBody::Single(item) => {
            if item.content_type == "file" {
                if let Some(parent) = dest.parent() {
                    std::fs::create_dir_all(parent)
                        .with_context(|| format!("create parent dir {:?}", parent))?;
                }
                download_file_bytes(client, &item, dest, token)?;
            } else if item.content_type == "symlink" {
                expand_github_symlink(
                    client,
                    owner,
                    repo,
                    branch,
                    &item,
                    dest,
                    cancel,
                    token,
                    symlink_depth,
                )?;
            } else {
                anyhow::bail!(
                    "expected a directory listing at {}, got single {:?}",
                    path,
                    item.content_type
                );
            }
        }
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn process_content_entry(
    client: &Client,
    owner: &str,
    repo: &str,
    branch: &str,
    item: &GithubContent,
    dest_dir: &Path,
    cancel: Option<&CancelToken>,
    token: Option<&str>,
    symlink_depth: u32,
) -> Result<()> {
    let local_path = dest_dir.join(&item.name);

    match item.content_type.as_str() {
        "file" => {
            if let Some(parent) = local_path.parent() {
                std::fs::create_dir_all(parent)
                    .with_context(|| format!("create parent dir {:?}", parent))?;
            }
            download_file_bytes(client, item, &local_path, token)?;
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
                symlink_depth,
            )?;
        }
        "symlink" => {
            expand_github_symlink(
                client,
                owner,
                repo,
                branch,
                item,
                &local_path,
                cancel,
                token,
                symlink_depth,
            )?;
        }
        _ => {}
    }
    Ok(())
}

fn download_file_bytes(
    client: &Client,
    item: &GithubContent,
    local_path: &Path,
    token: Option<&str>,
) -> Result<()> {
    let download_url = item
        .download_url
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("missing download_url for {}", item.path))?;
    let mut file_req = client.get(download_url).header("User-Agent", "skillverse");
    if let Some(t) = token {
        file_req = file_req.header("Authorization", format!("Bearer {}", t));
    }
    let file_resp = file_req
        .send()
        .with_context(|| format!("download file: {}", item.path))?;
    let file_resp = check_github_response(file_resp, &item.path)?;
    let bytes = file_resp
        .bytes()
        .with_context(|| format!("read file bytes: {}", item.path))?;
    std::fs::write(local_path, &bytes).with_context(|| format!("write file {:?}", local_path))?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn expand_github_symlink(
    client: &Client,
    owner: &str,
    repo: &str,
    branch: &str,
    item: &GithubContent,
    local_path: &Path,
    cancel: Option<&CancelToken>,
    token: Option<&str>,
    symlink_depth: u32,
) -> Result<()> {
    if symlink_depth >= MAX_SYMLINK_DEPTH {
        anyhow::bail!(
            "symlink expansion depth exceeded (max {}) at {}",
            MAX_SYMLINK_DEPTH,
            item.path
        );
    }
    let target = if let Some(t) = item.target.as_deref() {
        t.to_string()
    } else {
        // Directory listing payload may omit `target` for symlink items.
        // Query this entry directly to retrieve the concrete symlink target.
        let v =
            fetch_contents_response(client, owner, repo, branch, &item.path, token, &item.path)?;
        match parse_contents_body(v, &item.path)? {
            ContentsBody::Single(single) => single.target.ok_or_else(|| {
                anyhow::anyhow!("GitHub symlink missing target field: {}", item.path)
            })?,
            ContentsBody::Directory(_) => {
                anyhow::bail!("expected symlink object for {}, got directory", item.path);
            }
        }
    };
    let resolved = resolve_repo_relative_symlink(&item.path, &target)?;

    let v = fetch_contents_response(client, owner, repo, branch, &resolved, token, &resolved)?;
    match parse_contents_body(v, &resolved)? {
        ContentsBody::Directory(items) => {
            std::fs::create_dir_all(local_path)
                .with_context(|| format!("create dir {:?}", local_path))?;
            for child in items {
                if cancel.is_some_and(|c| c.is_cancelled()) {
                    anyhow::bail!("CANCELLED|操作已被用户取消。");
                }
                process_content_entry(
                    client,
                    owner,
                    repo,
                    branch,
                    &child,
                    local_path,
                    cancel,
                    token,
                    symlink_depth + 1,
                )?;
            }
        }
        ContentsBody::Single(child) => {
            if child.content_type == "file" {
                if let Some(parent) = local_path.parent() {
                    std::fs::create_dir_all(parent)
                        .with_context(|| format!("create parent dir {:?}", parent))?;
                }
                download_file_bytes(client, &child, local_path, token)?;
            } else if child.content_type == "symlink" {
                expand_github_symlink(
                    client,
                    owner,
                    repo,
                    branch,
                    &child,
                    local_path,
                    cancel,
                    token,
                    symlink_depth + 1,
                )?;
            } else {
                anyhow::bail!(
                    "unsupported symlink target type {:?} at {}",
                    child.content_type,
                    resolved
                );
            }
        }
    }
    Ok(())
}

/// Check a GitHub API response for rate-limit errors and surface a helpful message.
fn check_github_response(
    resp: reqwest::blocking::Response,
    context: &str,
) -> Result<reqwest::blocking::Response> {
    let status = resp.status();
    if status.is_success() {
        return Ok(resp);
    }
    if status.as_u16() == 403 {
        let reset_hint = resp
            .headers()
            .get("x-ratelimit-reset")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<i64>().ok())
            .map(|ts| {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs() as i64;
                let wait_mins = ((ts - now).max(0) + 59) / 60; // round up
                format!("RATE_LIMITED|{}", wait_mins)
            })
            .unwrap_or_else(|| "403 Forbidden".to_string());
        anyhow::bail!("{}", reset_hint);
    }
    // For other errors, use the standard error_for_status logic.
    Err(anyhow::anyhow!(
        "GitHub API error {} for: {}",
        status,
        context
    ))
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
    fn resolve_symlink_typical_monorepo() {
        let r = resolve_repo_relative_symlink(
            ".claude/skills/ui-ux-pro-max/data",
            "../../../src/ui-ux-pro-max/data",
        )
        .unwrap();
        assert_eq!(r, "src/ui-ux-pro-max/data");
    }

    #[test]
    fn resolve_symlink_same_dir() {
        let r = resolve_repo_relative_symlink("skills/foo/bar", "../baz").unwrap();
        assert_eq!(r, "skills/baz");
    }

    #[test]
    fn resolve_symlink_escape_fails() {
        assert!(resolve_repo_relative_symlink("a", "../../../..").is_err());
    }

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

    #[test]
    fn check_github_response_passes_success() {
        let mut server = mockito::Server::new();
        let _m = server
            .mock("GET", "/ok")
            .with_status(200)
            .with_body("ok")
            .create();
        let client = Client::new();
        let resp = client.get(format!("{}/ok", server.url())).send().unwrap();
        assert!(check_github_response(resp, "test").is_ok());
    }

    #[test]
    fn check_github_response_extracts_rate_limit_reset() {
        let mut server = mockito::Server::new();
        let reset_ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            + 600; // 10 minutes from now
        let _m = server
            .mock("GET", "/limited")
            .with_status(403)
            .with_header("x-ratelimit-reset", &reset_ts.to_string())
            .with_body("rate limited")
            .create();
        let client = Client::new();
        let resp = client
            .get(format!("{}/limited", server.url()))
            .send()
            .unwrap();
        let err = check_github_response(resp, "test").unwrap_err();
        let msg = format!("{:#}", err);
        assert!(msg.contains("RATE_LIMITED|"), "got: {}", msg);
        // Should contain a number of minutes (around 10)
        let mins: i64 = msg
            .strip_prefix("RATE_LIMITED|")
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        assert!((9..=11).contains(&mins), "expected ~10 mins, got {}", mins);
    }

    #[test]
    fn check_github_response_handles_403_without_reset_header() {
        let mut server = mockito::Server::new();
        let _m = server
            .mock("GET", "/forbidden")
            .with_status(403)
            .with_body("forbidden")
            .create();
        let client = Client::new();
        let resp = client
            .get(format!("{}/forbidden", server.url()))
            .send()
            .unwrap();
        let err = check_github_response(resp, "test").unwrap_err();
        let msg = format!("{:#}", err);
        assert!(msg.contains("403"), "got: {}", msg);
    }

    #[test]
    fn check_github_response_handles_other_errors() {
        let mut server = mockito::Server::new();
        let _m = server
            .mock("GET", "/notfound")
            .with_status(404)
            .with_body("not found")
            .create();
        let client = Client::new();
        let resp = client
            .get(format!("{}/notfound", server.url()))
            .send()
            .unwrap();
        let err = check_github_response(resp, "test").unwrap_err();
        let msg = format!("{:#}", err);
        assert!(msg.contains("404"), "got: {}", msg);
    }
}
