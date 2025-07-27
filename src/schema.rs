use std::{cell::RefCell, cmp::Ordering, fmt::Display, time::SystemTime};

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
pub struct Score {
    pub value: f64,
    pub timestamp: u64,
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
    pub fn record_hit(&mut self) {
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

pub(crate) trait ScoreContainer<K, V> {
    /// Returns the score of the given key, if it exists.
    fn score_of(&self, key: K) -> Option<Score>;

    /// Allocates a new value with an optional score and returns its key.
    fn alloc(&mut self, value: V, score: Option<Score>) -> K;

    /// Allocates a new value without a score and returns its key.
    fn alloc_without_score(&mut self, value: V) -> K {
        self.alloc(value, None)
    }
}

#[derive(Default, Serialize, Deserialize)]
pub struct Schema {
    pub objects: SlotMap<ObjectId, Object>,
    pub columns: SlotMap<ColumnId, Column>,
    pub object_scores: RefCell<SparseSecondaryMap<ObjectId, Score>>,
    pub column_scores: RefCell<SparseSecondaryMap<ColumnId, Score>>,
}

impl ScoreContainer<ObjectId, Object> for Schema {
    fn score_of(&self, key: ObjectId) -> Option<Score> {
        self.object_scores.borrow().get(key).cloned()
    }

    fn alloc(&mut self, value: Object, score: Option<Score>) -> ObjectId {
        let key = self.objects.insert(value);
        if let Some(score) = score {
            let mut scores = self.object_scores.borrow_mut();
            scores.insert(key, score);
        }

        key
    }
}

impl ScoreContainer<ColumnId, Column> for Schema {
    fn score_of(&self, key: ColumnId) -> Option<Score> {
        self.column_scores.borrow().get(key).cloned()
    }

    fn alloc(&mut self, value: Column, score: Option<Score>) -> ColumnId {
        let key = self.columns.insert(value);
        if let Some(score) = score {
            let mut scores = self.column_scores.borrow_mut();
            scores.insert(key, score);
        }

        key
    }
}
