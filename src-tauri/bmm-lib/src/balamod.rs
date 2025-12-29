use crate::finder::get_balatro_paths;
use libflate::deflate::Encoder;
use log::error;
use std::fs;
use std::fs::File;
use std::io::{BufReader, Cursor, Read, Write};
#[cfg(target_os = "macos")]
use std::path::Path;
use std::path::PathBuf;
use zip::ZipArchive;
use zip::{CompressionMethod, ZipWriter, write::FileOptions};

#[derive(Clone, Debug)]
pub struct Balatro {
    pub path: PathBuf,
}

#[cfg(target_os = "macos")]
const MAX_BUNDLE_SEARCH_DEPTH: usize = 12;

impl Balatro {
    #[cfg(target_os = "macos")]
    pub fn get_exe_path(&self) -> PathBuf {
        self.find_balatro_love().unwrap_or_else(|| {
            self.path
                .clone()
                .join("Balatro.app/Contents/Resources/Balatro.love")
        })
    }
    #[cfg(target_os = "windows")]
    pub fn get_exe_path(&self) -> PathBuf {
        self.path.clone().join("Balatro.exe")
    }
    #[cfg(target_os = "linux")]
    pub fn get_exe_path(&self) -> PathBuf {
        self.path.clone().join("Balatro.exe")
    }

    pub fn replace_file(&self, file_name: &str, new_contents: &[u8]) -> Result<(), std::io::Error> {
        let exe_path_buf = self.get_exe_path();
        let exe_path = exe_path_buf
            .to_str()
            .expect("Failed to convert exe_path to str");
        self.replace_file_in_exe(exe_path, file_name, new_contents)
    }

    pub fn get_file_data(&self, file_name: &str) -> Result<Vec<u8>, std::io::Error> {
        let exe_path_buf = self.get_exe_path();
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
        error!("'{file_name}' not found in the archive.");
        Ok(Vec::new())
    }

    pub fn get_all_files(&self) -> Result<Vec<String>, std::io::Error> {
        let exe_path_buf = self.get_exe_path();
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
        let file = File::open(self.get_exe_path())?;
        let mut archive = ZipArchive::new(BufReader::new(file))?;

        for i in 0..archive.len() {
            let mut file = archive.by_index(i)?;
            if file.name() == "version.jkr" {
                let mut contents = String::new();
                file.read_to_string(&mut contents)?;
                let version = contents.lines().nth(1).unwrap().to_string();
                return Ok(version);
            }
        }
        error!("'version.jkr' not found in the archive.");
        Ok("0.0.0".to_string())
    }

    fn replace_file_in_exe(
        &self,
        exe_path: &str,
        file_name: &str,
        new_contents: &[u8],
    ) -> Result<(), std::io::Error> {
        let mut exe_data = fs::read(exe_path)?;

        let zip_start = self.find_zip_start(&exe_data).unwrap();
        let cursor = Cursor::new(&exe_data[zip_start..]);

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

            let _ = zip_writer.start_file::<_, ()>(
                file_name,
                FileOptions::default().compression_method(CompressionMethod::Stored),
            );

            zip_writer.write_all(new_contents)?;

            zip_writer.finish()?;
        }

