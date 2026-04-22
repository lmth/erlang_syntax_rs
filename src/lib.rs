use lrlex::lrlex_mod;
use lrpar::lrpar_mod;

lrlex_mod!("erlang.l");
lrpar_mod!("erlang_grammar.y");

pub mod anno;
pub mod ast;

pub use anno::Anno;
pub use ast::Term;

/// Parse a single Erlang form (terminated by `.`) from `input`.
///
/// Returns the CST root on success, or an error string if lexing/parsing
/// fails.  The input should end with `.` followed by whitespace,
/// e.g. `"-module(foo).\n"`.
pub fn parse_form(input: &str) -> Result<Term, String> {
    let lexerdef = erlang_l::lexerdef();
    let lexer = lexerdef.lexer(input);
    let (res, errs) = erlang_grammar_y::parse(&lexer);
    if !errs.is_empty() {
        let msg = errs
            .iter()
            .map(|e| format!("{}", e.pp(&lexer, &erlang_grammar_y::token_epp)))
            .collect::<Vec<_>>()
            .join("; ");
        return Err(msg);
    }
    match res {
        Some(Ok(term)) => Ok(term),
        Some(Err(())) => Err("parse error (grammar action failed)".to_string()),
        None => Err("no parse result".to_string()),
    }
}

/// Split `input` into individual Erlang form source strings.
///
/// Forms are delimited by `.\n` (a dot at end of line).  The returned strings
/// each include the trailing dot and newline so that `parse_form` can process
/// them directly.
pub fn split_forms(input: &str) -> Vec<&str> {
    let mut forms = Vec::new();
    let mut start = 0;
    let bytes = input.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    while i < len {
        // Look for '.' at end of "logical line": '.' followed by whitespace or EOF
        if bytes[i] == b'.' && (i + 1 >= len || bytes[i + 1].is_ascii_whitespace()) {
            let end = (i + 1).min(len);
            let piece = input[start..end].trim();
            if !piece.is_empty() {
                forms.push(&input[start..end]);
            }
            start = i + 1;
        }
        i += 1;
    }
    // Remaining text after last dot
    let tail = input[start..].trim();
    if !tail.is_empty() {
        forms.push(&input[start..]);
    }
    forms
}

/// Parse all forms in `input`.
pub fn parse_forms(input: &str) -> Vec<Result<Term, String>> {
    split_forms(input)
        .into_iter()
        .map(|s| {
            let with_nl = format!("{}\n", s.trim_end());
            parse_form(&with_nl)
        })
        .collect()
}
