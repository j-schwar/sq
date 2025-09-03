use std::{
    cmp::Ordering,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};

/// An iterator over ASCII keywords in a string.
struct AsciiKeywords<'a> {
    s: &'a [u8],
    index: usize,
}

impl<'a> AsciiKeywords<'a> {
    /// Creates a new `AsciiKeywords` iterator from a string slice.
    fn new(s: &'a str) -> Self {
        AsciiKeywords {
            s: s.as_bytes(),
            index: 0,
        }
    }
}

impl<'a> Iterator for AsciiKeywords<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.s.len() {
            return None;
        }

        let start = self.index;
        if self.s[start].is_ascii_alphabetic() {
            while self.index < self.s.len() && self.s[self.index].is_ascii_alphabetic() {
                self.index += 1;
            }
        } else if self.s[start].is_ascii_digit() {
            while self.index < self.s.len() && self.s[self.index].is_ascii_digit() {
                self.index += 1;
            }
        }

        let keyword = std::str::from_utf8(&self.s[start..self.index]).ok()?;
        if keyword.is_empty() {
            return None;
        }

        // Skip any non-alphanumeric characters
        while self.index < self.s.len() && !self.s[self.index].is_ascii_alphanumeric() {
            self.index += 1;
        }

        Some(keyword)
    }
}

/// Trait providing access to iterators over keywords in a string.
trait Keywords {
    /// Returns an iterator over the ASCII keywords in the string.
    ///
    /// A keyword is defined as a sequence of ASCII alphabetic or numeric characters separated by
    /// non- alphanumeric characters (e.g., whitespace, punctuation). Non-alphanumeric characters
    /// will not be returned in the output.
    ///
    /// Example usage:
    /// ```
    /// use keywords::Keywords;
    ///
    /// let text = "hello_world, testing123!";
    /// let mut keywords = text.ascii_keywords();
    ///
    /// assert_eq!(Some("hello"), keywords.next());
    /// assert_eq!(Some("world"), keywords.next());
    /// assert_eq!(Some("testing"), keywords.next());
    /// assert_eq!(Some("123"), keywords.next());
    /// assert_eq!(None, keywords.next());
    /// ```
    fn ascii_keywords(&self) -> impl Iterator<Item = &str> + '_;
}

impl Keywords for &str {
    #[inline]
    fn ascii_keywords(&self) -> impl Iterator<Item = &str> + '_ {
        AsciiKeywords::new(self)
    }
}

impl Keywords for String {
    #[inline]
    fn ascii_keywords(&self) -> impl Iterator<Item = &str> + '_ {
        AsciiKeywords::new(self)
    }
}

/// Trait for named objects.
pub trait Name {
    /// Gets the name of this instance.
    fn name(&self) -> &str;
}

impl<'a, T> Name for &'a T
where
    T: Name,
{
    fn name(&self) -> &str {
        (*self).name()
    }
}

impl<'a, T> Name for &'a mut T
where
    T: Name,
{
    fn name(&self) -> &str {
        (**self).name()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum Match<V> {
    Exact(V),
    Prefix(V),
}

impl<V> Match<V> {
    /// Extracts the inner value from the `Match`.
    pub fn into_inner(self) -> V {
        match self {
            Match::Exact(v) | Match::Prefix(v) => v,
        }
    }
}

impl<V> Match<V>
where
    V: Name,
{
    fn match_named_value(pat: &str, v: V) -> Option<Self> {
        let name = v.name();
        if name == pat {
            return Some(Match::Exact(v));
        }

        let is_match = name
            .ascii_keywords()
            .filter(|k| k.starts_with(pat))
            .next()
            .is_some();

        if is_match {
            Some(Match::Prefix(v))
        } else {
            None
        }
    }
}

impl<V> AsRef<V> for Match<V> {
    fn as_ref(&self) -> &V {
        match self {
            Match::Exact(v) | Match::Prefix(v) => v,
        }
    }
}

impl<V> AsMut<V> for Match<V> {
    fn as_mut(&mut self) -> &mut V {
        match self {
            Match::Exact(v) | Match::Prefix(v) => v,
        }
    }
}

impl<V> PartialOrd for Match<V>
where
    V: PartialOrd,
{
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        match (self, other) {
            (Match::Exact(a), Match::Exact(b)) => a.partial_cmp(b),
            (Match::Prefix(a), Match::Prefix(b)) => a.partial_cmp(b),
            (Match::Exact(_), Match::Prefix(_)) => Some(std::cmp::Ordering::Less),
            (Match::Prefix(_), Match::Exact(_)) => Some(std::cmp::Ordering::Greater),
        }
    }
}

impl<V> Ord for Match<V>
where
    V: Ord,
{
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.partial_cmp(other).unwrap_or(std::cmp::Ordering::Equal)
    }
}

