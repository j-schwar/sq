use std::path::Path;

use rusqlite::Connection;

use crate::db::Database;

struct Sqlite {
    #[allow(dead_code)]
    conn: Connection,
}

impl Database for Sqlite {
    fn schema(&self) -> anyhow::Result<crate::schema::Schema> {
        todo!()
    }
}

/// Connects to a SQLite database.
pub fn connect(file: &Path) -> anyhow::Result<Box<dyn Database>> {
    let conn = Connection::open(file)?;
    Ok(Box::new(Sqlite { conn }))
}
