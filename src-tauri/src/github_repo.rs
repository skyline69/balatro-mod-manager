// use chrono::DateTime;
// use serde::Deserialize;
// use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
//

// const CURRENT_BRANCH: &str = "80-suggestion-allow-specifying-mod-folder-name";
const CURRENT_BRANCH: &str = "main";

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

    // Determine which branch to use based on the repository
    let mut branch = if url.contains("skyline69/balatro-mod-index") {
        CURRENT_BRANCH
    } else {
        // Default to "main" initially
        "main"
    };

    // Download the repository as a zip file from the chosen branch
    let mut download_url = format!(
        "https://github.com/{}/{}/archive/refs/heads/{}.zip",
        owner, repo, branch
    );

    let mut response = reqwest::get(&download_url).await;

    // If the first attempt fails and we're not using a specific branch already,
    // try with "master" branch instead
    if response.is_err()
        && branch == "main"
        && url != "https://github.com/skyline69/balatro-mod-index"
    {
        branch = "master"; // Update the branch variable
        download_url = format!(
            "https://github.com/{}/{}/archive/refs/heads/{}.zip",
            owner, repo, branch
        );
        response = reqwest::get(&download_url).await;
    }

    // If still failing, return the error
    let response = response.map_err(|e| format!("Failed to download repository: {}", e))?;

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

    // Create a simple .git_info file to store repo URL and branch (for pulls)
    let git_info = target_path.join(".git_info");
    let info_content = format!("{}\nbranch={}", url, branch);
    std::fs::write(git_info, info_content)
        .map_err(|e| format!("Failed to write repository info: {}", e))?;

    Ok(())
}

pub fn is_repository_directory(path: &str) -> bool {
    let repo_path = PathBuf::from(path);
    let git_info_path = repo_path.join(".git_info");
    git_info_path.exists()
}

// Get mod timestamps from the repository
// pub async fn get_mod_timestamps(repo_path: &str) -> Result<HashMap<String, i64>, String> {
//     let client = reqwest::Client::new();
//     let repo_path = PathBuf::from(repo_path);
//
//     // Read repo URL from .git_info file
//     let git_info_path = repo_path.join(".git_info");
//     let url = std::fs::read_to_string(git_info_path)
//         .map_err(|e| format!("Failed to read repository info: {}", e))?;
//
//     let (owner, repo) =
//         parse_github_url(&url).ok_or_else(|| "Invalid GitHub URL format".to_string())?;
//
//     // Get commits for the repository
//     let commits_url = format!("https://api.github.com/repos/{}/{}/commits", owner, repo);
//
//     let response = client
//         .get(&commits_url)
//         .header("User-Agent", "balatro-mod-manager")
//         .send()
//         .await
//         .map_err(|e| format!("Failed to fetch commits: {}", e))?;
//
//     if !response.status().is_success() {
//         return Err(format!(
//             "GitHub API error: {} - {}",
//             response.status().as_u16(),
//             response.text().await.unwrap_or_default()
//         ));
//     }
//
//     let commits: Vec<GitHubCommit> = response
//         .json()
//         .await
//         .map_err(|e| format!("Failed to parse commits: {}", e))?;
//
//     let mut timestamps = HashMap::new();
//
//     for commit in commits {
//         // Parse commit date to Unix timestamp
//         let date = DateTime::parse_from_rfc3339(&commit.commit.author.date)
//             .map_err(|e| format!("Failed to parse date: {}", e))?;
//         let timestamp = date.timestamp();
//
//         // Get the files included in this commit
//         let commit_detail_url = format!(
//             "https://api.github.com/repos/{}/{}/commits/{}",
//             owner, repo, commit.sha
//         );
//         let detail_response = client
//             .get(&commit_detail_url)
//             .header("User-Agent", "balatro-mod-manager")
//             .send()
//             .await
//             .map_err(|e| format!("Failed to fetch commit details: {}", e))?;
//
//         if !detail_response.status().is_success() {
//             continue; // Skip this commit if we can't get details
//         }
//
//         let commit_detail: GitHubCommit = match detail_response.json().await {
//             Ok(detail) => detail,
//             Err(_) => continue, // Skip if we can't parse
//         };
//
//         // Process files in this commit
//         for file in commit_detail.files {
//             if file.filename.starts_with("mods/") {
//                 let parts: Vec<&str> = file
//                     .filename
//                     .trim_start_matches("mods/")
//                     .split('/')
//                     .collect();
//                 if let Some(mod_name) = parts.first() {
//                     timestamps
//                         .entry(mod_name.to_string())
//                         .and_modify(|e| {
//                             if timestamp > *e {
//                                 *e = timestamp
//                             }
//                         })
//                         .or_insert(timestamp);
//                 }
//             }
//         }
//     }
//
//     Ok(timestamps)
// }
//
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

    // Read the git info content
    let git_info_content = std::fs::read_to_string(&git_info_path)
        .map_err(|e| format!("Failed to read repository info: {}", e))?;

    // Parse the content - first line is the URL
    let lines: Vec<&str> = git_info_content.lines().collect();
    let url = lines[0].trim().to_string();

    log::info!("URL from git_info: {}", url);

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

    // For balatro-mod-index, always use the specific branch regardless of what's in .git_info
    if url.contains("skyline69/balatro-mod-index") {
        clone_repository_with_branch(&url, path, CURRENT_BRANCH).await
    } else {
        // For other repositories, use the saved branch or default to main/master
        let branch_line = lines.get(1).unwrap_or(&"");
        let branch_prefix = "branch=";
        if let Some(stripped) = branch_line.strip_prefix(branch_prefix) {
            let branch = stripped.trim();
            clone_repository_with_branch(&url, path, branch).await
        } else {
            clone_repository(&url, path).await
        }
    }
}