/// Represents the score (or rank) of an item.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Score {
    pub value: f64,
    pub timestamp: u64,
}

impl Score {
    /// Constructs a new [`Score`] with a given value and the current timestamp.
    pub fn new(value: f64) -> Self {
        Score {
            value,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        }
    }
}

impl PartialEq for Score {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

impl PartialOrd for Score {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.value.partial_cmp(&other.value).map(|c| c.reverse())
    }
}

/// A value paired with a [`Score`].
#[derive(Debug, Serialize, Deserialize)]
pub struct ScoredValue<V> {
    pub value: V,
    pub score: Option<Score>,
}

impl<V> ScoredValue<V> {
    /// Constructs a new [`ScoredValue`] without a score.
    pub fn new(value: V) -> Self {
        ScoredValue { value, score: None }
    }

    /// Constructs a new [`ScoredValue`] with a provide score.
    pub fn with_score(value: V, score: Score) -> Self {
        ScoredValue {
            value,
            score: Some(score),
        }
    }
}

impl<V> PartialEq for ScoredValue<V> {
    fn eq(&self, other: &Self) -> bool {
        self.score == other.score
    }
}

impl<V> PartialOrd for ScoredValue<V> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.score.partial_cmp(&other.score)
    }
}

impl<V> Name for ScoredValue<V>
where
    V: Name,
{
    fn name(&self) -> &str {
        self.value.name()
    }
}

/// Finds the closest matching value in a given slice based on a partial name search.
pub fn find_best<'a, V, I>(name: &str, items: I) -> Option<&'a ScoredValue<V>>
where
    V: Name + std::fmt::Debug,
    I: Iterator<Item = &'a ScoredValue<V>>,
{
    let mut matches = items
        .into_iter()
        .filter_map(|x| Match::match_named_value(name, x))
        .collect::<Vec<_>>();

    matches.sort_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Greater));
    matches.into_iter().next().map(|m| m.into_inner())
}

#[cfg(test)]
mod test {
    use super::*;

    #[derive(Debug)]
    struct Value(&'static str);

    impl Name for Value {
        fn name(&self) -> &str {
            self.0
        }
    }

    #[test]
    fn find_best_match_simple_case() {
        let items = [
            ScoredValue::new(Value("foo")),
            ScoredValue::with_score(Value("bar"), Score::new(10.0)),
            ScoredValue::with_score(Value("baz"), Score::new(5.0)),
        ];

        let best_match = find_best("ba", items.iter()).unwrap();
        assert_eq!("bar", best_match.value.0);
    }

    #[test]
    fn find_best_match_exact_match_preferred() {
        let items = [
            ScoredValue::new(Value("foo")),
            ScoredValue::with_score(Value("bar"), Score::new(10.0)),
            ScoredValue::with_score(Value("baz"), Score::new(5.0)),
        ];

        let best_match = find_best("baz", items.iter()).unwrap();
        assert_eq!("baz", best_match.value.0);
    }

    #[test]
    fn find_best_match_match_not_found() {
        let items = [
            ScoredValue::new(Value("foo")),
            ScoredValue::with_score(Value("bar"), Score::new(10.0)),
            ScoredValue::with_score(Value("baz"), Score::new(5.0)),
        ];

        let best_match = find_best("fizz", items.iter());
        assert!(best_match.is_none());
    }
}
