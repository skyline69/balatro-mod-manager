use rusqlite::Connection;
use serde::Serialize;
use std::path::PathBuf;
use crate::errors::AppError;

pub struct Database {
    conn: Connection,
}

#[derive(Serialize)]
pub struct InstalledMod {
    pub name: String,
    pub path: String,
}

impl Database {
    pub fn new() -> Result<Self, AppError> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| AppError::DirNotFound(PathBuf::from("config directory")))?;
        let storage_path = config_dir.join("Balatro").join("bmm_storage.db");

        let db_exists = storage_path.exists();
        let conn = Connection::open(&storage_path)
            .map_err(|e| AppError::DatabaseInit(e.to_string()))?;

        if !db_exists {
            Self::initialize_database(&conn)?;
        }

        Ok(Database { conn })
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
                path TEXT NOT NULL
            )",
            [],
        )
        .map_err(|e| AppError::DatabaseInit(e.to_string()))?;

        Ok(())
    }

    pub fn get_installed_mods(&self) -> Result<Vec<InstalledMod>, AppError> {
        let mut stmt = self.conn.prepare("SELECT name, path FROM installed_mods")?;
        let mut mods = Vec::new();
        let mut rows = stmt.query([])?;

        while let Some(row) = rows.next()? {
            mods.push(InstalledMod {
                name: row.get(0)?,
                path: row.get(1)?,
            });
        }

        Ok(mods)
    }

    pub fn add_installed_mod(&self, name: &str, path: &str) -> Result<(), AppError> {
        self.conn.execute(
            "INSERT OR REPLACE INTO installed_mods (name, path) VALUES (?1, ?2)",
            [name, path],
        )?;
        Ok(())
    }

    pub fn remove_installed_mod(&self, name: &str) -> Result<(), AppError> {
        self.conn.execute("DELETE FROM installed_mods WHERE name = ?1", [name])?;
        Ok(())
    }

    pub fn get_installation_path(&self) -> Result<Option<String>, AppError> {
        let mut stmt = self.conn.prepare(
            "SELECT value FROM settings WHERE setting = 'installation_path'",
        )?;
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

    // fn get_setting(&self, setting: &str) -> Result<Option<String>, AppError> {
    //     let mut stmt = self.conn.prepare(
    //         "SELECT value FROM settings WHERE setting = ?1",
    //     )?;
    //     let mut rows = stmt.query([setting])?;
    //
    //     if let Some(row) = rows.next()? {
    //         Ok(Some(row.get(0)?))
    //     } else {
    //         Ok(None)
    //     }
    // }
    //
    // fn delete_setting(&self, setting: &str) -> Result<(), AppError> {
    //     self.conn.execute(
    //         "DELETE FROM settings WHERE setting = ?1",
    //         [setting],
    //     )?;
    //     Ok(())
    // }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn create_memory_db() -> Result<Database, AppError> {
        let conn = Connection::open_in_memory()
            .map_err(|e| AppError::DatabaseInit(e.to_string()))?;
        Database::initialize_database(&conn)?;
        Ok(Database { conn })
    }

    #[test]
    fn test_installed_mods_crud() -> Result<(), AppError> {
        let db = create_memory_db()?;
        
        db.add_installed_mod("TestMod", "/path/to/mod")?;
        let mods = db.get_installed_mods()?;
        assert_eq!(mods.len(), 1);
        assert_eq!(mods[0].name, "TestMod");

        db.remove_installed_mod("TestMod")?;
        assert!(db.get_installed_mods()?.is_empty());

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
}