pub async fn clone_repository_with_branch(
    url: &str,
    path: &str,
    branch: &str,
) -> Result<(), String> {
    let (owner, repo) =
        parse_github_url(url).ok_or_else(|| "Invalid GitHub URL format".to_string())?;

    // Download the repository as a zip file from the specified branch
    let download_url = format!(
        "https://github.com/{}/{}/archive/refs/heads/{}.zip",
        owner, repo, branch
    );

    log::info!("Downloading from URL: {}", download_url); // Debug print

    let response = reqwest::get(&download_url)
        .await
        .map_err(|e| format!("Failed to download repository from {}: {}", download_url, e))?;

    if !response.status().is_success() {
        return Err(format!(
            "GitHub returned error status: {} for URL {}",
            response.status(),
            download_url
        ));
    }

    let bytes = response
        .bytes()
        .await
        .map_err(|e| format!("Failed to read response: {}", e))?;

    log::info!("Downloaded {} bytes", bytes.len()); // Debug print

    // Create target directory
    let target_path = PathBuf::from(path);
    std::fs::create_dir_all(&target_path)
        .map_err(|e| format!("Failed to create directory: {}", e))?;

    // Save zip file to temporary location
    let temp_zip = target_path.join("temp.zip");
    let mut file = File::create(&temp_zip).map_err(|e| format!("Failed to create file: {}", e))?;
    file.write_all(&bytes)
        .map_err(|e| format!("Failed to write file: {}", e))?;

    log::info!("Saved zip file to {}", temp_zip.display()); // Debug print

    // Extract zip file
    let file =
        std::fs::File::open(&temp_zip).map_err(|e| format!("Failed to open zip file: {}", e))?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|e| format!("Failed to parse zip file: {}. This might mean the downloaded file is not a valid zip archive.", e))?;

    log::info!(
        "Successfully opened zip archive with {} files",
        archive.len()
    ); // Debug print

    // Rest of the extraction code remains the same...
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

    // Create a simple .git_info file to store repo URL and branch (for pulls)
    let git_info = target_path.join(".git_info");
    let info_content = format!("{}\nbranch={}", url, branch);
    std::fs::write(git_info, info_content)
        .map_err(|e| format!("Failed to write repository info: {}", e))?;

    log::info!("Successfully cloned repository with branch: {}", branch); // Debug print

    Ok(())
}