        exe_data.splice(zip_start.., new_zip);
        fs::write(exe_path, exe_data.clone())?;
        drop(exe_data);
        Ok(())
    }

    fn find_zip_start(&self, exe_data: &[u8]) -> Result<usize, &'static str> {
        let zip_signature: [u8; 4] = [0x50, 0x4b, 0x03, 0x04];
        exe_data
            .windows(4)
            .position(|window| window == zip_signature)
            .ok_or("ZIP start not found")
    }

    // #[allow(dead_code)]
    // fn copy_dir_all(&self, src: impl AsRef<Path>, dst: impl AsRef<Path>) -> std::io::Result<()> {
    //     fs::create_dir_all(&dst)?;
    //     for entry in fs::read_dir(src)? {
    //         let entry = entry?;
    //         let ty = entry.file_type()?;
    //         if ty.is_dir() {
    //             self.copy_dir_all(entry.path(), dst.as_ref().join(entry.file_name()))?;
    //         } else {
    //             fs::copy(entry.path(), dst.as_ref().join(entry.file_name()))?;
    //         }
    //     }
    //     Ok(())
    // }

    pub fn compress_file(&self, input_path: &str, output_path: &str) -> Result<(), std::io::Error> {
        // Open the input file for reading
        let mut input_file = File::open(input_path)?;
        let mut buffer = Vec::new();

        // Read the contents of the input file into a buffer
        input_file.read_to_end(&mut buffer)?;

        // Create a new encoder and pass the input data to it
        let mut encoder = Encoder::new(Vec::new());
        encoder.write_all(&buffer)?;

        // Finish the encoding process and retrieve the compressed data
        let compressed = encoder.finish().into_result()?;

        // Create and write the compressed data into the output file
        let mut output_file = File::create(output_path)?;
        output_file.write_all(&compressed)?;

        Ok(())
    }

    pub fn is_valid(&self) -> bool {
        #[cfg(target_os = "macos")]
        {
            if self
                .path
                .join("Balatro.app/Contents/Resources/Balatro.love")
                .exists()
            {
                return true;
            }

            if is_app_bundle(&self.path)
                && self.path.join("Contents/Resources/Balatro.love").exists()
            {
                return true;
            }

            self.find_balatro_love().is_some()
        }

        #[cfg(target_os = "windows")]
        {
            // For Windows, only check for LÖVE engine DLLs
            let dll_files = ["love.dll", "lua51.dll", "SDL2.dll"];
            let dir = self.path.clone();

            // Return true if at least one of the DLLs exists
            for dll in dll_files.iter() {
                if dir.join(dll).exists() {
                    return true;
                }
            }
            false
        }

        #[cfg(target_os = "linux")]
        {
            // For Linux, keep existing validation
            self.get_exe_path().exists()
        }
    }

    pub fn from_custom_path(path: PathBuf) -> Option<Self> {
        #[cfg(target_os = "macos")]
        let path = normalize_mac_install_path(path)?;

        #[cfg(not(target_os = "macos"))]
        let path = normalize_non_mac_install_path(path);

        let balatro = Balatro { path };
        if balatro.is_valid() {
            Some(balatro)
        } else {
            None
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn normalize_non_mac_install_path(path: PathBuf) -> PathBuf {
    if path.is_file()
        && let Some(parent) = path.parent()
    {
        return parent.to_path_buf();
    }
    path
}

#[cfg(target_os = "macos")]
fn normalize_mac_install_path(mut path: PathBuf) -> Option<PathBuf> {
    if path.is_file() {
        path = path.parent()?.to_path_buf();
    }

    let original = path.clone();
    let mut search_root = path;

    for _ in 0..MAX_BUNDLE_SEARCH_DEPTH {
        if let Some(love_path) = find_balatro_love(&search_root, MAX_BUNDLE_SEARCH_DEPTH) {
            let bundle = love_path
                .ancestors()
                .find(|ancestor| is_app_bundle(ancestor))
                .map(|p| p.to_path_buf())?;

            let mut candidate = if is_app_bundle(&original) {
                bundle.clone()
            } else if bundle
                .parent()
                .map(|parent| parent == original)
                .unwrap_or(false)
            {
                original.clone()
            } else {
                bundle.clone()
            };

            if let Some(parent) = bundle.parent()
                && parent
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n.eq_ignore_ascii_case("Game"))
                    .unwrap_or(false)
            {
                candidate = parent.to_path_buf();
            }

            return Some(candidate);
        }

        if let Some(parent) = search_root.parent() {
            search_root = parent.to_path_buf();
        } else {
            break;
        }
    }

    None
}

#[cfg(target_os = "macos")]
fn is_app_bundle(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("app"))
        .unwrap_or(false)
}

#[cfg(target_os = "macos")]
impl Balatro {
    fn find_balatro_love(&self) -> Option<PathBuf> {
        find_balatro_love(&self.path, MAX_BUNDLE_SEARCH_DEPTH)
    }

    pub fn get_app_bundle_path(&self) -> Option<PathBuf> {
        self.find_balatro_love().and_then(|love| {
            love.ancestors()
                .find(|ancestor| is_app_bundle(ancestor))
                .map(|p| p.to_path_buf())
        })
    }
}

#[cfg(target_os = "macos")]
fn find_balatro_love(path: &Path, depth: usize) -> Option<PathBuf> {
    if depth == 0 {
        return None;
    }

    if path.is_file() {
        return path
            .file_name()
            .and_then(|n| n.to_str())
            .filter(|n| n.eq_ignore_ascii_case("Balatro.love"))
            .map(|_| path.to_path_buf());
    }

    if is_app_bundle(path) {
        let bundle_love = path.join("Contents/Resources/Balatro.love");
        if bundle_love.exists() {
            return Some(bundle_love);
        }

        let nested_game = path.join("Contents/Game");
        if nested_game.exists()
            && let Some(found) = find_balatro_love(&nested_game, depth.saturating_sub(1))
        {
            return Some(found);
        }
    }

    let direct = path.join("Balatro.love");
    if direct.exists() {
        return Some(direct);
    }

    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let child = entry.path();
            if let Some(found) = find_balatro_love(&child, depth.saturating_sub(1)) {
                return Some(found);
            }
        }
    }

    None
}

pub fn find_balatros() -> Vec<Balatro> {
    let paths: Vec<PathBuf> = get_balatro_paths();
    let mut balatros = Vec::new();
    for path in paths {
        if let Some(balatro) = Balatro::from_custom_path(path) {
            balatros.push(balatro);
        }
    }
    balatros
}

pub fn get_save_dir(linux_native: bool) -> PathBuf {
    let mut save_dir = String::new();
    if cfg!(target_os = "macos") {
        let home_dir = format!("/Users/{}", std::env::var("USER").unwrap());
        save_dir = format!("{home_dir}/Library/Application Support/Balatro");
    } else if cfg!(target_os = "windows") {
        let appdata = std::env::var("APPDATA").unwrap();
        save_dir = format!("{appdata}/Balatro");
    } else if cfg!(target_os = "linux") {
        if linux_native {
            let home = std::env::var("HOME").unwrap();
            save_dir = format!("{home}/.local/share/love/Balatro");
        } else {
            let home = std::env::var("HOME").unwrap();
            save_dir = format!(
                "{home}/.local/share/Steam/steamapps/compatdata/2379780/pfx/drive_c/users/steamuser/AppData/Roaming/Balatro"
            );
        }
    }

    if save_dir.is_empty() {
        panic!("Unsupported OS");
    }

    PathBuf::from(save_dir)
}
