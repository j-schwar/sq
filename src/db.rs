use std::sync::OnceLock;

use odbc_api::{
    Connection, ConnectionOptions, Cursor, Environment, ParameterCollectionRef, buffers::TextRowSet,
};

use crate::{
    config::DriverConfig,
    schema::{Column, DataType, Object, Schema},
};

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
const ODBC_BATCH_SIZE: usize = 5000;
const ODBC_QUERY_TIMEOUT: usize = 10;

struct MsSql {
    connection: Connection<'static>,
}

impl MsSql {
    fn exec_query<F, T>(
        &self,
        query: &str,
        params: impl ParameterCollectionRef,
        mut f: F,
    ) -> anyhow::Result<Vec<T>>
    where
        F: FnMut(&[&[u8]]) -> T,
    {
        match self
            .connection
            .execute(query, params, Some(ODBC_QUERY_TIMEOUT))?
        {
            Some(mut cursor) => {
                let mut buffers = TextRowSet::for_cursor(ODBC_BATCH_SIZE, &mut cursor, Some(4096))?;
                let mut row_set_cursor = cursor.bind_buffer(&mut buffers)?;

                let mut obj_buffer = Vec::new();
                while let Some(batch) = row_set_cursor.fetch()? {
                    for row_index in 0..batch.num_rows() {
                        let record: Vec<&[u8]> = (0..batch.num_cols())
                            .map(|col_index| batch.at(col_index, row_index).unwrap_or(&[]))
                            .collect();

                        let obj = f(record.as_slice());
                        obj_buffer.push(obj);
                    }
                }

                Ok(obj_buffer)
            }

            None => Ok(Vec::new()),
        }
    }
}

impl Database for MsSql {
    #[tracing::instrument(skip_all, err)]
    fn schema(&self) -> anyhow::Result<Schema> {
        tracing::info!("Fetching SQL Server database schema");
        let mut schema = Schema::default();

        // Fetch information from [INFORMATION_SCHEMA] views.
        const QUERY: &str = r#"
            SELECT
                [T].[TABLE_NAME],
                [T].[TABLE_TYPE],
                [C].[COLUMN_NAME],
                [C].[IS_NULLABLE],
                [C].[DATA_TYPE]
            FROM
                [INFORMATION_SCHEMA].[TABLES] AS [T]
                JOIN [INFORMATION_SCHEMA].[COLUMNS] AS [C] ON [T].[TABLE_NAME] = [C].[TABLE_NAME]
            ORDER BY
                [T].[TABLE_NAME], [C].[ORDINAL_POSITION]
        "#;

        let mut active_table = None;
        let mut active_table_type = None;
        let mut columns = Vec::new();
        self.exec_query(QUERY, (), |r| {
            let table_name = std::str::from_utf8(r[0]).unwrap_or_default();
            let table_type = std::str::from_utf8(r[1]).unwrap_or_default();

            if active_table.is_none() {
                active_table = Some(table_name.to_string());
                active_table_type = Some(table_type.to_string());
            } else if Some(table_name) != active_table.as_deref() {
                // Store the previous table.
                schema
                    .objects
                    .insert_with_key(|id| match active_table_type.as_deref() {
                        Some("BASE TABLE") => Object::Table {
                            id,
                            score: None,
                            name: active_table.clone().unwrap(),
                            columns: columns.drain(..).collect(),
                            foreign_keys: Vec::new(),
                        },

                        Some("VIEW") => Object::View {
                            id,
                            score: None,
                            name: active_table.clone().unwrap(),
                            columns: columns.drain(..).collect(),
                        },
                        _ => panic!("Unknown table type: {:?}", active_table_type),
                    });
                tracing::debug!("Found table: {}", active_table.as_deref().unwrap());

                // Start a new table.
                active_table = Some(table_name.to_string());
                active_table_type = Some(table_type.to_string());
            }

            let column_name = std::str::from_utf8(r[2]).unwrap_or_default();
            let nullable = match r[3] {
                b"YES" => true,
                _ => false,
            };
            let data_type = match r[4] {
                b"int" => DataType::Integer,
                _ => DataType::Unknown,
            };

            let column_id = schema.columns.insert_with_key(|id| Column {
                id,
                score: None,
                name: column_name.to_string(),
                data_type,
                nullable,
            });

            tracing::debug!("Found column: {}", column_name);
            columns.push(column_id);
        })?;

        if let (Some(active_table), Some(active_table_type)) = (active_table, active_table_type) {
            tracing::debug!("Found table: {}", active_table);
            // Store the last table.
            schema
                .objects
                .insert_with_key(|id| match active_table_type.as_str() {
                    "BASE TABLE" => Object::Table {
                        id,
                        score: None,
                        name: active_table,
                        columns: columns.drain(..).collect(),
                        foreign_keys: Vec::new(),
                    },

                    "VIEW" => Object::View {
                        id,
                        score: None,
                        name: active_table,
                        columns: columns.drain(..).collect(),
                    },
                    _ => panic!("Unknown table type: {:?}", active_table_type),
                });
        }

        // TODO: Extract foreign keys

        Ok(schema)
    }
}
