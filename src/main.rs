use std::{env, process::ExitCode};

use anyhow::anyhow;
use clap::{Parser, Subcommand, command};
use tracing_subscriber::fmt::format::FmtSpan;

use crate::{
    alg::Name,
    config::{Config, Profile},
    db::Database,
    schema::Schema,
};

mod alg;
mod ast;
mod config;
mod db;
mod schema;
mod sql;

#[derive(Debug, Parser)]
struct QueryOpts {
    /// The query to execute.
    #[arg(trailing_var_arg = true)]
    query: Vec<String>,
}

#[derive(Debug, Parser)]
struct DefineOpts {
    /// Name of the object to define.
    object: Option<String>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Executes a query against a database.
    #[command(alias = "q")]
    Query(QueryOpts),

    /// Shows the definition of an object.
    #[command(alias = "d")]
    Define(DefineOpts),
}

/// sq - Simple Query
#[derive(Debug, Parser)]
#[command(author, version, about, long_about = None)]
struct Opts {
    /// Name of the connection profile to load.
    profile: Option<String>,

    #[command(subcommand)]
    command: Command,

    /// Enable debug output.
    #[arg(short, global = true, action = clap::ArgAction::Count)]
    debug: u8,
}

fn fetch_schema(database: &dyn Database, _profile: &Profile) -> anyhow::Result<Schema> {
    // TODO: load profile from cache if it exists
    database.schema()
}

fn connect(config: &Config, opts: &Opts) -> anyhow::Result<Box<dyn Database>> {
    let profile_name = opts.profile.as_deref().unwrap_or("default");
    let profile = config.profiles.get(profile_name).ok_or_else(|| {
        anyhow!(
            "Profile not found: {}",
            opts.profile.as_deref().unwrap_or("default")
        )
    })?;

    db::connect(&profile.driver).map_err(|err| {
        tracing::error!("Failed to connect to database: {}", err);
        anyhow!("failed to connect to database")
    })
}

#[tracing::instrument(skip_all, err)]
fn query(_config: &Config, _opts: &Opts, _query_opts: &QueryOpts) -> anyhow::Result<()> {
    todo!()
}

#[tracing::instrument(skip_all, err)]
fn define(config: &Config, opts: &Opts, define_opts: &DefineOpts) -> anyhow::Result<()> {
    let profile = opts.profile.as_deref().unwrap_or("default");
    let Some(profile) = config.profiles.get(profile) else {
        tracing::error!("Profile not found: {}", profile);
        return Err(anyhow!("unknown profile"));
    };

    let database = connect(config, opts)?;
    let schema = fetch_schema(database.as_ref(), profile)?;
    let Some(object_name) = &define_opts.object else {
        for obj in schema.objects.values() {
            println!("{}", obj.name());
        }
        return Ok(());
    };

    let Some(obj) = alg::find_best(object_name, schema.objects.values()) else {
        tracing::error!("Object not found: {}", object_name);
        return Err(anyhow!("unknown object"));
    };

    let column_ids = match obj {
        schema::Object::Table { columns, .. } | schema::Object::View { columns, .. } => columns,
    };

    let mut columns = Vec::with_capacity(column_ids.len());
    for column_id in column_ids {
        let Some(column) = schema.columns.get(*column_id) else {
            tracing::error!("Column not found: {:?}", column_id);
            return Err(anyhow!("unknown column"));
        };
        columns.push(column);
    }

    columns.sort_by(|a, b| a.name.cmp(&b.name));
    for column in columns {
        println!("{}", column.name);
    }

    Ok(())
}

fn run(opts: Opts) -> anyhow::Result<()> {
    let cfg = config::load().map_err(|err| anyhow!("invalid configuration: {}", err))?;

    match &opts.command {
        Command::Query(query_opts) => query(&cfg, &opts, query_opts)?,
        Command::Define(define_opts) => define(&cfg, &opts, define_opts)?,
        // Command::Config(config_opts) => config(&cfg, config_opts)?,
    }

    Ok(())
}

#[tracing::instrument]
fn main() -> ExitCode {
    let proc_name = env::args().next().unwrap_or_else(|| String::from("sq"));
    let opts = Opts::parse();

    // Setup tracing based on the debug level.
    if let Some(level) = match opts.debug {
        0 => None,
        1 => Some(tracing::Level::DEBUG),
        _ => Some(tracing::Level::TRACE),
    } {
        if let Err(err) = tracing::subscriber::set_global_default(
            tracing_subscriber::fmt()
                .with_max_level(level)
                .with_span_events(FmtSpan::CLOSE)
                .finish(),
        ) {
            eprintln!("{}: failed to initialize logging: {}", proc_name, err);
        }
    }

    if let Err(err) = run(opts) {
        eprintln!("{}: {}", proc_name, err);
        return ExitCode::FAILURE;
    }

    ExitCode::SUCCESS
}
