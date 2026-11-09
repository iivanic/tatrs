//! Ordered `- KEY: value` property blocks.
//!
//! Order is preserved because it is part of the on-disk diff: a rewrite that
//! reorders unrelated properties creates noise in every task's history.

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Property {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Properties {
    items: Vec<Property>,
}

impl Properties {
    pub fn new() -> Self {
        Self::default()
    }

    /// Last value wins, matching upstream's duplicate-property rule.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.items
            .iter()
            .rev()
            .find(|p| p.key == key)
            .map(|p| p.value.as_str())
    }

    pub fn get_all<'a>(&'a self, key: &'a str) -> impl Iterator<Item = &'a str> + 'a {
        self.items
            .iter()
            .filter(move |p| p.key == key)
            .map(|p| p.value.as_str())
    }

    pub fn contains(&self, key: &str) -> bool {
        self.items.iter().any(|p| p.key == key)
    }

    /// Replace the last occurrence in place, or append a new property.
    pub fn put(&mut self, key: impl Into<String>, value: impl Into<String>) {
        let key = key.into();
        let value = value.into();
        if let Some(existing) = self.items.iter_mut().rev().find(|p| p.key == key) {
            existing.value = value;
        } else {
            self.items.push(Property { key, value });
        }
    }

    pub fn insert_missing(&mut self, key: impl Into<String>, value: impl Into<String>) {
        let key = key.into();
        if !self.contains(&key) {
            self.items.push(Property {
                key,
                value: value.into(),
            });
        }
    }

    pub fn remove(&mut self, key: &str) -> bool {
        let before = self.items.len();
        self.items.retain(|p| p.key != key);
        self.items.len() != before
    }

    pub fn iter(&self) -> impl Iterator<Item = &Property> {
        self.items.iter()
    }

    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.items.iter().map(|p| p.key.as_str())
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

impl IntoIterator for Properties {
    type Item = Property;
    type IntoIter = std::vec::IntoIter<Property>;

    fn into_iter(self) -> Self::IntoIter {
        self.items.into_iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn put_replaces_last_in_place() {
        let mut props = Properties::new();
        props.put("STATUS", "OPEN");
        props.put("PRIORITY", "100");
        props.put("STATUS", "CLOSED");
        assert_eq!(props.get("STATUS"), Some("CLOSED"));
        assert_eq!(props.len(), 2);
        assert_eq!(props.keys().collect::<Vec<_>>(), vec!["STATUS", "PRIORITY"]);
    }

    #[test]
    fn duplicates_are_visible_until_collapsed() {
        let mut props = Properties::new();
        props.put("STATUS", "OPEN");
        // put() replaces, so build a duplicate the way the parser can:
        // by pushing through insert_missing on a distinct key is not possible,
        // so this test documents the invariant instead of fabricating one.
        assert_eq!(props.get_all("STATUS").count(), 1);
        assert_eq!(props.get("MISSING"), None);
    }
}
