//! GitHub/GitLab REST API integration.
//!
//! Detects platform from remote URL and provides PR/Issue listing.

use reqwest::header::{ACCEPT, AUTHORIZATION, USER_AGENT};
use serde::{Deserialize, Serialize};

/// Detected hosting platform for a repository.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Platform {
    GitHub,
    GitLab,
    Unknown,
}

/// Parsed repository coordinates.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoCoords {
    pub platform: Platform,
    pub owner: String,
    pub repo: String,
    pub api_base: String,
}

/// A pull request / merge request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PullRequest {
    pub number: u64,
    pub title: String,
    pub state: String,
    pub author: String,
    pub head_branch: String,
    pub base_branch: String,
    pub created_at: String,
    pub updated_at: String,
    pub url: String,
    pub draft: bool,
    pub additions: u64,
    pub deletions: u64,
    pub comments: u64,
}

/// A GitHub/GitLab issue.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Issue {
    pub number: u64,
    pub title: String,
    pub state: String,
    pub author: String,
    pub labels: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
    pub url: String,
}

/// Parse owner/repo from a git remote URL.
///
/// Supports:
/// - `git@github.com:owner/repo.git`
/// - `git@github-personal:owner/repo.git`  (SSH host alias)
/// - `https://github.com/owner/repo.git`
/// - `https://gitlab.com/owner/repo.git`
pub fn parse_remote_url(url: &str) -> RepoCoords {
    let unknown = RepoCoords {
        platform: Platform::Unknown,
        owner: String::new(),
        repo: String::new(),
        api_base: String::new(),
    };

    // Detect platform from URL
    let (platform, api_base, path_part) = if url.contains("github.com") || url.starts_with("git@github") {
        let path = extract_path(url);
        (Platform::GitHub, "https://api.github.com".to_string(), path)
    } else if url.contains("gitlab.com") || url.starts_with("git@gitlab") {
        let path = extract_path(url);
        (Platform::GitLab, "https://gitlab.com/api/v4".to_string(), path)
    } else {
        return unknown;
    };

    // Parse owner/repo from path
    let path = path_part.trim_end_matches(".git");
    let parts: Vec<&str> = path.split('/').filter(|p| !p.is_empty()).collect();
    if parts.len() < 2 {
        return unknown;
    }

    RepoCoords {
        platform,
        owner: parts[0].to_string(),
        repo: parts[1].to_string(),
        api_base,
    }
}

/// Extract the path portion from a git URL.
fn extract_path(url: &str) -> String {
    if url.starts_with("git@") {
        // git@github.com:owner/repo.git or git@alias:owner/repo.git
        url.split(':')
            .nth(1)
            .unwrap_or("")
            .to_string()
    } else if url.starts_with("https://") || url.starts_with("http://") {
        // https://github.com/owner/repo.git
        let after_host = url
            .split("//")
            .nth(1)
            .unwrap_or("")
            .split('/')
            .skip(1) // skip hostname
            .collect::<Vec<&str>>()
            .join("/");
        after_host
    } else {
        String::new()
    }
}

fn build_client(token: &str) -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent("GitAxon/0.1.0")
        .default_headers({
            let mut h = reqwest::header::HeaderMap::new();
            if !token.is_empty() {
                h.insert(
                    AUTHORIZATION,
                    format!("Bearer {}", token).parse().unwrap(),
                );
            }
            h.insert(ACCEPT, "application/json".parse().unwrap());
            h.insert(USER_AGENT, "GitAxon/0.1.0".parse().unwrap());
            h
        })
        .build()
        .unwrap_or_default()
}

// ─── GitHub API ─────────────────────────────────────────────────────

/// Raw GitHub PR from the API.
#[derive(Deserialize)]
struct GhPr {
    number: u64,
    title: String,
    state: String,
    draft: Option<bool>,
    html_url: String,
    created_at: String,
    updated_at: String,
    additions: Option<u64>,
    deletions: Option<u64>,
    comments: Option<u64>,
    user: Option<GhUser>,
    head: Option<GhRef>,
    base: Option<GhRef>,
}

