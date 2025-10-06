use std::fmt::Display;

use serde::{Deserialize, Serialize};
use slotmap::{SlotMap, new_key_type};

use crate::alg::{Name, Score, Scored};

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
    pub id: ColumnId,
    pub score: Option<Score>,
    pub name: String,
    pub data_type: DataType,
    pub nullable: bool,
}

impl Name for Column {
    #[inline]
    fn name(&self) -> &str {
        &self.name
    }
}

impl Scored for Column {
    #[inline]
    fn score(&self) -> Option<Score> {
        self.score
    }

    #[inline]
    fn score_mut(&mut self) -> &mut Option<Score> {
        &mut self.score
    }
}

/// Models a foreign key relationship between columns in different objects.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ForeignKey {
    pub column: ColumnId,
    pub referenced_object: ObjectId,
    pub referenced_column: ColumnId,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Object {
    Table {
        id: ObjectId,
        score: Option<Score>,
        name: String,
        columns: Vec<ColumnId>,
        foreign_keys: Vec<ForeignKey>,
    },

    View {
        id: ObjectId,
        score: Option<Score>,
        name: String,
        columns: Vec<ColumnId>,
    },
}

impl Name for Object {
    fn name(&self) -> &str {
        match self {
            Object::Table { name, .. } | Object::View { name, .. } => name,
        }
    }
}

impl Scored for Object {
    fn score(&self) -> Option<Score> {
        match self {
            Object::Table { score, .. } | Object::View { score, .. } => *score,
        }
    }

    fn score_mut(&mut self) -> &mut Option<Score> {
        match self {
            Object::Table { score, .. } | Object::View { score, .. } => score,
        }
    }
}

impl Object {
    pub fn foreign_keys(&self) -> &[ForeignKey] {
        match self {
            Object::Table { foreign_keys, .. } => foreign_keys,
            Object::View { .. } => &[],
        }
    }
}

#[derive(Default, Serialize, Deserialize)]
pub struct Schema {
    pub objects: SlotMap<ObjectId, Object>,
    pub columns: SlotMap<ColumnId, Column>,
}

impl Schema {
    /// Fetches all other objects that reference a given object via a foreign key.
    #[allow(dead_code)]
    pub fn foreign_objects(&self, id: ObjectId) -> impl Iterator<Item = &Object> {
        self.objects
            .values()
            .filter(move |o| o.foreign_keys().iter().any(|fk| fk.referenced_object == id))
    }
}
