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
}

impl Database {
    pub fn new() -> Result<Self, AppError> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| AppError::DirNotFound(PathBuf::from("config directory")))?;
        let storage_path = config_dir.join("Balatro").join("bmm_storage.db");

        let db_exists = storage_path.exists();
        let conn =
            Connection::open(&storage_path).map_err(|e| AppError::DatabaseInit(e.to_string()))?;

        if !db_exists {
            Self::initialize_database(&conn)?;
        }

        Ok(Database { conn })
    }

    pub fn get_mod_details(&self, mod_name: &str) -> Result<InstalledMod, AppError> {
        let mut stmt = self
            .conn
            .prepare("SELECT name, path, dependencies FROM installed_mods WHERE name = ?1")?;

        let mut rows = stmt.query([mod_name])?;

        if let Some(row) = rows.next()? {
            Ok(InstalledMod {
                name: row.get(0)?,
                path: row.get(1)?,
                dependencies: serde_json::from_str(&row.get::<_, String>(2)?)?,
            })
        } else {
            Err(AppError::InvalidState(format!(
                "Mod {} not found",
                mod_name
            )))
        }
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
                dependencies TEXT NOT NULL DEFAULT '[]'
            )",
            [],
        )
        .map_err(|e| AppError::DatabaseInit(e.to_string()))?;

        Ok(())
    }

    pub fn get_installed_mods(&self) -> Result<Vec<InstalledMod>, AppError> {
        let mut stmt = self
            .conn
            .prepare("SELECT name, path, dependencies FROM installed_mods")?;
        let mut mods = Vec::new();
        let mut rows = stmt.query([])?;

        while let Some(row) = rows.next()? {
            mods.push(InstalledMod {
                name: row.get(0)?,
                path: row.get(1)?,
                dependencies: serde_json::from_str(&row.get::<_, String>(2)?)?,
            });
        }

        Ok(mods)
    }

    pub fn add_installed_mod(
        &self,
        name: &str,
        path: &str,
        dependencies: &[String],
    ) -> Result<(), AppError> {
        let deps_json = serde_json::to_string(dependencies)?;
        self.conn.execute(
            "INSERT OR REPLACE INTO installed_mods (name, path, dependencies) VALUES (?1, ?2, ?3)",
            [name, path, &deps_json],
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
        db.add_installed_mod("TestMod", "/path/to/mod", &[])?;
        let mods = db.get_installed_mods()?;
        assert_eq!(mods.len(), 1);
        assert_eq!(mods[0].name, "TestMod");
        assert!(mods[0].dependencies.is_empty()); // Verify dependencies

        // Add with dependencies
        let deps = vec!["Steamodded".to_string()];
        db.add_installed_mod("DependentMod", "/another/path", &deps)?;
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

        db.add_installed_mod("TestMod", "/path/to/mod", &deps)?;

        let details = db.get_mod_details("TestMod")?;
        assert_eq!(details.name, "TestMod");
        assert_eq!(details.path, "/path/to/mod");
        assert_eq!(details.dependencies, deps);

        Ok(())
    }
}
