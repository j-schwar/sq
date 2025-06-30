use std::fmt::Display;

use keywords::KeywordMap;
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
        columns: KeywordMap<String, Id<Column>>,
    },

    View {
        name: String,
        columns: KeywordMap<String, Id<Column>>,
    },
}

#[derive(Serialize, Deserialize)]
pub struct Schema {
    objects: PersistedArena<Object>,
    columns: PersistedArena<Column>,
    object_names: KeywordMap<String, Id<Object>>,
}

impl Arena<Object> for Schema {
    fn alloc(&mut self, object: Object) -> Id<Object> {
        self.objects.alloc(object)
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
