use std::collections::BTreeMap;

use serde::{Serialize, Serializer};

use crate::content::model::Entry;

/// Entries of one kind in load order, with lookup by id.
///
/// Order: packs in load order, files in sorted path order, entries in file order. An override (same id
/// from a later pack) replaces the entry in place, so it keeps its position.
#[derive(Clone, Debug)]
pub struct Table<T> {
    items: Vec<T>,
    index: BTreeMap<String, usize>,
}

impl<T> Default for Table<T> {
    fn default() -> Self {
        Table {
            items: Vec::new(),
            index: BTreeMap::new(),
        }
    }
}

impl<T: Entry> Table<T> {
    /// Inserts or replaces by id. Returns the replaced entry.
    pub fn upsert(&mut self, item: T) -> Option<T> {
        match self.index.get(item.id()) {
            Some(&i) => Some(std::mem::replace(&mut self.items[i], item)),
            None => {
                self.index.insert(item.id().to_string(), self.items.len());
                self.items.push(item);
                None
            }
        }
    }

    pub fn get(&self, id: &str) -> Option<&T> {
        self.index.get(id).map(|&i| &self.items[i])
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut T> {
        self.index.get(id).map(|&i| &mut self.items[i])
    }

    pub fn contains(&self, id: &str) -> bool {
        self.index.contains_key(id)
    }

    /// Load-order position of an id (display order for stats and categories).
    pub fn position(&self, id: &str) -> Option<usize> {
        self.index.get(id).copied()
    }
}

impl<T> Table<T> {
    pub fn iter(&self) -> std::slice::Iter<'_, T> {
        self.items.iter()
    }

    pub fn iter_mut(&mut self) -> std::slice::IterMut<'_, T> {
        self.items.iter_mut()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn as_slice(&self) -> &[T] {
        &self.items
    }
}

impl<'a, T> IntoIterator for &'a Table<T> {
    type Item = &'a T;
    type IntoIter = std::slice::Iter<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.items.iter()
    }
}

impl<T: Serialize> Serialize for Table<T> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.items.serialize(s)
    }
}
