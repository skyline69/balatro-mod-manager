use std::fs::File;
use std::io::{Cursor, Read, Seek};
use std::path::{Path, PathBuf};

use flate2::read::GzDecoder;
use sevenz_rust::{Error as SevenZError, decompress_with_extract_fn};
use tar::Archive;
use unrar::Archive as UnrarArchive;
use zip::ZipArchive;

use crate::compat_helper;
use crate::state::AppState;
use bmm_lib::errors::AppError;

async fn sync_compat_helper_after_mod_change(state: &tauri::State<'_, AppState>) {
    let enabled = {
        let db = state.db.lock().unwrap_or_else(|e| e.into_inner());
        db.is_compat_helper_enabled().unwrap_or(false)
    };
    if let Err(err) = compat_helper::sync_compat_helper(enabled) {
        log::warn!("Failed to sync compatibility helper after mod change: {err}");
    }
}

#[tauri::command]
pub async fn process_dropped_file(
    state: tauri::State<'_, AppState>,
    path: String,
) -> Result<String, String> {
    let config_dir =
        dirs::config_dir().ok_or_else(|| "Could not find config directory".to_string())?;
    let mods_dir = config_dir.join("Balatro").join("Mods");
    tokio::fs::create_dir_all(&mods_dir)
        .await
        .map_err(|e| format!("Failed to create mods directory: {e}"))?;

    let file_path_str = path.clone();
    let mods_dir_clone = mods_dir.clone();

    let mod_dir = tokio::task::spawn_blocking(move || {
        let file_path = std::path::Path::new(&file_path_str);
        let file_name = file_path
            .file_name()
            .ok_or_else(|| "Invalid file path".to_string())?
            .to_str()
            .ok_or_else(|| "Invalid file name".to_string())?;

        let file = File::open(file_path).map_err(|e| format!("Failed to open file: {e}"))?;

        extract_archive_from_reader(file_name, file, Some(file_path), &mods_dir_clone)
            .map_err(|e| format!("Failed to extract archive: {e}"))
    })
    .await
    .map_err(|e| format!("Task join error: {e}"))??;

    sync_compat_helper_after_mod_change(&state).await;
    Ok(mod_dir.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn process_mod_archive(
    state: tauri::State<'_, AppState>,
    filename: String,
    data: Vec<u8>,
) -> Result<String, String> {
    let config_dir = dirs::config_dir()
        .ok_or_else(|| AppError::DirNotFound(PathBuf::from("config directory")).to_string())?;
    let mods_dir = config_dir.join("Balatro").join("Mods");
    tokio::fs::create_dir_all(&mods_dir)
        .await
        .map_err(|e| format!("Failed to create mods directory: {e}"))?;

    let mods_dir_clone = mods_dir.clone();
    let filename_clone = filename.clone();

    let mod_dir = tokio::task::spawn_blocking(move || {
        let cursor = Cursor::new(data);
        let mod_dir = extract_archive_from_reader(&filename_clone, cursor, None, &mods_dir_clone)?;

        // Flatten nested directory if needed
        if let Ok(entries) = std::fs::read_dir(&mod_dir) {
            let dirs: Vec<_> = entries
                .filter_map(Result::ok)
                .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
                .collect();
            if dirs.len() == 1 && std::fs::read_dir(&mod_dir).map(|e| e.count()).unwrap_or(0) == 1 {
                let nested_dir = dirs[0].path();
                for entry in std::fs::read_dir(&nested_dir)
                    .map_err(|e| format!("Failed to read nested directory: {e}"))?
                {
                    let entry = entry.map_err(|e| format!("Failed to read entry: {e}"))?;
                    let target_path = mod_dir.join(entry.file_name());
                    if entry
                        .file_type()
                        .map_err(|e| format!("Failed to get file type: {e}"))?
                        .is_dir()
                    {
                        std::fs::rename(entry.path(), &target_path)
                            .map_err(|e| format!("Failed to move directory: {e}"))?;
                    } else {
                        std::fs::rename(entry.path(), &target_path)
                            .map_err(|e| format!("Failed to move file: {e}"))?;
                    }
                }
                std::fs::remove_dir_all(&nested_dir)
                    .map_err(|e| format!("Failed to remove nested directory: {e}"))?;
            }
        }

        Ok::<_, String>(mod_dir)
    })
    .await
    .map_err(|e| format!("Task join error: {e}"))??;

    sync_compat_helper_after_mod_change(&state).await;
    Ok(mod_dir.to_string_lossy().to_string())
}

fn extract_archive_from_reader<R: Read + Seek>(
    filename: &str,
    reader: R,
    source_path: Option<&Path>,
    mods_dir: &std::path::Path,
) -> Result<PathBuf, String> {
    let mod_dir_name = filename
        .trim_end_matches(".zip")
        .trim_end_matches(".tar.gz")
        .trim_end_matches(".tgz")
        .trim_end_matches(".tar")
        .trim_end_matches(".rar")
        .trim_end_matches(".7z")
        .to_string();
    let mod_dir = mods_dir.join(&mod_dir_name);

    if mod_dir.exists() {
        std::fs::remove_dir_all(&mod_dir)
            .map_err(|e| format!("Failed to remove existing mod directory: {e}"))?;
    }

    if filename.ends_with(".zip") {
        extract_zip(reader, &mod_dir)?;
    } else if filename.ends_with(".rar") {
        let source_path =
            source_path.ok_or_else(|| "RAR archives require a file path".to_string())?;
        extract_rar(source_path, &mod_dir)?;
    } else if filename.ends_with(".7z") {
        extract_7z(reader, &mod_dir)?;
    } else if filename.ends_with(".tar") {
        extract_tar(reader, &mod_dir)?;
    } else if filename.ends_with(".tar.gz") || filename.ends_with(".tgz") {
        extract_tar_gz(reader, &mod_dir)?;
    } else {
        return Err(
            "Unsupported file format. Only ZIP, TAR, TAR.GZ, RAR, and 7Z are supported."
                .to_string(),
        );
    }

    Ok(mod_dir)
}

fn extract_7z<R: Read + Seek>(reader: R, target_dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(target_dir)
        .map_err(|e| format!("Failed to create target directory: {e}"))?;

    decompress_with_extract_fn(reader, target_dir, |entry, contents, _| {
        let relative_path = sanitize_7z_path(entry.name())
            .ok_or_else(|| SevenZError::other("7Z archive contains an unsafe entry path"))?;
        let output_path = target_dir.join(relative_path);

        if entry.is_directory() {
            std::fs::create_dir_all(&output_path).map_err(SevenZError::io)?;
        } else {
            if let Some(parent) = output_path.parent() {
                std::fs::create_dir_all(parent).map_err(SevenZError::io)?;
            }
            let mut output = File::create(&output_path).map_err(SevenZError::io)?;
            std::io::copy(contents, &mut output).map_err(SevenZError::io)?;
        }

        Ok(true)
    })
    .map_err(|e| format!("Failed to extract 7Z archive: {e}"))
}

fn sanitize_7z_path(entry_name: &str) -> Option<PathBuf> {
    let mut path = PathBuf::new();
    for component in Path::new(entry_name).components() {
        match component {
            std::path::Component::Normal(name) => path.push(name),
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir
            | std::path::Component::Prefix(_)
            | std::path::Component::RootDir => return None,
        }
    }
    (!path.as_os_str().is_empty()).then_some(path)
}

fn extract_zip<R: Read + Seek>(reader: R, target_dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(target_dir)
        .map_err(|e| format!("Failed to create target directory: {e}"))?;
    let mut archive =
        ZipArchive::new(reader).map_err(|e| format!("Failed to open ZIP archive: {e}"))?;
    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| format!("Failed to access file in archive: {e}"))?;
        if file.name().starts_with("__MACOSX/") {
            continue;
        }
        let file_path = match file.enclosed_name() {
            Some(path) => path.to_owned(),
            None => continue,
        };
        let output_path = target_dir.join(&file_path);
        if file.is_dir() {
            std::fs::create_dir_all(&output_path)
                .map_err(|e| format!("Failed to create directory: {e}"))?;
        } else {
            if let Some(parent) = output_path.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| format!("Failed to create parent directory: {e}"))?;
            }
            let mut outfile = std::fs::File::create(&output_path)
                .map_err(|e| format!("Failed to create file {}: {e}", output_path.display()))?;
            std::io::copy(&mut file, &mut outfile)
                .map_err(|e| format!("Failed to write file {}: {e}", output_path.display()))?;
        }
    }
    Ok(())
}

