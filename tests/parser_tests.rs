use erlang_syntax_rs::{parse_form, parse_forms, split_forms, Term};

// ────────────────────────────────────────────────────────────────────────────
// Helper: find the first Token whose text matches `needle` anywhere in the tree
fn find_token<'a>(term: &'a Term, needle: &str) -> bool {
    match term {
        Term::Token(t) => t == needle,
        Term::Node { children, .. } => children.iter().any(|c| find_token(c, needle)),
    }
}

fn rule_name(term: &Term) -> Option<&str> {
    match term {
        Term::Node { rule, .. } => Some(rule),
        _ => None,
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Basic smoke tests

#[test]
fn parses_module_attribute() {
    let result = parse_form("-module(foo).\n");
    assert!(result.is_ok(), "Expected Ok, got {:?}", result);
    let term = result.unwrap();
    assert_eq!(rule_name(&term), Some("form"));
    assert!(find_token(&term, "module"));
    assert!(find_token(&term, "foo"));
}

#[test]
fn parses_function_definition() {
    let result = parse_form("hello() -> world.\n");
    assert!(result.is_ok(), "Expected Ok, got {:?}", result);
    let term = result.unwrap();
    assert_eq!(rule_name(&term), Some("form"));
    assert!(find_token(&term, "hello"));
    assert!(find_token(&term, "world"));
}

// ────────────────────────────────────────────────────────────────────────────
// Attribute forms

#[test]
fn parses_vsn_attribute() {
    let res = parse_form("-vsn(1).\n");
    assert!(res.is_ok(), "Expected Ok, got {:?}", res);
    assert!(find_token(&res.unwrap(), "vsn"));
}

#[test]
fn parses_export_attribute() {
    let res = parse_form("-export([foo/1, bar/2]).\n");
    assert!(res.is_ok(), "Expected Ok, got {:?}", res);
}

#[test]
fn parses_define_integer() {
    let res = parse_form("-define(MAX_RETRIES, 3).\n");
    assert!(res.is_ok(), "Expected Ok, got {:?}", res);
    let t = res.unwrap();
    assert!(find_token(&t, "define"));
    assert!(find_token(&t, "MAX_RETRIES"));
    assert!(find_token(&t, "3"));
}

#[test]
fn parses_define_string() {
    let res = parse_form("-define(NAME, \"hello\").\n");
    assert!(res.is_ok(), "Expected Ok, got {:?}", res);
}

// ────────────────────────────────────────────────────────────────────────────
// CST structure checks

#[test]
fn cst_attribute_has_correct_shape() {
    // -module(foo).  ⟹ form[attribute["-","module",attr_val[...]],"."]
    let term = parse_form("-module(foo).\n").unwrap();
    let Term::Node { rule, children } = &term else { panic!("expected Node") };
    assert_eq!(rule, "form");
    assert_eq!(children.len(), 2);
    let Term::Node { rule: attr_rule, .. } = &children[0] else { panic!("expected attribute node") };
    assert_eq!(attr_rule, "attribute");
    // Last child is the dot token
    assert_eq!(children[1], Term::Token(".\n".to_string()));
}

#[test]
fn split_forms_two_forms() {
    let src = "-define(FOO, 1).\n-define(BAR, 2).\n";
    let forms = split_forms(src);
    assert_eq!(forms.len(), 2);
}

#[test]
fn parse_forms_all_ok() {
    let src = "-module(mymod).\n-export([run/0]).\nrun() -> ok.\n";
    let results = parse_forms(src);
    assert_eq!(results.len(), 3);
    for r in &results {
        assert!(r.is_ok(), "Expected Ok but got {:?}", r);
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Function forms

#[test]
fn parses_simple_function() {
    let res = parse_form("add(X, Y) -> X + Y.\n");
    assert!(res.is_ok(), "Expected Ok, got {:?}", res);
}

#[test]
fn parses_function_with_guard() {
    let res = parse_form("abs_val(X) when X >= 0 -> X; abs_val(X) -> -X.\n");
    assert!(res.is_ok(), "Expected Ok, got {:?}", res);
}
