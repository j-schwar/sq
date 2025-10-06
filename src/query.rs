use anyhow::Result;
use std::fmt;

#[derive(Debug)]
pub enum Literal<'a> {
    /// A string literal.
    String(&'a str),
    /// A integer literal.
    Integer(i64),
}

impl<'a> fmt::Display for Literal<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Literal::String(s) => {
                if s.contains(' ') {
                    write!(f, "'{}'", s)
                } else {
                    write!(f, "{}", s)
                }
            }
            Literal::Integer(i) => write!(f, "{}", i),
        }
    }
}

/// A tree structure representing a hierarchy of objects.
///
/// Models syntax like `a>b+c>d` where `a` is the root, `b` and `c` are children of `a`, and `d` is
/// a child of `c`.
#[derive(Debug)]
pub struct ObjectTree<T> {
    pub root: T,
    pub children: Vec<ObjectTree<T>>,
}

impl<T: fmt::Display> fmt::Display for ObjectTree<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.root)?;
        if !self.children.is_empty() {
            write!(f, ">")?;
            for (i, child) in self.children.iter().enumerate() {
                if i > 0 {
                    write!(f, "+")?;
                }
                write!(f, "{}", child)?;
            }
        }
        Ok(())
    }
}

/// Boolean operators for query predicates.
#[derive(Debug)]
pub enum Operator {
    Eq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
}

impl fmt::Display for Operator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Operator::Eq => "=",
            Operator::Ne => "!=",
            Operator::Lt => "<",
            Operator::Gt => ">",
            Operator::Le => "<=",
            Operator::Ge => ">=",
        };
        write!(f, "{}", s)
    }
}

/// A predicate for filtering results in a query.
#[derive(Debug)]
pub struct Predicate<'a, T> {
    identifier: T,
    operator: Operator,
    value: Literal<'a>,
}

impl<'a, T: fmt::Display> fmt::Display for Predicate<'a, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}{}", self.identifier, self.operator, self.value)
    }
}

/// Models a SQL-like query.
#[derive(Debug)]
pub struct Query<'a, O, P> {
    pub object: ObjectTree<O>,
    pub predicates: Vec<Predicate<'a, P>>,
}

impl<'a, O: fmt::Display, P: fmt::Display> fmt::Display for Query<'a, O, P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.object)?;
        if !self.predicates.is_empty() {
            write!(f, " ")?;
            for (i, predicate) in self.predicates.iter().enumerate() {
                if i > 0 {
                    write!(f, " ")?;
                }
                write!(f, "{}", predicate)?;
            }
        }
        Ok(())
    }
}

/// Error type for syntax errors encountered during parsing.
#[derive(Debug)]
pub struct SyntaxError;

impl fmt::Display for SyntaxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "syntax error")
    }
}

impl std::error::Error for SyntaxError {}

type ParseResult<'a, T> = std::result::Result<(&'a str, T), SyntaxError>;

fn skip_whitespace(input: &str) -> &str {
    input.trim_start()
}

/// Parses an identifier from the input string.
#[tracing::instrument(level = "trace", err)]
fn parse_identifier<'a>(input: &'a str) -> ParseResult<'a, String> {
    let input = skip_whitespace(input);
    for (i, c) in input.char_indices() {
        if !c.is_alphanumeric() && c != '_' {
            if i == 0 {
                return Err(SyntaxError);
            }
            return Ok((&input[i..], input[..i].to_string()));
        }
    }

    if input.is_empty() {
        return Err(SyntaxError);
    }

    Ok(("", input.to_string()))
}

#[tracing::instrument(level = "trace", err)]
fn parse_object_tree<'a>(input: &'a str) -> ParseResult<'a, ObjectTree<String>> {
    // Parse root identifier.
    let input = skip_whitespace(input);
    let (input, root) = parse_identifier(input)?;

    // Next character should be '>', if not, we're done.
    let input = skip_whitespace(input);
    if !input.starts_with('>') {
        tracing::trace!("remaining input=\"{}\"", input);
        return Ok((
            input,
            ObjectTree {
                root,
                children: vec![],
            },
        ));
    }

    // Consume '>'.
    let input = &input[1..];

    // Parse the first child.
    let mut children = Vec::new();
    let (input, child) = parse_object_tree(input)?;
    children.push(child);

    // If the next character is '+', parse another child and add it to the list.
    let mut input = skip_whitespace(input);
    while let Some('+') = input.chars().next() {
        input = &input[1..]; // Consume '+'
        let (remaining, child) = parse_object_tree(input)?;
        children.push(child);

        input = skip_whitespace(remaining);
    }

    tracing::trace!("remaining input=\"{}\"", input);
    Ok((input, ObjectTree { root, children }))
}

