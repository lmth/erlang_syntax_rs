use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-env-changed=OTP_DIR");

    let yrl_path = find_yrl();
    println!("cargo:rerun-if-changed={}", yrl_path.display());

    let yrl_src = std::fs::read_to_string(&yrl_path)
        .unwrap_or_else(|e| panic!("Cannot read {}: {}", yrl_path.display(), e));

    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let grammar_y = out_dir.join("erlang_grammar.y");
    let grammar_rs = out_dir.join("erlang_grammar.y.rs");

    let grammar_src = generate_grammar(&yrl_src);
    std::fs::write(&grammar_y, &grammar_src).unwrap();

    let ct_parser = lrpar::CTParserBuilder::<lrlex::DefaultLexerTypes<u32>>::new()
        .yacckind(cfgrammar::yacc::YaccKind::Grmtools)
        .rust_edition(lrpar::RustEdition::Rust2021)
        .error_on_conflicts(false)
        .warnings_are_errors(false)
        .show_warnings(false)
        .grammar_path(&grammar_y)
        .output_path(&grammar_rs)
        .build()
        .expect("Failed to build parser");

    lrlex::CTLexerBuilder::new()
        .rust_edition(lrlex::RustEdition::Rust2021)
        .rule_ids_map(ct_parser.token_map().clone())
        .lexer_in_src_dir("erlang.l")
        .expect("Failed to set lexer path")
        .allow_missing_terms_in_lexer(true)
        .build()
        .expect("Failed to build lexer");
}

fn find_yrl() -> PathBuf {
    if let Ok(otp_dir) = std::env::var("OTP_DIR") {
        let p = PathBuf::from(otp_dir).join("lib/stdlib/src/erl_parse.yrl");
        if p.exists() {
            return p;
        }
    }

    let output = std::process::Command::new("erl")
        .args(["-noshell", "-eval", "io:put_chars(code:lib_dir(stdlib)), init:stop()"])
        .output();

    if let Ok(out) = output {
        let stdlib_dir = String::from_utf8_lossy(&out.stdout);
        let stdlib_dir = stdlib_dir.trim();
        if !stdlib_dir.is_empty() {
            let p = PathBuf::from(stdlib_dir).join("src/erl_parse.yrl");
            if p.exists() {
                return p;
            }
        }
    }

    panic!("Cannot find erl_parse.yrl. Set OTP_DIR or ensure erl is in PATH.");
}

/// Strip the line-comment from a .yrl line (everything from `%` not inside a quoted atom).
fn strip_eol_comment(line: &str) -> String {
    let mut result = String::new();
    let mut in_quote = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\'' if !in_quote => {
                in_quote = true;
                result.push(c);
            }
            '\'' if in_quote => {
                in_quote = false;
                result.push(c);
            }
            '\\' if in_quote => {
                result.push(c);
                if let Some(next) = chars.next() {
                    result.push(next);
                }
            }
            '%' if !in_quote => break,
            _ => result.push(c),
        }
    }
    result
}

/// True when the original line starts at column 0 with a lowercase ident followed by `->`.
fn is_rule_start(original_line: &str, stripped_trimmed: &str) -> bool {
    let bytes = original_line.as_bytes();
    if bytes.is_empty() {
        return false;
    }
    // Must start at column 0 with a lowercase letter (no leading whitespace)
    if !bytes[0].is_ascii_lowercase() && bytes[0] != b'_' {
        return false;
    }
    // Check stripped content has `identifier ->`
    let ident_end = stripped_trimmed
        .bytes()
        .position(|b| !b.is_ascii_alphanumeric() && b != b'_')
        .unwrap_or(stripped_trimmed.len());
    if ident_end == 0 {
        return false;
    }
    let rest = stripped_trimmed[ident_end..].trim_start();
    rest.starts_with("->")
}

/// Find the position of the `:` that separates RHS from action in a rule,
/// skipping `:` that appear inside single-quoted atoms.
fn find_action_colon(text: &str) -> Option<usize> {
    let mut in_quote = false;
    let mut chars = text.char_indices();
    while let Some((i, c)) = chars.next() {
        match c {
            '\'' if !in_quote => in_quote = true,
            '\'' if in_quote => in_quote = false,
            '\\' if in_quote => {
                chars.next();
            }
            ':' if !in_quote => return Some(i),
            _ => {}
        }
    }
    None
}

