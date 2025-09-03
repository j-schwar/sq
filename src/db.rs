use std::sync::OnceLock;

use odbc_api::{Connection, ConnectionOptions, Environment};

use crate::{config::DriverConfig, schema::Schema};

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
        DriverConfig::Odbc { connection_string } => {
            let env = ODBC_ENV
                .get_or_init(|| Environment::new().expect("Failed to create ODBC environment"));

            let options = ConnectionOptions::default();
            let connection = env.connect_with_connection_string(connection_string, options)?;
            Ok(Box::new(MsSql { connection }))
        }
    }
}

static ODBC_ENV: OnceLock<Environment> = OnceLock::new();

struct MsSql {
    connection: Connection<'static>,
}

impl Database for MsSql {
    fn schema(&self) -> anyhow::Result<Schema> {
        todo!()
    }
}
