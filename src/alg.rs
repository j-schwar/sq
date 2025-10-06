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

impl<T> Name for &T
where
    T: Name,
{
    fn name(&self) -> &str {
        (*self).name()
    }
}

impl<T> Name for &mut T
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

        let is_match = name.ascii_keywords().any(|k| {
            let k = k.to_ascii_lowercase();
            let pat = pat.to_ascii_lowercase();
            k.starts_with(&pat)
        });

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

/// Trait for types which have a score.
pub trait Scored {
    /// Gets the score of this item.
    fn score(&self) -> Option<Score>;

    /// Gets a mutable reference to the score of this item.
    fn score_mut(&mut self) -> &mut Option<Score>;
}

impl<T> Scored for &T
where
    T: Scored,
{
    fn score(&self) -> Option<Score> {
        (*self).score()
    }

    fn score_mut(&mut self) -> &mut Option<Score> {
        panic!("cannot get mutable score from immutable reference");
    }
}

impl<T> Scored for &mut T
where
    T: Scored,
{
    fn score(&self) -> Option<Score> {
        (**self).score()
    }

    fn score_mut(&mut self) -> &mut Option<Score> {
        (**self).score_mut()
    }
}

fn compare_matches<T>(a: &Match<T>, b: &Match<T>) -> Ordering
where
    T: Scored,
{
    a.as_ref()
        .score()
        .partial_cmp(&b.as_ref().score())
        .unwrap_or(Ordering::Greater)
}

/// Finds the closest matching value in a given slice based on a partial name search.
pub fn find_best_mut<'a, T, I>(name: &str, items: I) -> Option<&'a mut T>
where
    T: Name + Scored,
    I: Iterator<Item = &'a mut T>,
{
    let mut matches = items
        .into_iter()
        .filter_map(|x| Match::match_named_value(name, x))
        .collect::<Vec<_>>();

    matches.sort_by(compare_matches);
    matches.into_iter().next().map(|m| m.into_inner())
}

/// Updates a given score according to usage patterns.
///
/// This function should be called when an item is used or selected.
pub fn update_score(score: &mut Option<Score>) {
    let Some(s) = score else {
        *score = Some(Score::new(1.0));
        return;
    };

    // Following zoxide's algorithm for score calculation:
    // * If this score was last referenced within an hour: score * 4
    // * If this score was last referenced within a day: score * 2
    // * If this score was last referenced within a week: score / 2
    // * Otherwise: score / 4
    //
    // ref: https://github.com/ajeetdsouza/zoxide/wiki/Algorithm
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let age = now.saturating_sub(s.timestamp);
    s.timestamp = now;
    s.value = if age < 3600 {
        s.value * 4.0
    } else if age < 86400 {
        s.value * 2.0
    } else if age < 604800 {
        s.value / 2.0
    } else {
        s.value / 4.0
    };
}

#[cfg(test)]
mod test {
    use super::*;

    #[derive(Debug)]
    struct Value(&'static str, Option<Score>);

    impl Name for Value {
        fn name(&self) -> &str {
            self.0
        }
    }

    impl Scored for Value {
        fn score(&self) -> Option<Score> {
            self.1
        }

        fn score_mut(&mut self) -> &mut Option<Score> {
            &mut self.1
        }
    }

    #[test]
    fn find_best_match_simple_case() {
        let mut items = [
            Value("foo", None),
            Value("bar", Some(Score::new(10.0))),
            Value("baz", Some(Score::new(5.0))),
        ];

        let best_match = find_best_mut("ba", items.iter_mut()).unwrap();
        assert_eq!("bar", best_match.0);
    }

    #[test]
    fn find_best_match_exact_match_preferred() {
        let mut items = [
            Value("foo", None),
            Value("bar", Some(Score::new(10.0))),
            Value("baz", Some(Score::new(5.0))),
        ];

        let best_match = find_best_mut("baz", items.iter_mut()).unwrap();
        assert_eq!("baz", best_match.0);
    }

    #[test]
    fn find_best_match_match_not_found() {
        let mut items = [
            Value("foo", None),
            Value("bar", Some(Score::new(10.0))),
            Value("baz", Some(Score::new(5.0))),
        ];

        let best_match = find_best_mut("fizz", items.iter_mut());
        assert!(best_match.is_none());
    }
}
