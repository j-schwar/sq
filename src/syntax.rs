#[derive(Debug, Clone, PartialEq, Eq)]
struct ObjectProjection {
    object: String,
    projection: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ObjectTree {
    root: ObjectProjection,
    children: Vec<ObjectTree>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Op {
    Eq,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Expr {
    StringLiteral(String),
    IntLiteral(i64),
    Identifier {
        object: Option<String>,
        field: String,
    },
    BinaryOp {
        left: Box<Expr>,
        op: Op,
        right: Box<Expr>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Query {
    object: ObjectTree,
    exprs: Vec<Expr>,
}

#[derive(Debug)]
pub struct SyntaxError;

impl std::fmt::Display for SyntaxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "syntax error")
    }
}

impl std::error::Error for SyntaxError {}

pub type ParseResult<'a, T> = std::result::Result<(&'a str, T), SyntaxError>;

fn skip_whitespace(input: &str) -> &str {
    input.trim_start()
}

#[tracing::instrument(level = "trace", ret)]
fn parse_identifier(input: &str) -> ParseResult<String> {
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

/*
    a>b
    parse a -> >b
    >

    a>b+c>d
    a>b>c+d

    a
    - b
    - c
      - d
*/

// fn parse_object_tree_children(children: &mut Vec<ObjectTree>, input: &str) -> ParseResult<&str> {
//     let input = skip_whitespace(input);
//     if input.starts_with('+') {

//     }
// }

#[tracing::instrument(level = "trace", ret)]
fn parse_object_tree(input: &str) -> ParseResult<ObjectTree> {
    #[tracing::instrument(level = "trace", ret, skip(parent))]
    fn parse_tree<'a>(parent: &mut ObjectTree, input: &'a str) -> ParseResult<'a, ()> {
        let input = skip_whitespace(input);
        if input.is_empty() {
            return Ok((input, ()));
        }

        let (input, object) = parse_identifier(input)?;
        let mut tree = ObjectTree {
            root: ObjectProjection {
                object: object.clone(),
                projection: None,
            },
            children: Vec::new(),
        };

        let mut input = skip_whitespace(input);
        if input.is_empty() {
            parent.children.push(tree);
            return Ok((input, ()));
        }

        match input.chars().next() {
            Some('>') => {
                input = &input[1..];
                let (next_input, _) = parse_tree(&mut tree, input)?;
                input = next_input;
            }

            Some('+') => {
                input = &input[1..];

                // Stop parsing current tree, add it to the parent and parse a sibling.
                parent.children.push(tree);
                let (next_input, _) = parse_tree(parent, input)?;
                input = next_input;
            }

            _ => todo!(),
        }

        Ok((input, ()))
    }

    let input = skip_whitespace(input);
    let (input, object) = parse_identifier(input)?;
    let mut root_tree = ObjectTree {
        root: ObjectProjection {
            object: object.to_string(),
            projection: None,
        },
        children: Vec::new(),
    };

    let input = skip_whitespace(input);
    if input.chars().next() != Some('>') {
        return Ok((input, root_tree));
    }

    let input = &input[1..]; // Skip the '>'
    let (input, _) = parse_tree(&mut root_tree, input)?;
    Ok((input, root_tree))
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_log::test;

    #[test]
    fn test_parse_identifier() {
        let input = "testIdentifier123";
        let (remaining, identifier) = parse_identifier(input).unwrap();
        assert_eq!(identifier, "testIdentifier123");
        assert_eq!(remaining, "");
    }

    #[test]
    fn test_parse_identifier_with_invalid_characters() {
        let input = "testIdentifier123!@#";
        let (remaining, identifier) = parse_identifier(input).unwrap();
        assert_eq!(identifier, "testIdentifier123");
        assert_eq!(remaining, "!@#");
    }

    #[test]
    fn test_parse_identifier_empty() {
        let input = "";
        assert!(parse_identifier(input).is_err());
    }

    #[test]
    fn test_parse_object_tree() {
        let input = "report";
        let (remaining, object_tree) = parse_object_tree(input).unwrap();
        assert_eq!(remaining, "");
        assert_eq!(
            object_tree,
            ObjectTree {
                root: ObjectProjection {
                    object: "report".to_string(),
                    projection: None,
                },
                children: vec![],
            }
        );
    }

    #[test]
    fn test_parse_object_tree_with_children() {
        let input = "report>param";
        let (remaining, object_tree) = parse_object_tree(input).unwrap();
        assert_eq!(remaining, "");
        assert_eq!(
            object_tree,
            ObjectTree {
                root: ObjectProjection {
                    object: "report".to_string(),
                    projection: None,
                },
                children: vec![ObjectTree {
                    root: ObjectProjection {
                        object: "param".to_string(),
                        projection: None,
                    },
                    children: vec![],
                }],
            }
        );
    }

    #[test]
    fn test_parse_object_tree_with_multiple_children() {
        let input = "report>param+data";
        let (remaining, object_tree) = parse_object_tree(input).unwrap();
        assert_eq!(remaining, "");
        assert_eq!(
            object_tree,
            ObjectTree {
                root: ObjectProjection {
                    object: "report".to_string(),
                    projection: None,
                },
                children: vec![
                    ObjectTree {
                        root: ObjectProjection {
                            object: "param".to_string(),
                            projection: None,
                        },
                        children: vec![],
                    },
                    ObjectTree {
                        root: ObjectProjection {
                            object: "data".to_string(),
                            projection: None,
                        },
                        children: vec![],
                    }
                ],
            }
        );
    }

    #[test]
    fn test_parse_object_tree_with_nested_children() {
        let input = "report>param+data>source";
        let (remaining, object_tree) = parse_object_tree(input).unwrap();
        assert_eq!(remaining, "");
        assert_eq!(
            object_tree,
            ObjectTree {
                root: ObjectProjection {
                    object: "report".to_string(),
                    projection: None,
                },
                children: vec![
                    ObjectTree {
                        root: ObjectProjection {
                            object: "param".to_string(),
                            projection: None,
                        },
                        children: vec![],
                    },
                    ObjectTree {
                        root: ObjectProjection {
                            object: "data".to_string(),
                            projection: None,
                        },
                        children: vec![ObjectTree {
                            root: ObjectProjection {
                                object: "source".to_string(),
                                projection: None,
                            },
                            children: vec![],
                        }],
                    }
                ],
            }
        );
    }
}
