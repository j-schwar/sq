#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SqlOp {
    Eq,
    Neq,
    Gt,
    Gte,
    Lt,
    Lte,
    And,
    Or,
    Like,
    IsNull,
    IsNotNull,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SqlFieldRef {
    pub object: String,
    pub field: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SqlExpr {
    Null,
    StringLiteral(String),
    IntLiteral(i64),
    Ref(SqlFieldRef),
    BinaryOp {
        left: Box<SqlExpr>,
        op: SqlOp,
        right: Box<SqlExpr>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SqlObjectRef {
    pub object: String,
    pub alias: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SqlJoinType {
    Inner,
    Left,
    Right,
    Outer,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SqlJoin {
    pub join_type: SqlJoinType,
    pub object: SqlObjectRef,
    pub on: SqlExpr,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SqlFromClause {
    pub object: SqlObjectRef,
    pub joins: Vec<SqlJoin>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SqlQuery {
    pub projection: Vec<SqlFieldRef>,
    pub from: SqlFromClause,
    pub where_clause: Option<SqlExpr>,
}

pub trait SqlDialect {
    fn query(&self, query: &SqlQuery) -> String {
        let mut sql = String::new();
        sql.push_str("SELECT ");
        sql.push_str(&self.projection(&query.projection));
        sql.push_str(" FROM ");
        sql.push_str(&self.from(&query.from));

        if let Some(ref where_expr) = query.where_clause {
            sql.push_str(" WHERE ");
            sql.push_str(&self.expr(where_expr));
        }

        sql
    }

    fn projection(&self, projection: &[SqlFieldRef]) -> String {
        projection
            .iter()
            .map(|field| self.field_ref(field))
            .collect::<Vec<_>>()
            .join(", ")
    }

    fn from(&self, from: &SqlFromClause) -> String {
        let mut sql = self.object_ref(&from.object);
        for join in &from.joins {
            sql.push_str(" ");
            sql.push_str(&self.join(join));
        }

        sql
    }

    fn join(&self, join: &SqlJoin) -> String {
        let join_type = match join.join_type {
            SqlJoinType::Inner => "INNER JOIN",
            SqlJoinType::Left => "LEFT JOIN",
            SqlJoinType::Right => "RIGHT JOIN",
            SqlJoinType::Outer => "OUTER JOIN",
        };

        format!(
            "{} {} ON {}",
            join_type,
            self.object_ref(&join.object),
            self.expr(&join.on)
        )
    }

    fn expr(&self, expr: &SqlExpr) -> String {
        match expr {
            SqlExpr::Null => "NULL".to_string(),
            SqlExpr::StringLiteral(s) => format!("'{}'", s),
            SqlExpr::IntLiteral(i) => i.to_string(),
            SqlExpr::Ref(field_ref) => self.field_ref(field_ref),
            SqlExpr::BinaryOp { left, op, right } => self.binary_op(left, *op, right),
        }
    }

    fn field_ref(&self, r: &SqlFieldRef) -> String {
        format!(
            "{}.{}",
            self.identifier(&r.object),
            self.identifier(&r.field)
        )
    }

    fn binary_op(&self, left: &SqlExpr, op: SqlOp, right: &SqlExpr) -> String {
        let op_str = match op {
            SqlOp::Eq => "=",
            SqlOp::Neq => "<>",
            SqlOp::Gt => ">",
            SqlOp::Gte => ">=",
            SqlOp::Lt => "<",
            SqlOp::Lte => "<=",
            SqlOp::And => "AND",
            SqlOp::Or => "OR",
            SqlOp::Like => "LIKE",
            SqlOp::IsNull => "IS NULL",
            SqlOp::IsNotNull => "IS NOT NULL",
        };

        format!("{} {} {}", self.expr(left), op_str, self.expr(right))
    }

    fn object_ref(&self, object: &SqlObjectRef) -> String {
        format!(
            "{} AS {}",
            self.identifier(&object.object),
            self.identifier(&object.alias)
        )
    }

    fn identifier(&self, ident: &str) -> String {
        format!("{}", ident)
    }
}

#[derive(Default, Debug, Clone, Copy)]
pub struct Sql;

impl SqlDialect for Sql {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_query() {
        let query = SqlQuery {
            projection: vec![
                SqlFieldRef {
                    object: "u".to_string(),
                    field: "id".to_string(),
                },
                SqlFieldRef {
                    object: "u".to_string(),
                    field: "name".to_string(),
                },
            ],
            from: SqlFromClause {
                object: SqlObjectRef {
                    object: "users".to_string(),
                    alias: "u".to_string(),
                },
                joins: vec![],
            },
            where_clause: Some(SqlExpr::BinaryOp {
                left: Box::new(SqlExpr::Ref(SqlFieldRef {
                    object: "u".to_string(),
                    field: "active".to_string(),
                })),
                op: SqlOp::Eq,
                right: Box::new(SqlExpr::IntLiteral(1)),
            }),
        };

        let sql = Sql.query(&query);
        assert_eq!(
            sql,
            "SELECT u.id, u.name FROM users AS u WHERE u.active = 1"
        );
    }

    #[test]
    fn test_query_with_join() {
        let query = SqlQuery {
            projection: vec![
                SqlFieldRef {
                    object: "u".to_string(),
                    field: "id".to_string(),
                },
                SqlFieldRef {
                    object: "p".to_string(),
                    field: "title".to_string(),
                },
            ],
            from: SqlFromClause {
                object: SqlObjectRef {
                    object: "users".to_string(),
                    alias: "u".to_string(),
                },
                joins: vec![SqlJoin {
                    join_type: SqlJoinType::Left,
                    object: SqlObjectRef {
                        object: "posts".to_string(),
                        alias: "p".to_string(),
                    },
                    on: SqlExpr::BinaryOp {
                        left: Box::new(SqlExpr::Ref(SqlFieldRef {
                            object: "u".to_string(),
                            field: "id".to_string(),
                        })),
                        op: SqlOp::Eq,
                        right: Box::new(SqlExpr::Ref(SqlFieldRef {
                            object: "p".to_string(),
                            field: "user_id".to_string(),
                        })),
                    },
                }],
            },
            where_clause: None,
        };

        let sql = Sql.query(&query);
        assert_eq!(
            sql,
            "SELECT u.id, p.title FROM users AS u LEFT JOIN posts AS p ON u.id = p.user_id"
        );
    }
}
