// use crate::cache::Mod;
use crate::errors::AppError;
use rusqlite::Connection;
use serde::Serialize;
use std::path::PathBuf;

pub struct Database {
    conn: Connection,
}

#[derive(Serialize)]
pub struct InstalledMod {
    pub name: String,
    pub path: String,
    pub dependencies: Vec<String>,
    pub current_version: Option<String>,
}

impl Database {
    const CURRENT_DB_VERSION: &'static str = "1.0"; // Update this when schema changes

    pub fn new() -> Result<Self, AppError> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| AppError::DirNotFound(PathBuf::from("config directory")))?;
        let balatro_dir = config_dir.join("Balatro");

        // Create directory if it doesn't exist
        if !balatro_dir.exists() {
            std::fs::create_dir_all(&balatro_dir).map_err(|e| {
                AppError::DirNotFound(format!("Failed to create config directory: {}", e).into())
            })?
        };

        // Create the Balatro config directory if it doesn't exist
        let balatro_dir = config_dir.join("Balatro");
        std::fs::create_dir_all(&balatro_dir).map_err(|e| AppError::DirCreate {
            path: balatro_dir.clone(),
            source: e.to_string(),
        })?;

        let storage_path = balatro_dir.join("bmm_storage.db");
        // Now check if the database exists and open it
        let db_exists = storage_path.exists();
        let conn =
            Connection::open(&storage_path).map_err(|e| AppError::DatabaseInit(e.to_string()))?;

        if !db_exists {
            Self::initialize_database(&conn)?;
        }

        // Try to open the database with a retry mechanism
        let mut retry_count = 0;
        let max_retries = 3;

        while retry_count < max_retries {
            // Try to open or create the database
            let conn_result = if storage_path.exists() {
                Connection::open(&storage_path)
            } else {
                // Create a new database
                let conn = Connection::open(&storage_path)
                    .map_err(|e| AppError::DatabaseInit(e.to_string()))?;
                Self::initialize_database(&conn)?;
                Ok(conn)
            };

            match conn_result {
                Ok(conn) => {
                    // Check if database needs migration
                    if Self::needs_migration(&conn)? {
                        // Close the connection before migration
                        drop(conn);

                        // Perform migration
                        Self::migrate_database(&storage_path)?;

                        // Reopen the database after migration
                        let conn = Connection::open(&storage_path)
                            .map_err(|e| AppError::DatabaseInit(e.to_string()))?;
                        return Ok(Database { conn });
                    }

                    return Ok(Database { conn });
                }
                Err(e) => {
                    if retry_count == max_retries - 1 {
                        return Err(AppError::DatabaseInit(format!(
                            "Failed to open database after {} attempts: {}",
                            max_retries, e
                        )));
                    }

                    log::warn!(
                        "Failed to open database, retrying ({}/{}): {}",
                        retry_count + 1,
                        max_retries,
                        e
                    );
                    std::thread::sleep(std::time::Duration::from_millis(500));
                    retry_count += 1;
                }
            }
        }

        Err(AppError::DatabaseInit(
            "Failed to open database after maximum retries".to_string(),
        ))
    }

    // Check if database needs migration
    fn needs_migration(conn: &Connection) -> Result<bool, AppError> {
        // First check if the version table exists
        let has_version_setting = match conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='settings'",
            [],
            |row| row.get::<_, i64>(0),
        ) {
            Ok(count) if count > 0 => {
                // Now check if the version setting exists
                match conn.query_row(
                    "SELECT COUNT(*) FROM settings WHERE setting = 'db_version'",
                    [],
                    |row| row.get::<_, i64>(0),
                ) {
                    Ok(count) => count > 0,
                    Err(_) => false,
                }
            }
            _ => false,
        };

        // If no version in settings, this is an old database needing migration
        if !has_version_setting {
            return Ok(true);
        }

        // Check if the version matches
        match conn.query_row(
            "SELECT value FROM settings WHERE setting = 'db_version'",
            [],
            |row| row.get::<_, String>(0),
        ) {
            Ok(version) => Ok(version != Self::CURRENT_DB_VERSION),
            Err(_) => Ok(true), // If we can't get the version, assume migration is needed
        }
    }

    fn migrate_database(db_path: &PathBuf) -> Result<(), AppError> {
        // Create a temporary database path
        let temp_db_path = db_path.with_file_name("bmm_storage_new.db");

        // If the old database exists but we can't access it, try with a retry mechanism
        let max_retries = 3;
        let mut retry_count = 0;

        while retry_count < max_retries {
            // Open connections to both databases
            let old_conn_result = Connection::open(db_path);

            if let Err(e) = old_conn_result {
                if retry_count == max_retries - 1 {
                    return Err(AppError::DatabaseInit(format!(
                        "Failed to open old database after {} retries: {}",
                        max_retries, e
                    )));
                }

                // Wait before retrying
                std::thread::sleep(std::time::Duration::from_millis(500));
                retry_count += 1;
                continue;
            }

            let old_conn = old_conn_result.unwrap();
            let new_conn = Connection::open(&temp_db_path).map_err(|e| {
                AppError::DatabaseInit(format!("Failed to create new database: {}", e))
            })?;

            // Initialize the new database with current schema
            Self::initialize_database(&new_conn)?;

            // Migrate data
            Self::migrate_settings(&old_conn, &new_conn)?;
            Self::migrate_installed_mods(&old_conn, &new_conn)?;

            // IMPORTANT: Explicitly close connections before file operations
            drop(old_conn);
            drop(new_conn);

            // Add a small delay to ensure all handles are released
            std::thread::sleep(std::time::Duration::from_millis(100));

            // Try to backup the old database
            let backup_path = db_path.with_extension("db.bak");

            // If backup fails, just log a warning but continue
            match std::fs::rename(db_path, &backup_path) {
                Ok(_) => {}
                Err(e) => {
                    log::warn!("Failed to backup old database, continuing anyway: {}", e);
                    // Try to directly remove the old file if we can't rename it
                    if let Err(e) = std::fs::remove_file(db_path) {
                        log::warn!("Failed to remove old database: {}", e);
                    }
                }
            }

            // Replace with the new one
            match std::fs::rename(&temp_db_path, db_path) {
                Ok(_) => return Ok(()),
                Err(e) => {
                    return Err(AppError::DatabaseInit(format!(
                        "Failed to install new database: {}",
                        e
                    )));
                }
            }
        }

        Err(AppError::DatabaseInit(
            "Failed to access database after maximum retries".to_string(),
        ))
    }

    fn migrate_settings(old_conn: &Connection, new_conn: &Connection) -> Result<(), AppError> {
        // Check if settings table exists in old database
        let has_settings = match old_conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='settings'",
            [],
            |row| row.get::<_, i64>(0),
        ) {
            Ok(count) => count > 0,
            Err(_) => false,
        };

        if !has_settings {
            return Ok(()); // No settings to migrate
        }

        // Get all settings except db_version (which will be set by initialize_database)
        let mut stmt = match old_conn
            .prepare("SELECT setting, value FROM settings WHERE setting != 'db_version'")
        {
            Ok(stmt) => stmt,
            Err(_) => return Ok(()), // If query fails, just continue
        };

        for (setting, value) in stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .flatten()
        {
            new_conn.execute(
                "INSERT OR REPLACE INTO settings (setting, value) VALUES (?1, ?2)",
                [&setting, &value],
            )?;
        }

        Ok(())
    }
    // Migrate installed mods from old database to new one
    fn migrate_installed_mods(
        old_conn: &Connection,
        new_conn: &Connection,
    ) -> Result<(), AppError> {
        // Check if installed_mods table exists in old database
        let has_installed_mods = match old_conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='installed_mods'",
            [],
            |row| row.get::<_, i64>(0),
        ) {
            Ok(count) => count > 0,
            Err(_) => false,
        };

        if !has_installed_mods {
            return Ok(()); // No mods to migrate
        }

        // Get all installed mods
        let mut stmt = match old_conn
            .prepare("SELECT name, path, dependencies, current_version FROM installed_mods")
        {
            Ok(stmt) => stmt,
            Err(_) => return Ok(()), // If query fails, just continue
        };

        for (name, path, dependencies, current_version) in stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                ))
            })?
            .flatten()
        {
            new_conn.execute(
            "INSERT INTO installed_mods (name, path, dependencies, current_version) VALUES (?1, ?2, ?3, ?4)",
            [&name, &path, &dependencies, &current_version.unwrap_or_default()],
        )?;
        }

        Ok(())
    }

    fn initialize_database(conn: &Connection) -> Result<(), AppError> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS settings (
                setting TEXT PRIMARY KEY,
                value TEXT NOT NULL
            )",
            [],
        )
        .map_err(|e| AppError::DatabaseInit(e.to_string()))?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS installed_mods (
                name TEXT PRIMARY KEY,
                path TEXT NOT NULL,
                dependencies TEXT NOT NULL DEFAULT '[]',
                current_version TEXT
            )",
            [],
        )
        .map_err(|e| AppError::DatabaseInit(e.to_string()))?;

        // Set the database version
        conn.execute(
            "INSERT OR REPLACE INTO settings (setting, value) VALUES ('db_version', ?1)",
            [Self::CURRENT_DB_VERSION],
        )
        .map_err(|e| AppError::DatabaseInit(e.to_string()))?;

        Ok(())
    }

    pub fn get_mod_details(&self, mod_name: &str) -> Result<InstalledMod, AppError> {
        let mut stmt = self.conn.prepare(
            "SELECT name, path, dependencies, current_version FROM installed_mods WHERE name = ?1",
        )?;

        let mut rows = stmt.query([mod_name])?;

        if let Some(row) = rows.next()? {
            Ok(InstalledMod {
                name: row.get(0)?,
                path: row.get(1)?,
                dependencies: serde_json::from_str(&row.get::<_, String>(2)?)?,
                current_version: row.get(3)?,
            })
        } else {
            Err(AppError::InvalidState(format!(
                "Mod {} not found",
                mod_name
            )))
        }
    }

    pub fn set_discord_rpc_enabled(&self, enabled: bool) -> Result<(), AppError> {
        let value = if enabled { "enabled" } else { "disabled" };
        self.conn.execute(
            "INSERT OR REPLACE INTO settings (setting, value) VALUES ('discord_rpc', ?1)",
            [value],
        )?;
        Ok(())
    }

    pub fn is_discord_rpc_enabled(&self) -> Result<bool, AppError> {
        let mut stmt = self
            .conn
            .prepare("SELECT value FROM settings WHERE setting = 'discord_rpc'")?;
        let mut rows = stmt.query([])?;

        if let Some(row) = rows.next()? {
            Ok(row.get::<_, String>(0)? == "enabled")
        } else {
            // Default to enabled if setting doesn't exist yet
            self.set_discord_rpc_enabled(true)?;
            Ok(true)
        }
    }

    pub fn set_last_fetched(&self, timestamp: u64) -> Result<(), AppError> {
        self.conn.execute(
            "INSERT OR REPLACE INTO settings (setting, value) VALUES ('last_fetched', ?1)",
            [timestamp.to_string()],
        )?;
        Ok(())
    }

    pub fn get_last_fetched(&self) -> Result<u64, AppError> {
        let mut stmt = self
            .conn
            .prepare("SELECT value FROM settings WHERE setting = 'last_fetched'")?;

        let mut rows = stmt.query([])?;
        if let Some(row) = rows.next()? {
            let val: String = row.get(0)?;
            val.parse()
                .map_err(|_| AppError::InvalidState("Invalid timestamp format".to_string()))
        } else {
            Ok(0)
        }
    }

    pub fn get_installed_mods(&self) -> Result<Vec<InstalledMod>, AppError> {
        let mut stmt = self
            .conn
            .prepare("SELECT name, path, dependencies, current_version FROM installed_mods")?;
        let mut mods = Vec::new();
        let mut rows = stmt.query([])?;

        while let Some(row) = rows.next()? {
            mods.push(InstalledMod {
                name: row.get(0)?,
                path: row.get(1)?,
                dependencies: serde_json::from_str(&row.get::<_, String>(2)?)?,
                current_version: row.get(3)?,
            });
        }

        Ok(mods)
    }

    pub fn add_installed_mod(
        &self,
        name: &str,
        path: &str,
        dependencies: &[String],
        current_version: Option<String>,
    ) -> Result<(), AppError> {
        let deps_json = serde_json::to_string(dependencies)?;
        self.conn.execute(
            "INSERT OR REPLACE INTO installed_mods (name, path, dependencies, current_version) VALUES (?1, ?2, ?3, ?4)",
            [name, path, &deps_json, &current_version.unwrap_or_default()],
        )?;
        Ok(())
    }

    pub fn get_dependents(&self, mod_name: &str) -> Result<Vec<String>, AppError> {
        let mut stmt = self.conn.prepare(
            "SELECT name FROM installed_mods
            WHERE EXISTS (
                SELECT 1 FROM json_each(dependencies)
                WHERE TRIM(json_each.value, '\"') = ?1
            )",
        )?;

        let mut rows = stmt.query([mod_name])?;
        let mut dependents = Vec::new();

        while let Some(row) = rows.next()? {
            dependents.push(row.get(0)?);
        }

        Ok(dependents)
    }

    pub fn remove_installed_mod(&self, name: &str) -> Result<(), AppError> {
        self.conn
            .execute("DELETE FROM installed_mods WHERE name = ?1", [name])?;
        Ok(())
    }

    pub fn get_installation_path(&self) -> Result<Option<String>, AppError> {
        let mut stmt = self
            .conn
            .prepare("SELECT value FROM settings WHERE setting = 'installation_path'")?;
        let mut rows = stmt.query([])?;

        if let Some(row) = rows.next()? {
            Ok(Some(row.get(0)?))
        } else {
            Ok(None)
        }
    }

    pub fn set_installation_path(&self, path: &str) -> Result<(), AppError> {
        self.conn.execute(
            "INSERT OR REPLACE INTO settings (setting, value) VALUES (?1, ?2)",
            ["installation_path", path],
        )?;
        Ok(())
    }

    pub fn remove_installation_path(&self) -> Result<(), AppError> {
        self.conn.execute(
            "DELETE FROM settings WHERE setting = 'installation_path'",
            [],
        )?;
        Ok(())
    }

    pub fn get_last_installed_version(&self, mod_name: &str) -> Result<String, AppError> {
        let mut stmt = self
            .conn
            .prepare("SELECT current_version FROM installed_mods WHERE name = ?1")?;
        let mut rows = stmt.query([mod_name])?;

        if let Some(row) = rows.next()? {
            Ok(row.get(0)?)
        } else {
            Ok(String::new())
        }
    }

    pub fn set_last_installed_version(
        &self,
        mod_name: &str,
        version: &str,
    ) -> Result<(), AppError> {
        self.conn.execute(
            "UPDATE installed_mods SET current_version = ?1 WHERE name = ?2",
            [version, mod_name],
        )?;
        Ok(())
    }

    pub fn set_background_enabled(&self, enabled: bool) -> Result<(), AppError> {
        let enabled: &str = if enabled { "enabled" } else { "disabled" };
        self.conn.execute(
            "INSERT OR REPLACE INTO settings (setting, value) VALUES ('background_enabled', ?1)",
            [enabled],
        )?;
        Ok(())
    }

    pub fn get_background_enabled(&self) -> Result<bool, AppError> {
        let mut stmt = self
            .conn
            .prepare("SELECT value FROM settings WHERE setting = 'background_enabled'")?;
        let mut rows = stmt.query([])?;

        if let Some(row) = rows.next()? {
            Ok(row.get::<_, String>(0)? == "enabled")
        } else {
            Ok(false)
        }
    }

    fn enable_lovely_console(&self) -> Result<(), AppError> {
        self.conn.execute(
            "INSERT OR REPLACE INTO settings (setting, value) VALUES ('lovely_console', 'enabled')",
            [],
        )?;
        Ok(())
    }

    fn disable_lovely_console(&self) -> Result<(), AppError> {
        self.conn.execute(
            "INSERT OR REPLACE INTO settings (setting, value) VALUES ('lovely_console', 'disabled')",
            [],
        )?;
        Ok(())
    }

    pub fn set_lovely_console_status(&self, enabled: bool) -> Result<(), AppError> {
        if enabled {
            self.enable_lovely_console()
        } else {
            self.disable_lovely_console()
        }
    }

    pub fn is_lovely_console_enabled(&self) -> Result<bool, AppError> {
        let mut stmt = self
            .conn
            .prepare("SELECT value FROM settings WHERE setting = 'lovely_console'")?;
        let mut rows = stmt.query([])?;

        if let Some(row) = rows.next()? {
            Ok(row.get::<_, String>(0)? == "enabled")
        } else {
            Ok(false)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn create_memory_db() -> Result<Database, AppError> {
        let conn =
            Connection::open_in_memory().map_err(|e| AppError::DatabaseInit(e.to_string()))?;
        Database::initialize_database(&conn)?;
        Ok(Database { conn })
    }

    #[test]

    fn test_installed_mods_crud() -> Result<(), AppError> {
        let db = create_memory_db()?;

        // Add with empty dependencies
        db.add_installed_mod("TestMod", "/path/to/mod", &[], None)?;
        let mods = db.get_installed_mods()?;
        assert_eq!(mods.len(), 1);
        assert_eq!(mods[0].name, "TestMod");
        assert!(mods[0].dependencies.is_empty()); // Verify dependencies

        // Add with dependencies
        let deps = vec!["Steamodded".to_string()];
        db.add_installed_mod("DependentMod", "/another/path", &deps, None)?;
        let mods = db.get_installed_mods()?;
        assert_eq!(mods[1].dependencies, deps);

        db.remove_installed_mod("TestMod")?;
        assert_eq!(db.get_installed_mods()?.len(), 1);

        Ok(())
    }

    #[test]
    fn test_installation_path_management() -> Result<(), AppError> {
        let db = create_memory_db()?;

        assert!(db.get_installation_path()?.is_none());
        db.set_installation_path("/games/balatro")?;
        assert_eq!(db.get_installation_path()?, Some("/games/balatro".into()));

        db.remove_installation_path()?;
        assert!(db.get_installation_path()?.is_none());

        Ok(())
    }

    #[test]
    fn test_mod_details() -> Result<(), AppError> {
        let db = create_memory_db()?;
        let deps = vec!["Steamodded".to_string()];

        db.add_installed_mod("TestMod", "/path/to/mod", &deps, None)?;

        let details = db.get_mod_details("TestMod")?;
        assert_eq!(details.name, "TestMod");
        assert_eq!(details.path, "/path/to/mod");
        assert_eq!(details.dependencies, deps);

        Ok(())
    }
}
