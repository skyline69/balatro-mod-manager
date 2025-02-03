// tests/helpers.rs
use tempfile::tempdir;

pub struct TestEnv {
    pub temp_dir: tempfile::TempDir,
}

impl TestEnv {
    pub fn new() -> Self {
        let temp_dir = match tempdir() {
            Ok(dir) => dir,
            Err(e) => panic!("Failed to create temporary directory: {}", e),
        }
        // Setup environment variables
        std::env::set_var("BMM_TEST_MODE", "1");
        std::env::set_var("BMM_TEST_DIR", match temp_dir.path().to_str() {
            Some(path) => path,
            None => panic!("Failed to convert temporary directory path to string"),
        });
        Self { temp_dir }
    }
}

impl Drop for TestEnv {
    fn drop(&mut self) {
        std::env::remove_var("BMM_TEST_MODE");
        std::env::remove_var("BMM_TEST_DIR");
    }
}