#[derive(Deserialize)]
struct GhUser {
    login: String,
}

#[derive(Deserialize)]
struct GhRef {
    #[serde(rename = "ref")]
    ref_name: String,
}

#[derive(Deserialize)]
struct GhIssue {
    number: u64,
    title: String,
    state: String,
    html_url: String,
    created_at: String,
    updated_at: String,
    user: Option<GhUser>,
    labels: Option<Vec<GhLabel>>,
    pull_request: Option<serde_json::Value>, // presence means it's a PR, not an issue
}

#[derive(Deserialize)]
struct GhLabel {
    name: String,
}

pub async fn list_github_prs(
    coords: &RepoCoords,
    token: &str,
) -> Result<Vec<PullRequest>, String> {
    let client = build_client(token);
    let url = format!(
        "{}/repos/{}/{}/pulls?state=open&per_page=30&sort=updated&direction=desc",
        coords.api_base, coords.owner, coords.repo
    );

    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("HTTP error: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("GitHub API error {}: {}", status, body));
    }

    let prs: Vec<GhPr> = resp
        .json()
        .await
        .map_err(|e| format!("JSON parse error: {}", e))?;

    Ok(prs
        .into_iter()
        .map(|pr| PullRequest {
            number: pr.number,
            title: pr.title,
            state: pr.state,
            author: pr.user.map(|u| u.login).unwrap_or_default(),
            head_branch: pr.head.map(|h| h.ref_name).unwrap_or_default(),
            base_branch: pr.base.map(|b| b.ref_name).unwrap_or_default(),
            created_at: pr.created_at,
            updated_at: pr.updated_at,
            url: pr.html_url,
            draft: pr.draft.unwrap_or(false),
            additions: pr.additions.unwrap_or(0),
            deletions: pr.deletions.unwrap_or(0),
            comments: pr.comments.unwrap_or(0),
        })
        .collect())
}

pub async fn list_github_issues(
    coords: &RepoCoords,
    token: &str,
) -> Result<Vec<Issue>, String> {
    let client = build_client(token);
    let url = format!(
        "{}/repos/{}/{}/issues?state=open&per_page=30&sort=updated&direction=desc",
        coords.api_base, coords.owner, coords.repo
    );

    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("HTTP error: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("GitHub API error {}: {}", status, body));
    }

    let raw_issues: Vec<GhIssue> = resp
        .json()
        .await
        .map_err(|e| format!("JSON parse error: {}", e))?;

    // Filter out pull requests (GitHub returns PRs in the issues endpoint)
    Ok(raw_issues
        .into_iter()
        .filter(|i| i.pull_request.is_none())
        .map(|i| Issue {
            number: i.number,
            title: i.title,
            state: i.state,
            author: i.user.map(|u| u.login).unwrap_or_default(),
            labels: i
                .labels
                .unwrap_or_default()
                .into_iter()
                .map(|l| l.name)
                .collect(),
            created_at: i.created_at,
            updated_at: i.updated_at,
            url: i.html_url,
        })
        .collect())
}

pub async fn create_github_pr(
    coords: &RepoCoords,
    token: &str,
    title: &str,
    body: &str,
    head: &str,
    base: &str,
) -> Result<PullRequest, String> {
    let client = build_client(token);
    let url = format!(
        "{}/repos/{}/{}/pulls",
        coords.api_base, coords.owner, coords.repo
    );

    let payload = serde_json::json!({
        "title": title,
        "body": body,
        "head": head,
        "base": base,
    });

    let resp = client
        .post(&url)
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("HTTP error: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("GitHub API error {}: {}", status, body));
    }

    let pr: GhPr = resp
        .json()
        .await
        .map_err(|e| format!("JSON parse error: {}", e))?;

    Ok(PullRequest {
        number: pr.number,
        title: pr.title,
        state: pr.state,
        author: pr.user.map(|u| u.login).unwrap_or_default(),
        head_branch: pr.head.map(|h| h.ref_name).unwrap_or_default(),
        base_branch: pr.base.map(|b| b.ref_name).unwrap_or_default(),
        created_at: pr.created_at,
        updated_at: pr.updated_at,
        url: pr.html_url,
        draft: pr.draft.unwrap_or(false),
        additions: pr.additions.unwrap_or(0),
        deletions: pr.deletions.unwrap_or(0),
        comments: pr.comments.unwrap_or(0),
    })
}