/// Parses a literal value from the input string.
///
/// A literal can be a string or an integer. String literals are enclosed in single/double quotes,
/// or are a single word without spaces. Integer literals are sequences of digits.
#[tracing::instrument(level = "trace", err)]
fn parse_literal<'a>(input: &'a str) -> ParseResult<'a, Literal<'a>> {
    let input = skip_whitespace(input);
    if input.is_empty() {
        return Err(SyntaxError);
    }

    // Check for string literal (enclosed in quotes).
    if input.starts_with('\'') || input.starts_with('"') {
        let quote_char = input.chars().next().unwrap();
        let end_quote = input[1..].find(quote_char).ok_or(SyntaxError)?;
        let value = &input[1..end_quote + 1];
        return Ok((&input[end_quote + 2..], Literal::String(value)));
    }

    // Check for integer literal (sequence of digits).
    let next_non_digit = input.find(|c: char| !c.is_ascii_digit());
    if let Some(end) = next_non_digit {
        if end != 0 {
            let value = &input[..end];
            return Ok((&input[end..], Literal::Integer(value.parse().unwrap())));
        }
    } else if next_non_digit.is_none() && !input.is_empty() {
        // If no non-digit found, the whole input is a digit.
        return Ok(("", Literal::Integer(input.parse().unwrap())));
    }

    // Check for a single word literal (no spaces).
    let next_whitespace = input.find(|c: char| c.is_whitespace());
    if let Some(end) = next_whitespace {
        if end != 0 {
            let value = &input[..end];
            return Ok((&input[end..], Literal::String(value)));
        }
    } else if next_whitespace.is_none() && !input.is_empty() {
        // If no whitespace found, the whole input is a single word.
        return Ok(("", Literal::String(input)));
    }

    // If we reach here, the input is not a valid literal.
    Err(SyntaxError)
}

/// Parses an operator from the input string.
#[tracing::instrument(level = "trace", err)]
fn parse_operator(input: &str) -> ParseResult<'_, Operator> {
    let input = skip_whitespace(input);
    if let Some(rest) = input.strip_prefix("=") {
        return Ok((rest, Operator::Eq));
    } else if let Some(rest) = input.strip_prefix("!=") {
        return Ok((rest, Operator::Ne));
    } else if let Some(rest) = input.strip_prefix("<") {
        if let Some(rest) = rest.strip_prefix("=") {
            return Ok((rest, Operator::Le));
        }
        return Ok((rest, Operator::Lt));
    } else if let Some(rest) = input.strip_prefix(">") {
        if let Some(rest) = rest.strip_prefix("=") {
            return Ok((rest, Operator::Ge));
        }
        return Ok((rest, Operator::Gt));
    }

    Err(SyntaxError)
}

/// Parses a predicate from the input string.
#[tracing::instrument(level = "trace", err)]
fn parse_predicate<'a>(input: &'a str) -> ParseResult<'a, Predicate<'a, String>> {
    let input = skip_whitespace(input);
    let (input, identifier) = parse_identifier(input)?;
    let input = skip_whitespace(input);

    // Parse operator.
    let input = skip_whitespace(input);
    let (input, operator) = parse_operator(input)?;

    // Parse value.
    let input = skip_whitespace(input);
    let (input, value) = parse_literal(input)?;

    Ok((
        input,
        Predicate {
            identifier,
            operator,
            value,
        },
    ))
}

/// Parses a SQL-like query from a string input.
///
/// A query is split into two parts: an object tree and a list of predicates. The object tree
/// defines the table/view being queries along with any joined objects (e.g., the `FROM` clause in a
/// SQL statement), while the predicates define conditions for filtering results.
#[tracing::instrument(level = "trace", err)]
pub fn parse<'a>(input: &'a str) -> Result<Query<'a, String, String>, SyntaxError> {
    let input = skip_whitespace(input);
    let (input, object) = parse_object_tree(input)?;

    let mut predicates = Vec::new();
    let mut remaining_input = skip_whitespace(input);

    while !remaining_input.is_empty() {
        let (input, predicate) = parse_predicate(remaining_input)?;
        predicates.push(predicate);
        remaining_input = skip_whitespace(input);
    }

    Ok(Query { object, predicates })
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_log::test;

    fn roundtrip(input: &str) {
        let query = parse(input).unwrap();
        let output = query.to_string();
        assert_eq!(input, output, "Roundtrip failed for input: {}", input);
    }

    #[test]
    fn test_minimal_query() {
        roundtrip("a");
    }

    #[test]
    fn test_query_with_child() {
        roundtrip("a>b");
    }

    #[test]
    fn test_query_with_multiple_children() {
        roundtrip("a>b+c");
    }

    #[test]
    fn test_query_with_nested_children() {
        roundtrip("a>b+c>d");
    }

    // TODO: Add support for moving back up the object hierarchy.
    // #[test]
    // fn test_query_with_multiple_nested_children() {
    //     roundtrip("a>b>c^d");
    // }

    #[test]
    fn test_query_with_predicates() {
        roundtrip("a>b foo=bar baz>42");
        roundtrip("report>param code=visit.edit");
    }
}
