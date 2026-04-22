use crate::Anno;

/// Erlang term / abstract syntax node.
/// Mirrors the Erlang abstract format as returned by erl_parse:parse_form/1.
#[derive(Debug, Clone, PartialEq)]
pub enum Term {
    Nil,
    Integer(i64),
    Float(f64),
    Atom(Anno, String),
    Var(Anno, String),
    String(Anno, String),
    Char(Anno, char),
    Tuple(Anno, Vec<Term>),
    List(Anno, Vec<Term>, Box<Term>),
    // Extend as actions are implemented
}
