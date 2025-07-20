use std::{cmp::Ordering, fmt::Display, time::SystemTime};

use keywords::{KeywordMap, Match};
use serde::{Deserialize, Serialize};
use slotmap::{SlotMap, SparseSecondaryMap, new_key_type};

new_key_type! { pub struct ObjectId; }
new_key_type! { pub struct ColumnId; }

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
        columns: Vec<ColumnId>,
    },

    View {
        name: String,
        columns: Vec<ColumnId>,
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
struct Score {
    value: f64,
    timestamp: u64,
}

impl Default for Score {
    fn default() -> Self {
        Self {
            value: 4.0,
            timestamp: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        }
    }
}

impl Score {
    fn record_hit(&mut self) {
        if self.value < 1.0 {
            self.value = 4.0;
        } else {
            self.value *= 2.0;
        }

        self.timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
    }
}

impl PartialEq for Score {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

impl Eq for Score {}

impl PartialOrd for Score {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.value.partial_cmp(&other.value)
    }
}

#[derive(Default, Serialize, Deserialize)]
pub struct Schema {
    objects: SlotMap<ObjectId, Object>,
    columns: SlotMap<ColumnId, Column>,
    object_scores: SparseSecondaryMap<ObjectId, Score>,
    column_scores: SparseSecondaryMap<ColumnId, Score>,
}

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
            tracing::debug!("Prefix match found: {:?}; score = {:?}", value, score);
        }
    }

    let (best_match, _) = best_match.as_ref();
    Some(best_match)
}

/// Context for evaluating objects in the schema.
pub struct ObjectEvalContext<'a> {
    schema: &'a mut Schema,
}

impl<'a> ObjectEvalContext<'a> {
    /// Creates a new evaluation context for the given schema.
    pub fn new(schema: &'a mut Schema) -> Self {
        Self { schema }
    }
}

/// Resolves an object by name (or partial name) within the context of the given schema.
pub fn resolve_object<'a>(ctx: &mut ObjectEvalContext<'a>, name: &str) -> Option<ObjectId> {
    // TODO: If we're resolving within the context of a parent object, we need to narrow down the
    //  search to only objects that have foreign keys to the parent object.

    let mut map = KeywordMap::new();
    for (id, _) in ctx.schema.objects.iter() {
        let object_name = ctx.schema.objects.get(id).unwrap().name();
        let score = ctx.schema.object_scores.get(id).cloned();
        map.insert(object_name.to_string(), (id, score));
    }

    let Some(best_match) = find_best_match(&map, name) else {
        return None;
    };

    // Record a hit for the matched object to increase its score
    if let Some(score) = ctx.schema.object_scores.get_mut(*best_match) {
        score.record_hit();
    } else {
        ctx.schema
            .object_scores
            .insert(*best_match, Score::default());
    }

    Some(*best_match)
}

#[cfg(test)]
mod tests {
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
}