/// Tokenise the RHS portion of a rule, returning tokens in .yrl format.
/// Quoted atoms: `'tok'`; bare identifiers: `ident`; `'$empty'` included as-is.
fn tokenize_rhs(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut chars = text.chars().peekable();
    while let Some(&c) = chars.peek() {
        if c.is_whitespace() {
            chars.next();
            continue;
        }
        if c == '\'' {
            chars.next();
            let mut content = String::new();
            loop {
                match chars.next() {
                    None => break,
                    Some('\\') => {
                        content.push('\\');
                        if let Some(next) = chars.next() {
                            content.push(next);
                        }
                    }
                    Some('\'') => break,
                    Some(ch) => content.push(ch),
                }
            }
            tokens.push(format!("'{}'", content));
        } else if c.is_ascii_alphanumeric() || c == '_' || c == '$' {
            let mut ident = String::new();
            while let Some(&ch) = chars.peek() {
                if ch.is_ascii_alphanumeric() || ch == '_' || ch == '$' {
                    ident.push(ch);
                    chars.next();
                } else {
                    break;
                }
            }
            tokens.push(ident);
        } else {
            chars.next();
        }
    }
    tokens
}

/// Parse a single rule text (without trailing `.`) into (lhs, rhs_tokens).
/// Returns None for ssa_check rules or rules referencing ssa_check nonterminals.
fn parse_rule_text(rule_text: &str) -> Option<(String, Vec<String>)> {
    let arrow_pos = rule_text.find("->")?;
    let lhs = rule_text[..arrow_pos].trim().to_string();

    if lhs.starts_with("ssa_check") {
        return None;
    }

    let after_arrow = &rule_text[arrow_pos + 2..];
    let colon_pos = find_action_colon(after_arrow)?;
    let rhs_text = &after_arrow[..colon_pos];

    let rhs = tokenize_rhs(rhs_text);

    // Drop rules that reference ssa_check nonterminals
    if rhs.iter().any(|t| t.starts_with("ssa_check")) {
        return None;
    }

    Some((lhs, rhs))
}

/// Parse bare terminal names (unquoted identifiers) from the Terminals section.
fn parse_bare_terminals(src: &str) -> HashSet<String> {
    let mut in_terminals = false;
    let mut result = HashSet::new();

    for line in src.lines() {
        let stripped = strip_eol_comment(line);
        let trimmed = stripped.trim();

        if trimmed == "Terminals" || trimmed.starts_with("Terminals ") {
            in_terminals = true;
            let rest = trimmed
                .strip_prefix("Terminals")
                .unwrap_or("")
                .trim()
                .trim_end_matches('.');
            collect_bare_from(rest, &mut result);
            if stripped.trim().ends_with('.') {
                in_terminals = false;
            }
            continue;
        }

        if in_terminals {
            let content = trimmed.trim_end_matches('.');
            collect_bare_from(content, &mut result);
            if stripped.trim().ends_with('.') {
                in_terminals = false;
            }
        }
    }

    // Remove the ssa% pseudo-terminal
    result.remove("%ssa%");
    result
}

fn collect_bare_from(text: &str, result: &mut HashSet<String>) {
    let mut chars = text.chars().peekable();
    while let Some(&c) = chars.peek() {
        if c.is_whitespace() {
            chars.next();
        } else if c == '\'' {
            // Skip quoted terminal
            chars.next();
            while let Some(ch) = chars.next() {
                if ch == '\'' {
                    break;
                }
                if ch == '\\' {
                    chars.next();
                }
            }
        } else if c.is_ascii_lowercase() || c == '_' {
            let mut name = String::new();
            while let Some(&ch) = chars.peek() {
                if ch.is_ascii_alphanumeric() || ch == '_' {
                    name.push(ch);
                    chars.next();
                } else {
                    break;
                }
            }
            result.insert(name);
        } else {
            chars.next();
        }
    }
}

/// Convert a single RHS token (in .yrl format) to .y (Grmtools) format.
fn convert_token(token: &str, bare_terminals: &HashSet<String>) -> String {
    if token.starts_with('\'') && token.ends_with('\'') && token.len() >= 2 {
        // Single-quoted terminal -> double-quoted
        let inner = &token[1..token.len() - 1];
        format!("\"{}\"", inner)
    } else if bare_terminals.contains(token) {
        // Bare terminal (char, integer, dot, etc.) -> double-quoted
        format!("\"{}\"", token)
    } else {
        // Nonterminal - bare identifier
        token.to_string()
    }
}

