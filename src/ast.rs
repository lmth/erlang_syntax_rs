/// Erlang Concrete Syntax Tree node.
///
/// Grammar actions produce a CST where every rule reduction creates a `Node`
/// and every matched terminal becomes a `Token`.  Higher-level semantic types
/// (attributes, function definitions, etc.) can be derived by walking the CST.
#[derive(Debug, Clone, PartialEq)]
pub enum Term {
    /// A terminal token (the matched source text).
    Token(String),
    /// A grammar rule reduction node.
    Node {
        /// The name of the grammar rule (LHS nonterminal).
        rule: String,
        /// The matched sub-trees in left-to-right order.
        children: Vec<Term>,
    },
}

impl Term {
    /// Construct a token leaf.
    pub fn token(text: &str) -> Self {
        Term::Token(text.to_string())
    }

    /// Construct a rule node.
    pub fn node(rule: &str, children: Vec<Term>) -> Self {
        Term::Node {
            rule: rule.to_string(),
            children,
        }
    }

    /// Return the token text if this is a `Token`, otherwise `None`.
    pub fn as_token(&self) -> Option<&str> {
        match self {
            Term::Token(s) => Some(s.as_str()),
            Term::Node { .. } => None,
        }
    }

    /// Return `(rule, children)` if this is a `Node`, otherwise `None`.
    pub fn as_node(&self) -> Option<(&str, &[Term])> {
        match self {
            Term::Node { rule, children } => Some((rule.as_str(), children.as_slice())),
            Term::Token(_) => None,
        }
    }

    /// Return the rule name if this is a `Node`, otherwise `None`.
    pub fn rule(&self) -> Option<&str> {
        self.as_node().map(|(r, _)| r)
    }

    /// Return the Nth child (0-indexed), or `None`.
    pub fn child(&self, n: usize) -> Option<&Term> {
        self.as_node().and_then(|(_, ch)| ch.get(n))
    }

    /// Collapse chains of single-child nodes, returning the innermost term.
    ///
    /// Useful for navigating deeply nested CST paths like
    /// `expr → expr_max → expr_remote → expr_max → atomic → integer`.
    pub fn unwrap_single_child(&self) -> &Term {
        match self {
            Term::Node { children, .. } if children.len() == 1 => {
                children[0].unwrap_single_child()
            }
            _ => self,
        }
    }

    /// Return the first `Token` reachable by always following the first child.
    ///
    /// Useful for extracting the atom/var/literal text from a deeply nested
    /// expression sub-tree.
    pub fn first_token(&self) -> Option<&str> {
        match self {
            Term::Token(s) => Some(s.as_str()),
            Term::Node { children, .. } => children.first()?.first_token(),
        }
    }

    /// Collect all `Token` leaves in left-to-right order.
    pub fn tokens(&self) -> Vec<&str> {
        let mut result = Vec::new();
        self.collect_tokens(&mut result);
        result
    }

    fn collect_tokens<'a>(&'a self, out: &mut Vec<&'a str>) {
        match self {
            Term::Token(s) => out.push(s.as_str()),
            Term::Node { children, .. } => {
                for ch in children {
                    ch.collect_tokens(out);
                }
            }
        }
    }
}

