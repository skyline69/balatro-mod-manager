use chrono::DateTime;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

// GitHub API response structures
#[derive(Deserialize, Debug)]
struct GitHubCommit {
    sha: String,
    commit: GitHubCommitDetails,
    #[serde(default)]
    files: Vec<GitHubFile>,
}

#[derive(Deserialize, Debug)]
struct GitHubCommitDetails {
    author: GitHubAuthor,
}

#[derive(Deserialize, Debug)]
struct GitHubAuthor {
    date: String,
}

#[derive(Deserialize, Debug)]
struct GitHubFile {
    filename: String,
}

// Helper function to extract repo owner and name from URL
pub fn parse_github_url(url: &str) -> Option<(String, String)> {
    let url = url.trim_end_matches(".git");

    // Match patterns like:
    // https://github.com/owner/repo
    // git@github.com:owner/repo
    if url.contains("github.com") {
        let parts: Vec<&str> = if url.contains("github.com/") {
            url.split("github.com/").collect()
        } else if url.contains("github.com:") {
            url.split("github.com:").collect()
        } else {
            return None;
        };

        if parts.len() < 2 {
            return None;
        }

        let repo_parts: Vec<&str> = parts[1].split('/').collect();
        if repo_parts.len() < 2 {
            return None;
        }

        return Some((repo_parts[0].to_string(), repo_parts[1].to_string()));
    }

    None
}

// Clone a repository from GitHub
pub async fn clone_repository(url: &str, path: &str) -> Result<(), String> {
    let (owner, repo) =
        parse_github_url(url).ok_or_else(|| "Invalid GitHub URL format".to_string())?;

    // Download the repository as a zip file
    let download_url = format!(
        "https://github.com/{}/{}/archive/refs/heads/main.zip",
        owner, repo
    );
    let response = reqwest::get(&download_url)
        .await
        .map_err(|e| format!("Failed to download repository: {}", e))?;

    let bytes = response
        .bytes()
        .await
        .map_err(|e| format!("Failed to read response: {}", e))?;

    // Create target directory
    let target_path = PathBuf::from(path);
    std::fs::create_dir_all(&target_path)
        .map_err(|e| format!("Failed to create directory: {}", e))?;

    // Save zip file to temporary location
    let temp_zip = target_path.join("temp.zip");
    let mut file = File::create(&temp_zip).map_err(|e| format!("Failed to create file: {}", e))?;
    file.write_all(&bytes)
        .map_err(|e| format!("Failed to write file: {}", e))?;

    // Extract zip file
    let file =
        std::fs::File::open(&temp_zip).map_err(|e| format!("Failed to open zip file: {}", e))?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|e| format!("Failed to parse zip file: {}", e))?;

    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| format!("Failed to access file in zip: {}", e))?;

        // Get the path removing the repository name folder
        let name = file.name();
        let parts: Vec<&str> = name.split('/').collect();
        if parts.len() <= 1 {
            continue;
        }

        let rel_path = parts[1..].join("/");
        if rel_path.is_empty() {
            continue;
        }

        let target = target_path.join(&rel_path);

        // Create directories
        if file.is_dir() {
            std::fs::create_dir_all(&target)
                .map_err(|e| format!("Failed to create directory: {}", e))?;
            continue;
        }

        // Create parent directories for files
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create directory: {}", e))?;
        }

        // Extract files
        let mut outfile =
            File::create(&target).map_err(|e| format!("Failed to create file: {}", e))?;
        std::io::copy(&mut file, &mut outfile)
            .map_err(|e| format!("Failed to write file: {}", e))?;
    }

    // Clean up temp zip file
    std::fs::remove_file(temp_zip).ok();

    // Create a simple .git_info file to store repo URL (for pulls)
    let git_info = target_path.join(".git_info");
    std::fs::write(git_info, url).map_err(|e| format!("Failed to write repository info: {}", e))?;

    Ok(())
}

pub fn is_repository_directory(path: &str) -> bool {
    let repo_path = PathBuf::from(path);
    let git_info_path = repo_path.join(".git_info");
    git_info_path.exists()
}

