// src-tauri/bmm-lib/src/balamod.rs
use crate::errors::AppError;
use crate::finder::get_balatro_paths;
use libflate::deflate::Encoder; // Correct import if using libflate
                                // If you intended flate2: use flate2::write::DeflateEncoder; use flate2::Compression;
use log::error;
use std::fs;
use std::fs::File;
use std::io::{BufReader, Cursor, Read, Write};
use std::path::PathBuf;
use zip::write::FileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter}; // Adjusted zip imports

#[derive(Clone, Debug)]
pub struct Balatro {
    pub path: PathBuf,
}

impl Balatro {
    #[cfg(target_os = "macos")]
    pub fn get_exe_path(&self) -> PathBuf {
        self.path
            .clone()
            .join("Balatro.app/Contents/Resources/Balatro.love")
    }
    #[cfg(target_os = "windows")]
    pub fn get_exe_path(&self) -> PathBuf {
        let exe_path = self.path.clone().join("Balatro.exe");
        if exe_path.exists() {
            exe_path
        } else {
            self.path.clone() // Fallback
        }
    }
    #[cfg(target_os = "linux")]
    pub fn get_exe_path(&self) -> PathBuf {
        let love_path = self.path.clone().join("Balatro.love");
        let exe_path = self.path.clone().join("Balatro.exe"); // For Proton
        if love_path.exists() {
            love_path
        } else if exe_path.exists() {
            exe_path
        } else {
            self.path.clone() // Fallback
        }
    }

    pub fn replace_file(&self, file_name: &str, new_contents: &[u8]) -> Result<(), std::io::Error> {
        let exe_path_buf = self.get_exe_path();
        if exe_path_buf.is_dir() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Could not determine exact executable/archive path",
            ));
        }
        let exe_path = exe_path_buf
            .to_str()
            .expect("Failed to convert exe_path to str");
        self.replace_file_in_archive(exe_path, file_name, new_contents)
    }

    pub fn get_file_data(&self, file_name: &str) -> Result<Vec<u8>, std::io::Error> {
        let exe_path_buf = self.get_exe_path();
        if exe_path_buf.is_dir() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Could not determine exact executable/archive path",
            ));
        }
        let exe_path = exe_path_buf
            .to_str()
            .expect("Failed to convert exe_path to str");
        let file = File::open(exe_path)?;
        let mut archive = ZipArchive::new(BufReader::new(file))?;

        for i in 0..archive.len() {
            let mut file = archive.by_index(i)?;
            if file.name() == file_name {
                let mut contents = Vec::new();
                file.read_to_end(&mut contents)?;
                return Ok(contents);
            }
        }
        error!("'{}' not found in the archive.", file_name);
        Ok(Vec::new())
    }

    pub fn get_all_files(&self) -> Result<Vec<String>, std::io::Error> {
        let exe_path_buf = self.get_exe_path();
        if exe_path_buf.is_dir() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Could not determine exact executable/archive path",
            ));
        }
        let exe_path = exe_path_buf
            .to_str()
            .expect("Failed to convert exe_path to str");
        let file = File::open(exe_path)?;
        let mut archive = ZipArchive::new(BufReader::new(file))?;

        let mut files = Vec::new();
        for i in 0..archive.len() {
            let file = archive.by_index(i)?;
            files.push(file.name().to_string());
        }
        Ok(files)
    }

    pub fn get_version(&self) -> Result<String, std::io::Error> {
        let exe_path_buf = self.get_exe_path();
        if exe_path_buf.is_dir() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Could not determine exact executable/archive path",
            ));
        }
        let file = File::open(exe_path_buf)?;
        let mut archive = ZipArchive::new(BufReader::new(file))?;

        for i in 0..archive.len() {
            let mut file = archive.by_index(i)?;
            if file.name() == "version.jkr" {
                let mut contents = String::new();
                file.read_to_string(&mut contents)?;
                let version = contents.lines().nth(1).unwrap_or("").to_string();
                return Ok(version);
            }
        }
        error!("'version.jkr' not found in the archive.");
        Ok("0.0.0".to_string())
    }

    fn replace_file_in_archive(
        &self,
        archive_path: &str,
        file_name: &str,
        new_contents: &[u8],
    ) -> Result<(), std::io::Error> {
        let mut archive_data = fs::read(archive_path)?;
        let zip_start = self.find_zip_start(&archive_data).unwrap_or(0);
        let cursor = Cursor::new(&archive_data[zip_start..]);
        let mut zip_archive = ZipArchive::new(cursor)?;
        let mut new_zip = Vec::new();

        {
            let mut zip_writer = ZipWriter::new(Cursor::new(&mut new_zip));
            for i in 0..zip_archive.len() {
                let raw_file = zip_archive.by_index_raw(i)?;
                if raw_file.name() == file_name {
                    continue;
                }
                zip_writer.raw_copy_file(raw_file)?;
            }

            // --- Fix E0283 Here ---
            // Specify the generic argument for the options type as ()
            zip_writer.start_file::<_, ()>( // Use turbofish ::<_, ()>
                file_name,
                FileOptions::default().compression_method(CompressionMethod::Deflated), // Use Deflate for .love
            )?;
            // --- End Fix ---

            zip_writer.write_all(new_contents)?;
            zip_writer.finish()?;
        }

        archive_data.splice(zip_start.., new_zip);
        fs::write(archive_path, archive_data)?;
        Ok(())
    }

    fn find_zip_start(&self, data: &[u8]) -> Option<usize> {
        let zip_signature: [u8; 4] = [0x50, 0x4b, 0x03, 0x04];
        data.windows(4).position(|window| window == zip_signature)
    }

    pub fn compress_file(&self, input_path: &str, output_path: &str) -> Result<(), std::io::Error> {
        let mut input_file = File::open(input_path)?;
        let mut buffer = Vec::new();
        input_file.read_to_end(&mut buffer)?;

        let mut encoder = Encoder::new(Vec::new()); // Assuming libflate based on previous context
        encoder.write_all(&buffer)?;
        let compressed = encoder.finish().into_result()?;

        let mut output_file = File::create(output_path)?;
        output_file.write_all(&compressed)?;
        Ok(())
    }

    pub fn is_valid(&self) -> bool {
        let potential_exe = self.get_exe_path();
        potential_exe.exists() && potential_exe.is_file()
    }

    pub fn from_custom_path(path: PathBuf) -> Option<Self> {
        let balatro = Balatro { path };
        if balatro.is_valid() {
            Some(balatro)
        } else {
            None
        }
    }
}