/// True if a .yrl token (quoted or bare) is a terminal.
fn is_terminal_tok(token: &str, bare_terminals: &HashSet<String>) -> bool {
    if token == "'$empty'" || token == "$empty" {
        return false;
    }
    (token.starts_with('\'') && token.ends_with('\'') && token.len() >= 2)
        || bare_terminals.contains(token)
}

/// Generate the Rust action code block (without braces) for a rule.
///
/// Terminals become `crate::Term::token(...)` leaves; nonterminals are
/// propagated with `?`.  Empty productions return an empty node.
fn generate_action(lhs: &str, rhs: &[String], bare_terminals: &HashSet<String>) -> String {
    let real_rhs: Vec<&String> = rhs
        .iter()
        .filter(|t| **t != "'$empty'" && **t != "$empty")
        .collect();

    if real_rhs.is_empty() {
        return format!("Ok(crate::Term::node(\"{lhs}\", vec![]))");
    }

    let mut parts = Vec::new();
    for (i, tok) in real_rhs.iter().enumerate() {
        let n = i + 1;
        if is_terminal_tok(tok, bare_terminals) {
            parts.push(format!(
                "crate::Term::token(__gt_lexer.span_str(\
                 __gt_arg_{n}.as_ref().unwrap_or_else(|e| e).span()))"
            ));
        } else {
            parts.push(format!("__gt_arg_{n}?"));
        }
    }
    format!(
        "Ok(crate::Term::node(\"{lhs}\", vec![{}]))",
        parts.join(", ")
    )
}

/// Format a RHS token list for output, returning a string ending with a space
/// (or empty string for empty productions).
fn format_rhs(rhs: &[String], bare_terminals: &HashSet<String>) -> String {
    let parts: Vec<String> = rhs
        .iter()
        .filter(|t| *t != "'$empty'" && **t != "$empty")
        .map(|t| convert_token(t, bare_terminals))
        .collect();
    if parts.is_empty() {
        String::new()
    } else {
        format!("{} ", parts.join(" "))
    }
}

/// Determine if a rule needs a %prec annotation.
fn needs_prec(lhs: &str, rhs: &[String]) -> Option<&'static str> {
    let has = |s: &str| rhs.iter().any(|t| t == s);

    match lhs {
        "expr" => {
            if rhs.len() == 2 && !rhs.is_empty() && rhs[0] == "'catch'" {
                return Some("PREC_CATCH");
            }
            if rhs.len() == 2 && has("prefix_op") {
                return Some("PREC_PREFIX");
            }
            if rhs.len() == 3 && has("comp_op") {
                return Some("PREC_COMP");
            }
            if rhs.len() == 3 && has("list_op") {
                return Some("PREC_LIST");
            }
            if rhs.len() == 3 && has("add_op") {
                return Some("PREC_ADD");
            }
            if rhs.len() == 3 && has("mult_op") {
                return Some("PREC_MULT");
            }
        }
        "type" => {
            if rhs.len() == 2 && has("prefix_op") {
                return Some("PREC_PREFIX");
            }
            if rhs.len() == 3 && has("add_op") {
                return Some("PREC_ADD");
            }
            if rhs.len() == 3 && has("mult_op") {
                return Some("PREC_MULT");
            }
        }
        "pat_expr" => {
            if rhs.len() == 2 && has("prefix_op") {
                return Some("PREC_PREFIX");
            }
            if rhs.len() == 3 && has("comp_op") {
                return Some("PREC_COMP");
            }
            if rhs.len() == 3 && has("list_op") {
                return Some("PREC_LIST");
            }
            if rhs.len() == 3 && has("add_op") {
                return Some("PREC_ADD");
            }
            if rhs.len() == 3 && has("mult_op") {
                return Some("PREC_MULT");
            }
        }
        _ => {}
    }
    None
}