// Get mod timestamps from the repository
pub async fn get_mod_timestamps(repo_path: &str) -> Result<HashMap<String, i64>, String> {
    let client = reqwest::Client::new();
    let repo_path = PathBuf::from(repo_path);

    // Read repo URL from .git_info file
    let git_info_path = repo_path.join(".git_info");
    let url = std::fs::read_to_string(git_info_path)
        .map_err(|e| format!("Failed to read repository info: {}", e))?;

    let (owner, repo) =
        parse_github_url(&url).ok_or_else(|| "Invalid GitHub URL format".to_string())?;

    // Get commits for the repository
    let commits_url = format!("https://api.github.com/repos/{}/{}/commits", owner, repo);

    let response = client
        .get(&commits_url)
        .header("User-Agent", "balatro-mod-manager")
        .send()
        .await
        .map_err(|e| format!("Failed to fetch commits: {}", e))?;

    if !response.status().is_success() {
        return Err(format!(
            "GitHub API error: {} - {}",
            response.status().as_u16(),
            response.text().await.unwrap_or_default()
        ));
    }

    let commits: Vec<GitHubCommit> = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse commits: {}", e))?;

    let mut timestamps = HashMap::new();

    for commit in commits {
        // Parse commit date to Unix timestamp
        let date = DateTime::parse_from_rfc3339(&commit.commit.author.date)
            .map_err(|e| format!("Failed to parse date: {}", e))?;
        let timestamp = date.timestamp();

        // Get the files included in this commit
        let commit_detail_url = format!(
            "https://api.github.com/repos/{}/{}/commits/{}",
            owner, repo, commit.sha
        );
        let detail_response = client
            .get(&commit_detail_url)
            .header("User-Agent", "balatro-mod-manager")
            .send()
            .await
            .map_err(|e| format!("Failed to fetch commit details: {}", e))?;

        if !detail_response.status().is_success() {
            continue; // Skip this commit if we can't get details
        }

        let commit_detail: GitHubCommit = match detail_response.json().await {
            Ok(detail) => detail,
            Err(_) => continue, // Skip if we can't parse
        };

        // Process files in this commit
        for file in commit_detail.files {
            if file.filename.starts_with("mods/") {
                let parts: Vec<&str> = file
                    .filename
                    .trim_start_matches("mods/")
                    .split('/')
                    .collect();
                if let Some(mod_name) = parts.first() {
                    timestamps
                        .entry(mod_name.to_string())
                        .and_modify(|e| {
                            if timestamp > *e {
                                *e = timestamp
                            }
                        })
                        .or_insert(timestamp);
                }
            }
        }
    }

    Ok(timestamps)
}

pub async fn pull_repository(path: &str) -> Result<(), String> {
    let repo_path = PathBuf::from(path);
    let git_info_path = repo_path.join(".git_info");

    // Check if .git_info exists
    if !git_info_path.exists() {
        return Err(format!(
            "Directory at '{}' is not a valid repository. Please clone it first.",
            path
        ));
    }

    // Rest of the function remains the same...
    let url = std::fs::read_to_string(&git_info_path)
        .map_err(|e| format!("Failed to read repository info: {}", e))?;

    // Delete everything except .git_info
    for entry in
        std::fs::read_dir(&repo_path).map_err(|e| format!("Failed to read directory: {}", e))?
    {
        let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
        let entry_path = entry.path();

        // Skip .git_info file
        if entry_path.to_string_lossy() == git_info_path.to_string_lossy() {
            continue;
        }

        // Remove file or directory
        if entry
            .file_type()
            .map_err(|e| format!("Failed to get file type: {}", e))?
            .is_dir()
        {
            std::fs::remove_dir_all(&entry_path)
                .map_err(|e| format!("Failed to remove directory: {}", e))?;
        } else {
            std::fs::remove_file(&entry_path)
                .map_err(|e| format!("Failed to remove file: {}", e))?;
        }
    }

    // Clone again
    clone_repository(&url, path).await
}