pub fn find_balatros() -> Vec<Balatro> {
    let paths: Vec<PathBuf> = get_balatro_paths();
    let mut balatros = Vec::new();
    for path in paths {
        let balatro = Balatro { path };
        if balatro.is_valid() {
            balatros.push(balatro);
        }
    }
    balatros
}

pub fn get_balatro_save_directory() -> Result<PathBuf, AppError> {
    let linux_native = false; // Default assumption

    let save_dir_str: String; // Declare without initializing

    if cfg!(target_os = "macos") {
        let home = dirs::home_dir()
            .ok_or_else(|| AppError::DirNotFound(PathBuf::from("Home directory")))?;
        save_dir_str = home
            .join("Library/Application Support/Balatro")
            .to_string_lossy()
            .to_string();
    } else if cfg!(target_os = "windows") {
        let appdata = std::env::var("APPDATA")
            .map_err(|_| AppError::DirNotFound(PathBuf::from("%APPDATA%")))?;
        save_dir_str = format!("{}/Balatro", appdata);
    } else if cfg!(target_os = "linux") {
        let home = std::env::var("HOME")
            .map_err(|_| AppError::DirNotFound(PathBuf::from("Home directory")))?;
        if linux_native {
            save_dir_str = format!("{}/.local/share/love/Balatro", home);
        } else {
            save_dir_str = format!("{}/.local/share/Steam/steamapps/compatdata/2379780/pfx/drive_c/users/steamuser/AppData/Roaming/Balatro", home);
        }
    } else {
        return Err(AppError::SystemDetection("Unsupported OS".to_string()));
    }

    // save_dir_str is now guaranteed to be initialized
    let save_dir = PathBuf::from(save_dir_str);

    if !save_dir.exists() {
        log::warn!("Primary save path not found: {}", save_dir.display());
        #[cfg(target_os = "linux")]
        if !linux_native {
            let home = std::env::var("HOME")
                .map_err(|_| AppError::DirNotFound(PathBuf::from("Home directory")))?;
            let native_path = PathBuf::from(format!("{}/.local/share/love/Balatro", home));
            if native_path.exists() {
                log::info!(
                    "Found alternative native Linux save path: {}",
                    native_path.display()
                );
                return Ok(native_path);
            }
        }
        // Add similar checks for other OS if necessary
        return Err(AppError::DirNotFound(save_dir));
    }

    Ok(save_dir)
}
