//! Headword table shared by all parsers.

use std::collections::HashMap;

/// Headwords of one dictionary.
///
/// Keeps the first-seen order (used to store definitions) and a copy sorted
/// case-insensitively — the same order the Python edition uses
/// (`sorted(words, key=str.lower)`, stable for equal keys).
#[derive(Debug, Default, Clone)]
pub struct WordTable {
    index: HashMap<String, usize>,
    insertion: Vec<String>,
    sorted: Vec<String>,
    sorted_lower: Vec<String>,
}

impl WordTable {
    /// Empty table.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a headword. Returns its slot, or `None` when it already exists
    /// (the first definition of a headword wins, as in the Python edition).
    pub fn insert(&mut self, word: &str) -> Option<usize> {
        if word.is_empty() || self.index.contains_key(word) {
            return None;
        }
        let slot = self.insertion.len();
        self.index.insert(word.to_string(), slot);
        self.insertion.push(word.to_string());
        Some(slot)
    }

    /// Build the sorted view. Call once after the last [`WordTable::insert`].
    pub fn finish(&mut self) {
        let mut pairs: Vec<(String, String)> = self
            .insertion
            .iter()
            .map(|w| (w.to_lowercase(), w.clone()))
            .collect();
        pairs.sort_by(|a, b| a.0.cmp(&b.0)); // stable
        self.sorted_lower = pairs.iter().map(|p| p.0.clone()).collect();
        self.sorted = pairs.into_iter().map(|p| p.1).collect();
    }

    /// Number of headwords.
    pub fn len(&self) -> usize {
        self.insertion.len()
    }

    /// `true` when there are no headwords.
    pub fn is_empty(&self) -> bool {
        self.insertion.is_empty()
    }

    /// Exact membership test.
    pub fn contains(&self, word: &str) -> bool {
        self.index.contains_key(word)
    }

    /// Slot assigned by [`WordTable::insert`].
    pub fn slot(&self, word: &str) -> Option<usize> {
        self.index.get(word).copied()
    }

    /// Headwords sorted case-insensitively.
    pub fn sorted(&self) -> &[String] {
        &self.sorted
    }

    /// First headword (in sorted order) equal to `word` ignoring case.
    pub fn find_case_insensitive(&self, word: &str) -> Option<&str> {
        let key = word.to_lowercase();
        let start = self
            .sorted_lower
            .partition_point(|w| w.as_str() < key.as_str());
        match self.sorted_lower.get(start) {
            Some(w) if *w == key => Some(self.sorted[start].as_str()),
            _ => None,
        }
    }

    /// Up to `limit` headwords starting with `prefix`, ignoring case.
    pub fn prefix_case_insensitive(&self, prefix: &str, limit: usize) -> Vec<&str> {
        let key = prefix.to_lowercase();
        let start = self
            .sorted_lower
            .partition_point(|w| w.as_str() < key.as_str());
        self.sorted_lower[start..]
            .iter()
            .zip(&self.sorted[start..])
            .take_while(|(lw, _)| lw.starts_with(key.as_str()))
            .take(limit)
            .map(|(_, w)| w.as_str())
            .collect()
    }

    /// Drop everything.
    pub fn clear(&mut self) {
        self.index.clear();
        self.insertion.clear();
        self.sorted.clear();
        self.sorted_lower.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table(words: &[&str]) -> WordTable {
        let mut t = WordTable::new();
        for w in words {
            t.insert(w);
        }
        t.finish();
        t
    }

    #[test]
    fn first_insert_wins() {
        let mut t = WordTable::new();
        assert_eq!(t.insert("a"), Some(0));
        assert_eq!(t.insert("a"), None);
        assert_eq!(t.insert(""), None);
        assert_eq!(t.len(), 1);
    }

    #[test]
    fn sorted_case_insensitive_and_stable() {
        let t = table(&["banana", "Apple", "apple", "cherry"]);
        assert_eq!(t.sorted(), &["Apple", "apple", "banana", "cherry"]);
    }

    #[test]
    fn case_insensitive_lookup() {
        let t = table(&["Apple", "banana"]);
        assert_eq!(t.find_case_insensitive("APPLE"), Some("Apple"));
        assert_eq!(t.find_case_insensitive("pear"), None);
    }

    #[test]
    fn prefix_lookup() {
        let t = table(&["car", "Cart", "cat", "dog", "ca"]);
        assert_eq!(
            t.prefix_case_insensitive("ca", 50),
            vec!["ca", "car", "Cart", "cat"]
        );
        assert_eq!(t.prefix_case_insensitive("CA", 2), vec!["ca", "car"]);
        assert!(t.prefix_case_insensitive("x", 50).is_empty());
    }
}
