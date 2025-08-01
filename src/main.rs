use std::{cmp::Ordering, fmt::Display};

use clap::{Parser, Subcommand, command};
use keywords::{KeywordMap, Match};

use crate::{
    ast::{ObjectTree, Query},
    schema::{ColumnId, Name, Object, ObjectId, Schema, Score, ScoreContainer},
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
    tracing::debug!(
        "Found {} matches from {} possible options",
        matches.len(),
        map.len()
    );

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
#[inline]
fn normalize_case(s: &str) -> String {
    s.to_lowercase()
}

/// Constructs a keyword map from an iterator over id, value pairs.
fn create_id_keyword_map<'a, I, K, V>(iter: I) -> KeywordMap<String, (K, Option<Score>)>
where
    V: Name + 'a,
    I: Iterator<Item = (K, &'a V, Option<Score>)>,
{
    let mut map = KeywordMap::new();
    for (id, value, score) in iter {
        let name = normalize_case(value.name());
        map.insert(name, (id, score));
    }

    map
}

/// Fetches all other objects that reference a given object via a foreign key.
fn foreign_objects(
    schema: &Schema,
    id: ObjectId,
) -> impl Iterator<Item = (ObjectId, &Object, Option<Score>)> {
    <Schema as ScoreContainer<'_, ObjectId, Object>>::iter_with_score(schema).filter(
        move |(_, obj, _)| {
            obj.foreign_keys()
                .iter()
                .any(|fk| fk.referenced_object == id)
        },
    )
}

/// Error type indicating name resolution could not be performed.
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

/// Trait for types which can resolve names within a schema context.
trait NameResolution {
    /// Type of additional context required for name resolution. This could include the schema,
    /// parent object, or other relevant information.
    type Ctx;

    /// The output type after resolving names. This could be an object ID, column ID, or any other
    /// relevant type.
    type Output;

    /// Transforms the current instance by resolving names based on the provided context.
    fn resolve_names(self, ctx: &Self::Ctx) -> Result<Self::Output, ResolutionError>;
}

impl NameResolution for ObjectTree<String> {
    type Ctx = Schema;
    type Output = ObjectTree<ObjectId>;

    fn resolve_names(self, ctx: &Self::Ctx) -> Result<Self::Output, ResolutionError> {
        self.try_map_with_ancestors(|ancestors, name| {
            // Construct a keyword map for all possible objects given the current context.
            let map = if let Some(last) = ancestors.last() {
                let iter = foreign_objects(ctx, *last);
                create_id_keyword_map(iter)
            } else {
                let iter = <Schema as ScoreContainer<'_, ObjectId, Object>>::iter_with_score(ctx);
                create_id_keyword_map(iter)
            };

            // Find the best match for `name`.
            let Some(best_match) = find_best_match(&map, &normalize_case(&name)) else {
                return Err(ResolutionError { name });
            };

            // Record a hit for the matched object to increase its score.
            let mut object_scores = ctx.object_scores.borrow_mut();
            if let Some(score) = object_scores.get_mut(*best_match) {
                score.record_hit();
            } else {
                object_scores.insert(*best_match, Score::default());
            }

            // Return the best match.
            Ok(*best_match)
        })
    }
}

impl<'a> NameResolution for Query<'a, String, String> {
    type Ctx = Schema;
    type Output = Query<'a, ObjectId, ColumnId>;

    fn resolve_names(self, ctx: &Self::Ctx) -> Result<Self::Output, ResolutionError> {
        let object = self.object.resolve_names(ctx)?;
        let query = Query {
            object,
            predicates: vec![], // TODO: implement me
        };

        Ok(query)
    }
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

    schema.alloc_without_score(Object::Table {
        name: "HM_VOYAGE".to_string(),
        columns: vec![],
        foreign_keys: vec![],
    });

    schema.alloc_without_score(Object::Table {
        name: "HM_VOYAGE_JOB".to_string(),
        columns: vec![],
        foreign_keys: vec![],
    });

    schema.alloc_without_score(Object::Table {
        name: "REF_DOMAIN".to_string(),
        columns: vec![],
        foreign_keys: vec![],
    });

    tracing::debug!("Loaded dummy schema");
    schema
}

fn query(opts: QueryOpts) -> anyhow::Result<()> {
    let query_string = opts.query.join(" ");
    tracing::debug!("Executing query: {}", query_string);

    let s = fetch_schema();
    let query = ast::parse(&query_string)?.resolve_names(&s)?;
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
    use crate::schema::{Column, DataType, ForeignKey, Object};

    use super::*;
    use test_log::test;

    #[test]
    fn test_resolve_object_tree() {
        let mut schema = Schema::default();

        let users_id = schema.alloc_without_score(Column {
            name: "ID".to_string(),
            data_type: DataType::Integer,
            nullable: false,
        });

        let users_table_id = schema.alloc_without_score(Object::Table {
            name: "AUTH_USERS".to_string(),
            columns: vec![users_id],
            foreign_keys: vec![],
        });

        let privilege_id = schema.alloc_without_score(Column {
            name: "ID".to_string(),
            data_type: DataType::Integer,
            nullable: false,
        });

        let privilege_user_id = schema.alloc_without_score(Column {
            name: "USER_ID".to_string(),
            data_type: DataType::Integer,
            nullable: false,
        });

        let privilege_table_id = schema.alloc_without_score(Object::Table {
            name: "AUTH_PRIVILEGES".to_string(),
            columns: vec![privilege_id, privilege_user_id],
            foreign_keys: vec![ForeignKey {
                column: privilege_user_id,
                referenced_object: users_table_id,
                referenced_column: users_id,
            }],
        });

        let query = ast::parse("user>priv").unwrap();
        let resolved_tree = query.object.resolve_names(&schema).unwrap();
        assert_eq!(resolved_tree.root, users_table_id);
        assert_eq!(resolved_tree.children.len(), 1);
        assert_eq!(resolved_tree.children[0].root, privilege_table_id);
    }

    #[test]
    fn test_resolve_object_tree_with_score() {
        let mut schema = Schema::default();

        let _ = schema.alloc_without_score(Object::Table {
            name: "companions".to_string(),
            columns: vec![],
            foreign_keys: vec![],
        });

        let table = schema.alloc(
            Object::Table {
                name: "companies".to_string(),
                columns: vec![],
                foreign_keys: vec![],
            },
            Some(Score::default()),
        );

        let query = ast::parse("comp").unwrap();
        let resolved_tree = query.object.resolve_names(&schema).unwrap();
        assert_eq!(resolved_tree.root, table);
    }
}
