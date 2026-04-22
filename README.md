# erlang-syntax-rs

A trustworthy Erlang source parser in Rust, derived directly from OTP's
[`erl_parse.yrl`](https://github.com/erlang/otp/blob/master/lib/stdlib/src/erl_parse.yrl).

## Approach

Rather than hand-writing an Erlang grammar, `build.rs` locates `erl_parse.yrl`
in the installed OTP distribution, extracts and translates the grammar to
[Grmtools](https://github.com/softdevteam/grmtools) format, and drives
`lrpar`/`lrlex` to generate an LALR(1) parser at compile time.  This means the
parser always tracks the real Erlang grammar — no drift, no reimplementation.

```
erl_parse.yrl   ──build.rs──▶   erlang_grammar.y   ──lrpar──▶   parser tables
erlang.l        ──lrlex──────────────────────────────────────▶   lexer tables
```

The result of parsing is a **Concrete Syntax Tree** (CST) composed of two node
kinds:

```rust
pub enum Term {
    Token(String),                          // a matched terminal (leaf)
    Node { rule: String, children: Vec<Term> }, // a rule reduction (branch)
}
```

Every grammar rule returns `Result<Term, ()>`, so the full parse tree is
available for any downstream analysis without committing to a specific semantic
AST up front.

## Prerequisites

An OTP installation with `erl_parse.yrl` accessible.  `build.rs` finds it by:

1. Checking `$OTP_DIR/lib/stdlib/src/erl_parse.yrl`, or
2. Invoking `erl -noshell -eval 'io:put_chars(code:lib_dir(stdlib)), init:stop()'`
   and appending `src/erl_parse.yrl`.

Any standard OTP installation (≥ OTP 24) will work.

## Usage

Add to `Cargo.toml`:

```toml
[dependencies]
erlang-syntax-rs = { git = "https://github.com/lmth/erlang_syntax_rs" }
```

### Parse a single form

```rust
use erlang_syntax_rs::parse_form;

let term = parse_form("-module(myapp).\n")?;
// term = Node("form", [Node("attribute", [Token("-"), Token("module"),
//                            Node("attr_val", [...])]), Token(".\n")])
```

### Parse a whole file

```rust
use erlang_syntax_rs::parse_forms;

let src = std::fs::read_to_string("src/myapp.erl")?;
for result in parse_forms(&src) {
    match result {
        Ok(cst) => { /* walk the CST */ }
        Err(msg) => eprintln!("parse error: {msg}"),
    }
}
```

### Navigate the CST

```rust
use erlang_syntax_rs::Term;

fn first_token(term: &Term) -> Option<&str> {
    match term {
        Term::Token(t) => Some(t),
        Term::Node { children, .. } => children.iter().find_map(first_token),
    }
}
```

`Term` also provides helpers: `as_token()`, `as_node()`, `child(n)`,
`unwrap_single_child()`, `first_token()`, `tokens()`.

## Example — `hrl2rust`

Convert Erlang `.hrl` header defines to Rust `pub const` declarations:

```
cargo run --example hrl2rust -- path/to/file.hrl
```

Given:

```erlang
-define(MAX_RETRIES, 3).
-define(PI, 3.14159).
-define(GREETING, "hello").
-define(APP_NAME, my_app).
-define(COMPLEX, foo:bar()).
```

Produces:

```rust
pub const MAX_RETRIES: i64 = 3;
pub const PI: f64 = 3.14159;
pub const GREETING: &str = "hello";
pub const APP_NAME: &str = "my_app";
// SKIP: -define(COMPLEX, foo : bar ( ))
```

## Architecture

| File | Role |
|------|------|
| `build.rs` | Locates `erl_parse.yrl`, translates grammar to Grmtools format, drives lrpar/lrlex codegen |
| `src/erlang.l` | Hand-written lrlex lexer (all Erlang terminals, keyword ordering) |
| `src/ast.rs` | `Term` CST type and navigation helpers |
| `src/lib.rs` | Public API: `parse_form`, `split_forms`, `parse_forms` |
| `src/anno.rs` | Feature-gated `Anno` type (source location; future use) |
| `examples/hrl2rust.rs` | Example tool: `.hrl` → Rust constants |

## Design notes

**No chicken-and-egg.** We avoid the classic bootstrapping problem (needing
Erlang to parse Erlang) by driving the parser generator entirely from Rust
build tooling.  The `.yrl` file is read as text; its grammar rules are
translated mechanically to Grmtools syntax; lrpar does the rest.

**Precedence.** `erl_parse.yrl` uses nonterminals in precedence declarations
(e.g. `comp_op`, `add_op`).  lrpar only supports terminals.  `build.rs`
introduces virtual precedence tokens (`PREC_COMP`, `PREC_ADD`, etc.) and
annotates the relevant rules with `%prec`.

**CST over AST.** All 400+ rules return `Result<Term, ()>`.  This avoids
hand-translating Erlang's semantic action functions into Rust.  A typed
semantic AST can be layered on top by walking the CST.

## License

Apache-2.0
