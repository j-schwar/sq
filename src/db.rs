use serde::{Deserialize, Serialize};

use crate::schema::Schema;

mod mssql;

/// Configuration options for SQL drivers.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum DriverConfig {
    #[serde(rename_all = "camelCase")]
    Odbc {
        /// Connection string for the ODBC driver.
        connection_string: String,
    },
}

/// Trait for interacting with a database.
pub(crate) trait Database {
    /// Fetches the schema from this database.
    fn schema(&self) -> anyhow::Result<Schema>;
}

/// Connects to a database using the provided configuration.
///
/// # Panics
///
/// This function may panic if certain initialization operations fail. For example, if connecting
/// to an ODBC database, this function will panic if the ODBC environment cannot be initialized.
pub(crate) fn connect(config: &DriverConfig) -> anyhow::Result<Box<dyn Database>> {
    match config {
        DriverConfig::Odbc { connection_string } => mssql::connect(connection_string),
    }
}