fn extract_tar<R: Read>(reader: R, target_dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(target_dir)
        .map_err(|e| format!("Failed to create target directory: {e}"))?;
    let mut archive = Archive::new(reader);
    for entry in archive
        .entries()
        .map_err(|e| format!("Failed to read TAR entries: {e}"))?
    {
        let mut entry = entry.map_err(|e| format!("Failed to read TAR entry: {e}"))?;
        let path = entry
            .path()
            .map_err(|e| format!("Failed to get entry path: {e}"))?;
        let output_path = target_dir.join(path);
        if let Some(parent) = output_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create parent directory: {e}"))?;
        }
        entry
            .unpack(&output_path)
            .map_err(|e| format!("Failed to unpack file {}: {e}", output_path.display()))?;
    }
    Ok(())
}

fn extract_tar_gz<R: Read>(reader: R, target_dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(target_dir)
        .map_err(|e| format!("Failed to create target directory: {e}"))?;
    let gz = GzDecoder::new(reader);
    let mut archive = Archive::new(gz);
    for entry in archive
        .entries()
        .map_err(|e| format!("Failed to read TAR entries: {e}"))?
    {
        let mut entry = entry.map_err(|e| format!("Failed to read TAR entry: {e}"))?;
        let path = entry
            .path()
            .map_err(|e| format!("Failed to get entry path: {e}"))?;
        let output_path = target_dir.join(path);
        if let Some(parent) = output_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create parent directory: {e}"))?;
        }
        entry
            .unpack(&output_path)
            .map_err(|e| format!("Failed to unpack file {}: {e}", output_path.display()))?;
    }
    Ok(())
}

