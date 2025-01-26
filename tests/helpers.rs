// tests/helpers.rs
use tempfile::tempdir;

pub struct TestEnv {
    pub temp_dir: tempfile::TempDir,
}

impl TestEnv {
    pub fn new() -> Self {
        let temp_dir = tempdir().unwrap();
        // Setup environment variables
        std::env::set_var("BMM_TEST_MODE", "1");
        std::env::set_var("BMM_TEST_DIR", temp_dir.path().to_str().unwrap());
        Self { temp_dir }
    }
}

impl Drop for TestEnv {
    fn drop(&mut self) {
        std::env::remove_var("BMM_TEST_MODE");
        std::env::remove_var("BMM_TEST_DIR");
    }
}
