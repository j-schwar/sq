use std::{cell::RefCell, cmp::Ordering, fmt::Display};

use keywords::{KeywordMap, Match};
use serde::{Deserialize, Serialize};

use crate::arena::{Arena, Id, PersistedArena};

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DataType {
    #[default]
    Unknown,
    Integer,
}

impl Display for DataType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DataType::Integer => write!(f, "int"),
            DataType::Unknown => write!(f, "unknown"),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Column {
    pub name: String,
    pub data_type: DataType,
    pub nullable: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Object {
    Table {
        name: String,
        columns: KeywordMap<String, Score<Id<Column>>>,
    },

    View {
        name: String,
        columns: KeywordMap<String, Score<Id<Column>>>,
    },
}

impl Object {
    /// Returns the name of the object.
    pub fn name(&self) -> &str {
        match self {
            Object::Table { name, .. } | Object::View { name, .. } => name,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Score<T> {
    value: T,
    score: RefCell<f64>,
}

impl<T> Score<T> {
    /// Creates a new [`Score`] wrapper around a value with a set initial score.
    fn with_default_score(value: T) -> Self {
        Self {
            value,
            score: RefCell::new(4.0),
        }
    }

    /// Increases this score by a set amount.
    fn increase_score(&self) {
        let mut score = self.score.borrow_mut();
        let new_score = if *score < 1.0 { 2.0 } else { *score * 2.0 };

        tracing::debug!("Score increased from {} to {}", score, new_score);
        *score = new_score;
    }
}

impl<T> PartialEq for Score<T> {
    fn eq(&self, other: &Self) -> bool {
        self.score == other.score
    }
}

impl<T> PartialOrd for Score<T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.score
            .partial_cmp(&other.score)
            .map(|ord| ord.reverse())
    }
}

#[derive(Default, Serialize, Deserialize)]
pub struct Schema {
    objects: PersistedArena<Object>,
    columns: PersistedArena<Column>,
    object_names: KeywordMap<String, Score<Id<Object>>>,
}

impl Arena<Object> for Schema {
    fn alloc(&mut self, object: Object) -> Id<Object> {
        let name = object.name().to_owned();
        let id = self.objects.alloc(object);
        let score = Score::with_default_score(id);
        self.object_names.insert(name, score);
        id
    }

    fn get(&self, id: Id<Object>) -> Option<&Object> {
        self.objects.get(id)
    }

    fn iter<'a>(&'a self) -> impl Iterator<Item = (Id<Object>, &'a Object)>
    where
        Object: 'a,
    {
        self.objects.iter()
    }
}

impl Arena<Column> for Schema {
    fn alloc(&mut self, column: Column) -> Id<Column> {
        self.columns.alloc(column)
    }

    fn get(&self, id: Id<Column>) -> Option<&Column> {
        self.columns.get(id)
    }

    fn iter<'a>(&'a self) -> impl Iterator<Item = (Id<Column>, &'a Column)>
    where
        Column: 'a,
    {
        self.columns.iter()
    }
}

/// Finds the best match for a name in a [`KeywordMap`], returning the ID of the matched item.
///
/// This is either an exact match, a partial match with the highest score, or `None` if no matches
/// are found.
#[tracing::instrument(skip(map), level = "debug")]
fn find_best_match<'a, T>(
    map: &'a KeywordMap<String, Score<Id<T>>>,
    name: &str,
) -> Option<&'a Score<Id<T>>>
where
    T: std::fmt::Debug,
{
    let mut matches = map.find_by_partial_keyword(name).collect::<Vec<_>>();
    tracing::debug!("Found {} matches", matches.len());

    matches.sort_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Greater));
    let Some(best_match) = matches.first() else {
        tracing::debug!("No matches found");
        return None;
    };

    match best_match {
        Match::Exact(wrapper) => {
            tracing::debug!("Exact match found: {:?}", wrapper.value);
        }

        Match::Prefix(wrapper) => {
            tracing::debug!(
                "Prefix match found: {:?}; score = {}",
                wrapper.value,
                wrapper.score.borrow()
            );
        }
    }

    Some(best_match.as_ref())
}

/// Context for evaluating objects in the schema.
pub struct ObjectEvalContext<'a> {
    schema: &'a Schema,
}

impl<'a> ObjectEvalContext<'a> {
    /// Creates a new evaluation context for the given schema.
    pub fn new(schema: &'a Schema) -> Self {
        Self { schema }
    }
}

/// Resolves an object by name (or partial name) within the context of the given schema.
pub fn resolve_object<'a>(ctx: &ObjectEvalContext<'a>, name: &str) -> Option<Id<Object>> {
    // TODO: If we're resolving within the context of a parent object, we need to narrow down the
    //  search to only objects that have foreign keys to the parent object.

    let Some(best_match) = find_best_match(&ctx.schema.object_names, name) else {
        return None;
    };

    best_match.increase_score();
    Some(best_match.value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_log::test;

    #[test]
    fn test_resolve_object() {
        let mut schema = Schema::default();

        let table_id = schema.alloc(Object::Table {
            name: "users".to_string(),
            columns: KeywordMap::default(),
        });

        let view_id = schema.alloc(Object::View {
            name: "active_users".to_string(),
            columns: KeywordMap::default(),
        });

        let ctx = ObjectEvalContext::new(&schema);

        assert_eq!(resolve_object(&ctx, "users"), Some(table_id));
        assert_eq!(resolve_object(&ctx, "active"), Some(view_id));
        assert_eq!(resolve_object(&ctx, "nonexistent"), None);
    }

    #[test]
    fn test_resolve_object_with_score() {
        let mut schema = Schema::default();

        let table_id = schema.alloc(Object::Table {
            name: "users".to_string(),
            columns: KeywordMap::default(),
        });

        let _ = schema.alloc(Object::View {
            name: "active_users".to_string(),
            columns: KeywordMap::default(),
        });

        let ctx = ObjectEvalContext::new(&schema);

        // Resolve the object once to increase its score
        resolve_object(&ctx, "users");

        // Resolve using a partial name and ensure the correct object is returned
        assert_eq!(resolve_object(&ctx, "us"), Some(table_id));
    }
}