/// Parse all grammar rules from the .yrl source, filtering out ssa_check rules.
fn parse_rules(src: &str) -> Vec<(String, Vec<String>)> {
    let mut rules = Vec::new();
    let mut current: Vec<String> = Vec::new();
    let mut in_rules = false;

    for line in src.lines() {
        let stripped = strip_eol_comment(line);
        let trimmed = stripped.trim();

        // Stop at the Header or Erlang code sections
        if trimmed == "Erlang code." || trimmed.starts_with("Erlang code.") || trimmed == "Header" {
            break;
        }

        if trimmed.is_empty() {
            continue;
        }

        // Detect start of rules section (first line that looks like a rule)
        if !in_rules {
            if is_rule_start(line, trimmed) {
                in_rules = true;
            } else {
                continue;
            }
        }

        // Starting a new rule?
        if is_rule_start(line, trimmed) && !current.is_empty() {
            // Finalize the previous accumulated rule
            let rule_text = current.join(" ");
            let rule_text = rule_text.trim_end_matches(|c: char| c == '.' || c.is_whitespace());
            if let Some(parsed) = parse_rule_text(rule_text) {
                rules.push(parsed);
            }
            current.clear();
        }

        current.push(trimmed.to_string());

        // Check if this line completes the rule (ends with `.`)
        if trimmed.ends_with('.') {
            let rule_text = current.join(" ");
            let rule_text = rule_text.trim_end_matches(|c: char| c == '.' || c.is_whitespace());
            if let Some(parsed) = parse_rule_text(rule_text) {
                rules.push(parsed);
            }
            current.clear();
        }
    }

    // Handle any remaining accumulated rule
    if !current.is_empty() {
        let rule_text = current.join(" ");
        let rule_text = rule_text.trim_end_matches(|c: char| c == '.' || c.is_whitespace());
        if let Some(parsed) = parse_rule_text(rule_text) {
            rules.push(parsed);
        }
    }

    rules
}

fn generate_grammar(src: &str) -> String {
    let bare_terminals = parse_bare_terminals(src);
    let rules = parse_rules(src);

    // Group rules by LHS, preserving order of first occurrence
    let mut order: Vec<String> = Vec::new();
    let mut grouped: BTreeMap<String, Vec<Vec<String>>> = BTreeMap::new();
    for (lhs, rhs) in &rules {
        if !grouped.contains_key(lhs) {
            order.push(lhs.clone());
        }
        grouped.entry(lhs.clone()).or_default().push(rhs.clone());
    }

    let mut out = String::new();

    out.push_str("%start form\n\n");
    out.push_str("/* Operator precedence virtual tokens */\n");
    out.push_str("%nonassoc PREC_CATCH\n");
    out.push_str("%right \"=\" \"!\"\n");
    out.push_str("%right \"orelse\" \"::\"\n");
    out.push_str("%right \"andalso\"\n");
    out.push_str("%left \"|\"\n");
    out.push_str(
        "%nonassoc PREC_COMP \"==\" \"/=\" \"=<\" \"<\" \">=\" \">\" \"=:=\" \"=/=\" \"..\"\n",
    );
    out.push_str("%right PREC_LIST \"++\" \"--\"\n");
    out.push_str(
        "%left PREC_ADD \"+\" \"-\" \"bor\" \"bxor\" \"bsl\" \"bsr\" \"or\" \"xor\"\n",
    );
    out.push_str("%left PREC_MULT \"/\" \"*\" \"div\" \"rem\" \"band\" \"and\"\n");
    out.push_str("%right PREC_PREFIX\n");
    out.push_str("%nonassoc \"#\"\n");
    out.push_str("%nonassoc \":\"\n");
    out.push_str("%nonassoc PREC_CLAUSE_BODY\n");
    out.push_str("\n%%\n");

    for lhs in &order {
        let alts = match grouped.get(lhs) {
            Some(a) => a,
            None => continue,
        };

        out.push_str(&format!("{} -> Result<crate::Term, ()>:\n", lhs));
        for (i, rhs) in alts.iter().enumerate() {
            let prefix = if i == 0 { "    " } else { "  | " };
            let rhs_str = format_rhs(rhs, &bare_terminals);
            let action = generate_action(lhs, rhs, &bare_terminals);
            if let Some(prec) = needs_prec(lhs, rhs) {
                out.push_str(&format!(
                    "{}{}%prec {} {{ {} }}\n",
                    prefix, rhs_str, prec, action
                ));
            } else {
                out.push_str(&format!("{}{}{{ {} }}\n", prefix, rhs_str, action));
            }
        }
        out.push_str("  ;\n");
    }

    out
}