// ─── PR Review Comments ────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewComment {
    pub id: u64,
    pub path: String,
    pub line: Option<u32>,
    pub body: String,
    pub author: String,
    pub created_at: String,
    pub in_reply_to_id: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrFile {
    pub filename: String,
    pub status: String,
    pub additions: u64,
    pub deletions: u64,
    pub patch: Option<String>,
}

#[derive(Deserialize)]
struct GhReviewComment {
    id: u64,
    path: Option<String>,
    line: Option<u32>,
    body: String,
    user: Option<GhUser>,
    created_at: String,
    in_reply_to_id: Option<u64>,
}

#[derive(Deserialize)]
struct GhPrFile {
    filename: String,
    status: String,
    additions: u64,
    deletions: u64,
    patch: Option<String>,
}

pub async fn get_pr_comments(
    coords: &RepoCoords,
    token: &str,
    pr_number: u64,
) -> Result<Vec<ReviewComment>, String> {
    let client = build_client(token);
    let url = format!(
        "{}/repos/{}/{}/pulls/{}/comments?per_page=100",
        coords.api_base, coords.owner, coords.repo, pr_number
    );

    let resp = client.get(&url).send().await.map_err(|e| format!("HTTP error: {}", e))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("GitHub API error {}: {}", status, body));
    }

    let comments: Vec<GhReviewComment> = resp.json().await.map_err(|e| format!("JSON parse error: {}", e))?;
    Ok(comments
        .into_iter()
        .map(|c| ReviewComment {
            id: c.id,
            path: c.path.unwrap_or_default(),
            line: c.line,
            body: c.body,
            author: c.user.map(|u| u.login).unwrap_or_default(),
            created_at: c.created_at,
            in_reply_to_id: c.in_reply_to_id,
        })
        .collect())
}

pub async fn get_pr_files(
    coords: &RepoCoords,
    token: &str,
    pr_number: u64,
) -> Result<Vec<PrFile>, String> {
    let client = build_client(token);
    let url = format!(
        "{}/repos/{}/{}/pulls/{}/files?per_page=100",
        coords.api_base, coords.owner, coords.repo, pr_number
    );

    let resp = client.get(&url).send().await.map_err(|e| format!("HTTP error: {}", e))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("GitHub API error {}: {}", status, body));
    }

    let files: Vec<GhPrFile> = resp.json().await.map_err(|e| format!("JSON parse error: {}", e))?;
    Ok(files
        .into_iter()
        .map(|f| PrFile {
            filename: f.filename,
            status: f.status,
            additions: f.additions,
            deletions: f.deletions,
            patch: f.patch,
        })
        .collect())
}

pub async fn submit_pr_review(
    coords: &RepoCoords,
    token: &str,
    pr_number: u64,
    body: &str,
    event: &str, // "APPROVE", "REQUEST_CHANGES", "COMMENT"
) -> Result<String, String> {
    let client = build_client(token);
    let url = format!(
        "{}/repos/{}/{}/pulls/{}/reviews",
        coords.api_base, coords.owner, coords.repo, pr_number
    );

    let payload = serde_json::json!({
        "body": body,
        "event": event,
    });

    let resp = client.post(&url).json(&payload).send().await.map_err(|e| format!("HTTP error: {}", e))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("GitHub API error {}: {}", status, body));
    }

    Ok("Review submitted".to_string())
}
