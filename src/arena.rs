use serde::{Deserialize, Serialize};

pub trait Arena<T> {
    /// Allocates a new object in the arena and returns its id.
    fn alloc(&mut self, object: T) -> Id<T>;

    /// Returns a reference to the object with the given id, if it exists.
    fn get(&self, id: Id<T>) -> Option<&T>;

    /// Retrieves an iterator over the objects in the arena, and their corresponding ids.
    fn iter<'a>(&'a self) -> impl Iterator<Item = (Id<T>, &'a T)>
    where
        T: 'a;
}

/// Identifier for an object stored in a [`PersistedArena`].
#[derive(Serialize, Deserialize)]
pub struct Id<T> {
    index: usize,
    #[serde(skip)]
    _marker: std::marker::PhantomData<T>,
}

impl<T> std::fmt::Debug for Id<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Id({})", self.index)
    }
}

impl<T> Clone for Id<T> {
    fn clone(&self) -> Self {
        Self {
            index: self.index,
            _marker: std::marker::PhantomData,
        }
    }
}

impl<T> Copy for Id<T> {}

impl<T> PartialEq for Id<T> {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index
    }
}

impl<T> Eq for Id<T> {}

/// An serializable arena for storing objects of type `T`.
#[derive(Serialize, Deserialize)]
pub struct PersistedArena<T> {
    objects: Vec<T>,
}

impl<T> PersistedArena<T> {
    /// Creates a new empty `PersistedArena`.
    pub fn new() -> Self {
        Self {
            objects: Vec::new(),
        }
    }
}

impl<T> Arena<T> for PersistedArena<T> {
    /// Allocates a new object in the arena and returns its id.
    fn alloc(&mut self, object: T) -> Id<T> {
        let index = self.objects.len();
        self.objects.push(object);
        Id {
            index,
            _marker: std::marker::PhantomData,
        }
    }

    /// Returns a reference to the object with the given id, if it exists.
    fn get(&self, id: Id<T>) -> Option<&T> {
        self.objects.get(id.index)
    }

    /// Retrieves an iterator over the objects in the arena, and their corresponding ids.
    fn iter<'a>(&'a self) -> impl Iterator<Item = (Id<T>, &'a T)>
    where
        T: 'a,
    {
        self.objects.iter().enumerate().map(|(index, object)| {
            (
                Id {
                    index,
                    _marker: std::marker::PhantomData,
                },
                object,
            )
        })
    }
}

impl<T> Default for PersistedArena<T> {
    fn default() -> Self {
        Self::new()
    }
}
