//! `hrl2rust` — Convert Erlang `.hrl` header defines to Rust constants.
//!
//! For each `-define(NAME, VALUE).` in the header where VALUE is a simple
//! literal (integer, float, atom, or string), emit a Rust `pub const`.
//! Complex or non-literal defines are emitted as a `// SKIP:` comment.
//!
//! # Usage
//! ```
//! cargo run --example hrl2rust -- path/to/file.hrl
//! ```

use erlang_syntax_rs::{Term, parse_forms, split_forms};
use std::path::PathBuf;

fn main() {
    let path: PathBuf = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            eprintln!("Usage: hrl2rust <file.hrl>");
            std::process::exit(1);
        });

    let src = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        eprintln!("Error reading {}: {}", path.display(), e);
        std::process::exit(1);
    });

    println!("// Generated from {}", path.display());
    println!("// by erlang-syntax-rs hrl2rust");
    println!();

    // Check there are any forms
    let raw_forms = split_forms(&src);
    if raw_forms.is_empty() {
        println!("// (no forms found)");
        return;
    }

    let forms = parse_forms(&src);
    for result in forms {
        match result {
            Err(e) => {
                println!("// PARSE ERROR: {}", e.lines().next().unwrap_or(""));
            }
            Ok(tree) => {
                process_form(&tree);
            }
        }
    }
}

/// Try to extract a define from a form CST and emit a Rust constant.
///
/// CST shape for `-define(FOO, 42).`:
/// ```
/// Node("form", [
///   Node("attribute", [
///     Token("-"),
///     Token("define"),
///     Node("attr_val", [
///       Token("("),
///       Node("expr", [...Token("FOO")...]),
///       Token(","),
///       Node("exprs", [...Token("42")...]),
///       Token(")")
///     ])
///   ]),
///   Token(".")
/// ])
/// ```
fn process_form(form: &Term) {
    let Some(("form", form_ch)) = form.as_node() else {
        return;
    };
    let Some(attr) = form_ch.first() else { return };
    let Some(("attribute", attr_ch)) = attr.as_node() else {
        return;
    };

    // attr_ch: [Token("-"), Token(name), Node("attr_val", ...)]
    if attr_ch.len() < 3 {
        return;
    }
    let attr_name = match attr_ch[1].as_token() {
        Some(n) => n,
        None => return,
    };

    if attr_name != "define" {
        return;
    }

    let Some(("attr_val", av_ch)) = attr_ch[2].as_node() else {
        return;
    };

    // attr_val has shape: Token("("), expr(name), Token(","), exprs(value), Token(")")
    // or (for zero-argument macros): Token("("), expr(name), Token(")")
    if av_ch.len() < 3 {
        return;
    }

    // The name is av_ch[1] (expr containing var/atom)
    let macro_name = match av_ch[1].unwrap_single_child().as_token() {
        Some(t) => t.to_string(),
        None => {
            // Might still be nested — try first_token
            match av_ch[1].first_token() {
                Some(t) => t.to_string(),
                None => return,
            }
        }
    };

    // The value is av_ch[3] if there's a comma (av_ch[2] == ",")
    let has_value = av_ch.len() >= 5 && av_ch[2].as_token() == Some(",");
    if !has_value {
        // Zero-argument macro / macro with no value → skip
        println!("// SKIP: -define({macro_name}, ...)  [no value]");
        return;
    }

    let value_node = &av_ch[3];
    let value_leaf = value_node.unwrap_single_child();

    match value_leaf.as_token() {
        Some(text) => emit_const(&macro_name, text),
        None => {
            // More complex expression — emit skip comment
            let tokens = value_node.tokens();
            println!("// SKIP: -define({macro_name}, {})", tokens.join(" "));
        }
    }
}

fn emit_const(name: &str, value: &str) {
    // Determine Rust type from the value text
    if let Ok(i) = value.parse::<i64>() {
        let rust_name = to_screaming_snake(name);
        println!("pub const {rust_name}: i64 = {i};");
    } else if let Ok(f) = value.parse::<f64>() {
        let rust_name = to_screaming_snake(name);
        println!("pub const {rust_name}: f64 = {f};");
    } else if value.starts_with('"') {
        // String literal
        let rust_name = to_screaming_snake(name);
        println!("pub const {rust_name}: &str = {value};");
    } else if value.starts_with('$') {
        // Char literal: $a or $\n etc.
        let rust_name = to_screaming_snake(name);
        println!("// SKIP: -define({name}, {value})  [char literal — handle manually]");
        let _ = rust_name;
    } else {
        // Atom — emit as &str constant
        let rust_name = to_screaming_snake(name);
        let atom_val = if value.starts_with('\'') {
            // quoted atom: strip outer quotes
            &value[1..value.len() - 1]
        } else {
            value
        };
        println!("pub const {rust_name}: &str = \"{atom_val}\";");
    }
}

/// Convert an Erlang identifier (CamelCase, snake_case, or UPPER) to
/// SCREAMING_SNAKE_CASE for Rust const names.
fn to_screaming_snake(s: &str) -> String {
    // If already SCREAMING_SNAKE_CASE (all uppercase, digits, underscores), return as-is.
    if s.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_') {
        return s.to_string();
    }
    // Convert camelCase / lower_case to SCREAMING_SNAKE_CASE.
    let mut result = String::new();
    let mut prev_upper = false;
    let mut prev_underscore = false;
    for (i, ch) in s.char_indices() {
        if ch == '_' {
            result.push('_');
            prev_upper = false;
            prev_underscore = true;
        } else if ch.is_ascii_uppercase() {
            if i > 0 && !prev_upper && !prev_underscore {
                result.push('_');
            }
            result.push(ch);
            prev_upper = true;
            prev_underscore = false;
        } else {
            result.push(ch.to_ascii_uppercase());
            prev_upper = false;
            prev_underscore = false;
        }
    }
    result
}

