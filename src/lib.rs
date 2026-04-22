use lrlex::lrlex_mod;
use lrpar::lrpar_mod;

lrlex_mod!("erlang.l");
lrpar_mod!("erlang_grammar.y");

pub mod anno;
pub mod ast;

pub use anno::Anno;
pub use ast::Term;

/// Parse a single Erlang form from source text.
pub fn parse_form(input: &str) -> Result<Term, String> {
    let lexerdef = erlang_l::lexerdef();
    let lexer = lexerdef.lexer(input);
    let (res, errs) = erlang_grammar_y::parse(&lexer);
    if !errs.is_empty() {
        return Err(format!("{:?}", errs));
    }
    match res {
        Some(Ok(_)) => Ok(Term::Nil),
        Some(Err(())) => Err("parse error".to_string()),
        None => Err("no result".to_string()),
    }
}
