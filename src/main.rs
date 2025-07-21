use std::{cmp::Ordering, fmt::Display};

use clap::{Parser, Subcommand, command};
use keywords::{KeywordMap, Match};

use crate::{
    ast::{ObjectTree, Query},
    schema::{ColumnId, ObjectId, Schema, Score, ScoreContainer},
};

mod ast;
mod schema;
mod sql;

/// Finds the best match for a name in a [`KeywordMap`], returning the ID of the matched item.
///
/// This is either an exact match, a partial match with the highest score, or `None` if no matches
/// are found.
#[tracing::instrument(skip(map), level = "debug")]
fn find_best_match<'a, T>(
    map: &'a KeywordMap<String, (T, Option<Score>)>,
    name: &str,
) -> Option<&'a T>
where
    T: std::fmt::Debug,
{
    let mut matches = map.find_by_partial_keyword(name).collect::<Vec<_>>();
    tracing::debug!("Found {} matches", matches.len());

    matches.sort_by(|a, b| {
        let a = a.as_ref().1;
        let b = b.as_ref().1;
        a.partial_cmp(&b).unwrap_or(Ordering::Greater).reverse()
    });

    let Some(best_match) = matches.first() else {
        tracing::debug!("No matches found");
        return None;
    };

    match best_match {
        Match::Exact((value, _)) => {
            tracing::debug!("Exact match found: {:?}", value);
        }

        Match::Prefix((value, score)) => {
            tracing::debug!(
                "Prefix match found: {:?}; score = {:?}",
                value,
                score.map(|s| s.value)
            );
        }
    }

    let (best_match, _) = best_match.as_ref();
    Some(best_match)
}

/// Normalizes a string to a consistent case for comparison.
fn normalize_case(s: &str) -> String {
    s.to_lowercase()
}

/// Context for evaluating objects in the schema.
struct ObjectEvalContext<'a> {
    schema: &'a Schema,
}

impl<'a> ObjectEvalContext<'a> {
    /// Creates a new evaluation context for the given schema.
    fn new(schema: &'a Schema) -> Self {
        Self { schema }
    }
}

/// Resolves an object by name (or partial name) within the context of the given schema.
fn resolve_object<'a>(ctx: &ObjectEvalContext<'a>, name: &str) -> Option<ObjectId> {
    // TODO: If we're resolving within the context of a parent object, we need to narrow down the
    //  search to only objects that have foreign keys to the parent object.

    let mut map = KeywordMap::new();
    for (id, _) in ctx.schema.objects.iter() {
        let object_name = ctx.schema.objects.get(id).unwrap().name();
        let score = ctx.schema.score_of(id);
        map.insert(normalize_case(object_name), (id, score));
    }

    let Some(best_match) = find_best_match(&map, &normalize_case(name)) else {
        return None;
    };

    // Record a hit for the matched object to increase its score
    let mut object_scores = ctx.schema.object_scores.borrow_mut();
    if let Some(score) = object_scores.get_mut(*best_match) {
        score.record_hit();
    } else {
        object_scores.insert(*best_match, Score::default());
    }

    Some(*best_match)
}

#[derive(Debug)]
struct ResolutionError {
    name: String,
}

impl Display for ResolutionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "unable to resolve: '{}'", self.name)
    }
}

impl std::error::Error for ResolutionError {}

fn resolve_object_tree(
    schema: &Schema,
    tree: ObjectTree<String>,
) -> Result<ObjectTree<ObjectId>, ResolutionError> {
    tree.try_map_with_ancestors(|_, ident| {
        let ctx = ObjectEvalContext::new(schema);
        let Some(id) = resolve_object(&ctx, &ident) else {
            return Err(ResolutionError { name: ident });
        };

        Ok(id)
    })
}

/// Performs name resolution for a query using a given schema.
fn resolve_names<'a>(
    schema: &Schema,
    query: Query<'a, String, String>,
) -> Result<Query<'a, ObjectId, ColumnId>, ResolutionError> {
    let object = resolve_object_tree(schema, query.object)?;

    let query = Query {
        object,
        predicates: vec![],
    };
    Ok(query)
}

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
    let query = resolve_names(&s, query)?;
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

#[cfg(test)]
mod tests {
    use crate::ast;
    use crate::schema::Object;

    use super::*;
    use test_log::test;

    #[test]
    fn test_resolve_object() {
        let mut schema = Schema::default();

        let table_id = schema.objects.insert(Object::Table {
            name: "users".to_string(),
            columns: vec![],
        });

        let view_id = schema.objects.insert(Object::View {
            name: "active_users".to_string(),
            columns: vec![],
        });

        let mut ctx = ObjectEvalContext::new(&mut schema);

        assert_eq!(resolve_object(&mut ctx, "users"), Some(table_id));
        assert_eq!(resolve_object(&mut ctx, "active"), Some(view_id));
        assert_eq!(resolve_object(&mut ctx, "nonexistent"), None);
    }

    #[test]
    fn test_resolve_object_case_insensitive() {
        let mut schema = Schema::default();

        let table_id = schema.objects.insert(Object::Table {
            name: "Users".to_string(),
            columns: vec![],
        });

        let mut ctx = ObjectEvalContext::new(&mut schema);

        assert_eq!(resolve_object(&mut ctx, "users"), Some(table_id));
        assert_eq!(resolve_object(&mut ctx, "USERS"), Some(table_id));
        assert_eq!(resolve_object(&mut ctx, "UsErS"), Some(table_id));
    }

    #[test]
    fn test_resolve_object_with_score() {
        let mut schema = Schema::default();

        let _ = schema.objects.insert(Object::View {
            name: "active_users".to_string(),
            columns: vec![],
        });

        let table_id = schema.objects.insert(Object::Table {
            name: "users".to_string(),
            columns: vec![],
        });

        let mut ctx = ObjectEvalContext::new(&mut schema);

        // Resolve the object once to increase its score
        resolve_object(&mut ctx, "users");

        // Resolve using a partial name and ensure the correct object is returned
        assert_eq!(resolve_object(&mut ctx, "us"), Some(table_id));
    }

    #[test]
    fn test_resolve_object_tree() {
        let mut schema = Schema::default();

        let users_table_id = schema.objects.insert(Object::Table {
            name: "auth_users".to_string(),
            columns: vec![],
        });

        let privilege_table_id = schema.objects.insert(Object::Table {
            name: "auth_privileges".to_string(),
            columns: vec![],
        });

        let query = ast::parse("user>priv").unwrap();
        let resolved_tree = resolve_object_tree(&mut schema, query.object).unwrap();
        assert_eq!(resolved_tree.root, users_table_id);
        assert_eq!(resolved_tree.children.len(), 1);
        assert_eq!(resolved_tree.children[0].root, privilege_table_id);
    }
}