fn extract_rar(path: &Path, target_dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(target_dir)
        .map_err(|e| format!("Failed to create target directory: {e}"))?;
    let mut archive = UnrarArchive::new(path)
        .as_first_part()
        .open_for_processing()
        .map_err(|e| format!("Failed to open RAR archive: {e}"))?;
    while let Some(header) = archive
        .read_header()
        .map_err(|e| format!("Failed to read RAR entry: {e}"))?
    {
        archive = if header.entry().is_file() {
            header
                .extract_with_base(target_dir)
                .map_err(|e| format!("Failed to extract RAR entry: {e}"))?
        } else {
            header
                .skip()
                .map_err(|e| format!("Failed to skip RAR entry: {e}"))?
        };
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_extract_zip_basic() {
        let temp_dir = TempDir::new().unwrap();
        let target = temp_dir.path().join("extracted");

        // Create a minimal valid ZIP file in memory
        let mut zip_data = Vec::new();
        {
            let cursor = std::io::Cursor::new(&mut zip_data);
            let mut zip = zip::ZipWriter::new(cursor);
            let options = zip::write::SimpleFileOptions::default();
            zip.start_file("test.txt", options).unwrap();
            zip.write_all(b"hello world").unwrap();
            zip.finish().unwrap();
        }

        let cursor = std::io::Cursor::new(zip_data);
        extract_zip(cursor, &target).unwrap();

        assert!(target.join("test.txt").exists());
        let content = std::fs::read_to_string(target.join("test.txt")).unwrap();
        assert_eq!(content, "hello world");
    }

    #[test]
    fn test_extract_zip_with_directory() {
        let temp_dir = TempDir::new().unwrap();
        let target = temp_dir.path().join("extracted");

        let mut zip_data = Vec::new();
        {
            let cursor = std::io::Cursor::new(&mut zip_data);
            let mut zip = zip::ZipWriter::new(cursor);
            let options = zip::write::SimpleFileOptions::default();
            zip.add_directory("subdir/", options).unwrap();
            zip.start_file("subdir/file.txt", options).unwrap();
            zip.write_all(b"nested content").unwrap();
            zip.finish().unwrap();
        }

        let cursor = std::io::Cursor::new(zip_data);
        extract_zip(cursor, &target).unwrap();

        assert!(target.join("subdir").is_dir());
        assert!(target.join("subdir/file.txt").exists());
    }

    #[test]
    fn test_extract_zip_skips_macosx_folder() {
        let temp_dir = TempDir::new().unwrap();
        let target = temp_dir.path().join("extracted");

        let mut zip_data = Vec::new();
        {
            let cursor = std::io::Cursor::new(&mut zip_data);
            let mut zip = zip::ZipWriter::new(cursor);
            let options = zip::write::SimpleFileOptions::default();
            zip.start_file("valid.txt", options).unwrap();
            zip.write_all(b"valid").unwrap();
            zip.start_file("__MACOSX/junk.txt", options).unwrap();
            zip.write_all(b"junk").unwrap();
            zip.finish().unwrap();
        }

        let cursor = std::io::Cursor::new(zip_data);
        extract_zip(cursor, &target).unwrap();

        assert!(target.join("valid.txt").exists());
        assert!(!target.join("__MACOSX").exists());
    }

    #[test]
    fn test_extract_tar_basic() {
        let temp_dir = TempDir::new().unwrap();
        let target = temp_dir.path().join("extracted");

        let mut tar_data = Vec::new();
        {
            let mut builder = tar::Builder::new(&mut tar_data);
            let mut header = tar::Header::new_gnu();
            let content = b"tar content";
            header.set_size(content.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            builder
                .append_data(&mut header, "file.txt", &content[..])
                .unwrap();
            builder.finish().unwrap();
        }

        let cursor = std::io::Cursor::new(tar_data);
        extract_tar(cursor, &target).unwrap();

        assert!(target.join("file.txt").exists());
    }

    #[test]
    fn test_extract_tar_gz_basic() {
        let temp_dir = TempDir::new().unwrap();
        let target = temp_dir.path().join("extracted");

        // Create tar first
        let mut tar_data = Vec::new();
        {
            let mut builder = tar::Builder::new(&mut tar_data);
            let mut header = tar::Header::new_gnu();
            let content = b"gzipped tar content";
            header.set_size(content.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            builder
                .append_data(&mut header, "gzfile.txt", &content[..])
                .unwrap();
            builder.finish().unwrap();
        }

        // Compress with gzip
        let mut gz_data = Vec::new();
        {
            let mut encoder =
                flate2::write::GzEncoder::new(&mut gz_data, flate2::Compression::default());
            encoder.write_all(&tar_data).unwrap();
            encoder.finish().unwrap();
        }

        let cursor = std::io::Cursor::new(gz_data);
        extract_tar_gz(cursor, &target).unwrap();

        assert!(target.join("gzfile.txt").exists());
    }

    #[test]
    fn test_mod_dir_name_from_zip() {
        let filename = "TestMod.zip";
        let mod_name = filename
            .trim_end_matches(".zip")
            .trim_end_matches(".tar.gz")
            .trim_end_matches(".tgz")
            .trim_end_matches(".tar")
            .trim_end_matches(".rar");
        assert_eq!(mod_name, "TestMod");
    }

    #[test]
    fn test_mod_dir_name_from_tar_gz() {
        let filename = "TestMod.tar.gz";
        let mod_name = filename
            .trim_end_matches(".zip")
            .trim_end_matches(".tar.gz")
            .trim_end_matches(".tgz")
            .trim_end_matches(".tar")
            .trim_end_matches(".rar");
        assert_eq!(mod_name, "TestMod");
    }

    #[test]
    fn test_mod_dir_name_from_tgz() {
        let filename = "TestMod.tgz";
        let mod_name = filename
            .trim_end_matches(".zip")
            .trim_end_matches(".tar.gz")
            .trim_end_matches(".tgz")
            .trim_end_matches(".tar")
            .trim_end_matches(".rar");
        assert_eq!(mod_name, "TestMod");
    }

    #[test]
    fn test_mod_dir_name_from_rar() {
        let filename = "TestMod.rar";
        let mod_name = filename
            .trim_end_matches(".zip")
            .trim_end_matches(".tar.gz")
            .trim_end_matches(".tgz")
            .trim_end_matches(".tar")
            .trim_end_matches(".rar");
        assert_eq!(mod_name, "TestMod");
    }

    #[test]
    fn test_extract_7z_basic() {
        let temp_dir = TempDir::new().unwrap();
        let source_dir = temp_dir.path().join("source");
        std::fs::create_dir_all(&source_dir).unwrap();
        std::fs::write(source_dir.join("init.lua"), "-- mod").unwrap();

        let archive_path = temp_dir.path().join("mod.7z");
        let mut archive = sevenz_rust::SevenZWriter::create(&archive_path).unwrap();
        archive.push_source_path(&source_dir, |_| true).unwrap();
        archive.finish().unwrap();

        let target = temp_dir.path().join("extracted");
        extract_7z(File::open(&archive_path).unwrap(), &target).unwrap();

        assert_eq!(
            std::fs::read_to_string(target.join("init.lua")).unwrap(),
            "-- mod"
        );
    }

    #[test]
    fn test_unsupported_format_error() {
        let temp_dir = TempDir::new().unwrap();
        let mods_dir = temp_dir.path();
        let cursor = std::io::Cursor::new(Vec::<u8>::new());

        let result = extract_archive_from_reader("file.invalid", cursor, None, mods_dir);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unsupported file format"));
    }

    #[test]
    fn test_rar_requires_file_path() {
        let temp_dir = TempDir::new().unwrap();
        let mods_dir = temp_dir.path();
        let cursor = std::io::Cursor::new(Vec::<u8>::new());

        let result = extract_archive_from_reader("file.rar", cursor, None, mods_dir);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .contains("RAR archives require a file path")
        );
    }

    #[test]
    fn test_extract_archive_creates_mod_dir() {
        let temp_dir = TempDir::new().unwrap();
        let mods_dir = temp_dir.path();

        let mut zip_data = Vec::new();
        {
            let cursor = std::io::Cursor::new(&mut zip_data);
            let mut zip = zip::ZipWriter::new(cursor);
            let options = zip::write::SimpleFileOptions::default();
            zip.start_file("init.lua", options).unwrap();
            zip.write_all(b"-- mod").unwrap();
            zip.finish().unwrap();
        }

        let cursor = std::io::Cursor::new(zip_data);
        let result = extract_archive_from_reader("MyMod.zip", cursor, None, mods_dir);

        assert!(result.is_ok());
        let mod_dir = result.unwrap();
        assert_eq!(mod_dir.file_name().unwrap().to_str().unwrap(), "MyMod");
        assert!(mod_dir.join("init.lua").exists());
    }
}
