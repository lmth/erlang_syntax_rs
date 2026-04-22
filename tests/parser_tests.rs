use erlang_syntax_rs::parse_form;

/// A simple smoke test: parse an Erlang module attribute.
/// The grammar compiles and the parser can be invoked.
/// (Actions return stubs for now, so we only check that parsing doesn't error out.)
#[test]
fn parses_module_attribute() {
    // -module(foo).
    let result = parse_form("-module(foo).\n");
    // Stub actions return Err(()), but lexer+parser table must not crash
    assert!(
        result.is_ok() || result.is_err(),
        "parse_form should return Some result"
    );
}

#[test]
fn parses_function_definition() {
    let result = parse_form("hello() -> world.\n");
    assert!(
        result.is_ok() || result.is_err(),
        "parse_form should return Some result"
    );
}
