use clap::{Parser, Subcommand, command};

use crate::schema::Schema;

mod arena;
mod ast;
mod schema;
mod sql;

#[derive(Debug, Parser)]
struct QueryOpts {
    /// The query to execute.
    #[arg(trailing_var_arg = true)]
    query: Vec<String>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Executes a query against a database.
    #[command(alias = "q")]
    Query(QueryOpts),
}

/// sq - Simple Query
#[derive(Debug, Parser)]
#[command(author, version, about, long_about = None)]
struct Opts {
    /// Enable debug output.
    #[arg(short, global = true, action = clap::ArgAction::Count)]
    debug: u8,

    #[command(subcommand)]
    command: Command,
}

fn fetch_schema() -> Schema {
    // TODO: Fetch schema from cache or database.
    let mut schema = Schema::default();

    schema.objects.insert(schema::Object::Table {
        name: "HM_VOYAGE".to_string(),
        columns: vec![],
    });

    schema.objects.insert(schema::Object::Table {
        name: "HM_VOYAGE_JOB".to_string(),
        columns: vec![],
    });

    schema.objects.insert(schema::Object::Table {
        name: "REF_DOMAIN".to_string(),
        columns: vec![],
    });

    tracing::debug!("Loaded dummy schema");
    schema
}

fn query(opts: QueryOpts) -> anyhow::Result<()> {
    let query_string = opts.query.join(" ");
    tracing::debug!("Executing query: {}", query_string);

    let s = fetch_schema();
    let query = ast::parse(&query_string)?;
    let query = schema::resolve_names(&s, query)?;
    println!("{:#?}", query);
    Ok(())
}

fn main() {
    let opts = Opts::parse();

    // Setup tracing based on the debug level.
    if let Some(level) = match opts.debug {
        0 => None,
        1 => Some(tracing::Level::DEBUG),
        _ => Some(tracing::Level::TRACE),
    } {
        tracing::subscriber::set_global_default(
            tracing_subscriber::fmt().with_max_level(level).finish(),
        )
        .expect("Failed to set global default subscriber");
    }

    if let Err(err) = match opts.command {
        Command::Query(query_opts) => query(query_opts),
    } {
        eprintln!("{}: {}", std::env::args().next().unwrap(), err);
    }
}
