// use crate::mod_collections::ModCollectionManager;
use rusqlite::{Connection, Result};
use serde::Serialize;

pub struct Database {
    conn: Connection,
    // pub mod_manager: ModCollectionManager,
}

#[derive(Serialize)]
pub struct InstalledMod {
    pub name: String,
    pub path: String,
    // pub collection_hash: Option<String>,
}

impl Database {
    pub fn new() -> Result<Self> {
        let storage_path = dirs::config_dir()
            .expect("Could not find config directory")
            .join("Balatro")
            .join("bmm_storage.db");

        let db_exists = storage_path.exists();
        let conn = Connection::open(storage_path)?;

        if !db_exists {
            Self::initialize_database(&conn)?;
        }

        Ok(Database {
            conn,
            // mod_manager: ModCollectionManager::new(),
        })
    }

    fn initialize_database(conn: &Connection) -> Result<()> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS settings (
                setting TEXT PRIMARY KEY,
                value TEXT NOT NULL
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS installed_mods (
                name TEXT PRIMARY KEY,
                path TEXT NOT NULL
            )",
            [],
        )?;
        // collection_hash TEXT

        // ModCollectionManager::initialize_table(conn)?;

        Ok(())
    }

    pub fn get_installed_mods(&self) -> Result<Vec<InstalledMod>> {
        let mut stmt = self.conn.prepare("SELECT * FROM installed_mods")?;
        let mut mods: Vec<InstalledMod> = Vec::new();
        let mut rows = stmt.query([])?;

        while let Some(row) = rows.next()? {
            mods.push(InstalledMod {
                name: row.get(0)?,
                path: row.get(1)?,
                // collection_hash: row.get(2)?,
            });
        }

        Ok(mods)
    }

    pub fn add_installed_mod(&self, name: &str, path: &str) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO installed_mods (name, path) VALUES (?1, ?2)",
            [name, path],
        )?;
        Ok(())
    }

    // pub fn add_mod_to_collection(&self, name: &str, collection_hash: &str) -> Result<()> {
    //     self.conn.execute(
    //         "UPDATE installed_mods SET collection_hash = ?1 WHERE name = ?2",
    //         [collection_hash, name],
    //     )?;
    //     Ok(())
    // }
    //
    pub fn remove_installed_mod(&self, name: &str) -> Result<()> {
        self.conn
            .execute("DELETE FROM installed_mods WHERE name = ?1", [name])?;
        Ok(())
    }

    pub fn get_connection(&self) -> &Connection {
        &self.conn
    }

    pub fn get_installation_path(&self) -> Result<Option<String>> {
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

    pub fn remove_installation_path(&self) -> Result<()> {
        self.delete_setting("installation_path")?;
        Ok(())
    }

    pub fn set_installation_path(&self, path: &str) -> Result<()> {
        self.set_setting("installation_path", path)?;
        Ok(())
    }

    pub fn get_setting(&self, setting: &str) -> Result<Option<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT value FROM settings WHERE setting = ?1")?;
        let mut rows = stmt.query([setting])?;

        if let Some(row) = rows.next()? {
            Ok(Some(row.get(0)?))
        } else {
            Ok(None)
        }
    }

    pub fn set_setting(&self, setting: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO settings (setting, value) VALUES (?1, ?2)",
            [setting, value],
        )?;
        Ok(())
    }

    pub fn delete_setting(&self, setting: &str) -> Result<()> {
        self.conn
            .execute("DELETE FROM settings WHERE setting = ?1", [setting])?;
        Ok(())
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn create_memory_db() -> Result<Database> {
        let conn = Connection::open_in_memory()?;
        Database::initialize_database(&conn)?;
        Ok(Database { conn })
    }

    #[test]
    fn test_installed_mods_crud() -> Result<()> {
        let db = create_memory_db()?;
        
        // Test create
        db.add_installed_mod("TestMod", "/path/to/mod")?;
        let mods = db.get_installed_mods()?;
        assert_eq!(mods.len(), 1);
        assert_eq!(mods[0].name, "TestMod");

        // Test delete
        db.remove_installed_mod("TestMod")?;
        assert!(db.get_installed_mods()?.is_empty());

        Ok(())
    }

    #[test]
    fn test_installation_path_management() -> Result<()> {
        let db = create_memory_db()?;
        
        assert!(db.get_installation_path()?.is_none());
        db.set_installation_path("/games/balatro")?;
        assert_eq!(db.get_installation_path()?, Some("/games/balatro".into()));
        
        db.remove_installation_path()?;
        assert!(db.get_installation_path()?.is_none());

        Ok(())
    }
}
