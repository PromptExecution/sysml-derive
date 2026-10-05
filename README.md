# sysml-derive

`#[derive(SysmlBlock)]` walks a named Rust struct and emits a SysML v2 `part def`.
The generated implementation preserves Rust generics and where clauses.

```rust
use sysml_derive::SysmlBlock;

#[derive(SysmlBlock)]
struct Controller {
    enabled: bool,
    label: String,
}

let scope = ["ScalarValues::Boolean", "ScalarValues::String"];
let text = Controller::sysml_block_def_checked(&scope).unwrap();
```

`sysml_type_references()` lists sorted unique dependencies. The checked emitter
returns every missing name, including library and domain references. Its caller
supplies the inventory resolved from the selected model and pinned libraries;
a list of invented names is not evidence of semantic validity. Parser acceptance,
type/metaclass compatibility and native server retention require their own checks.
The legacy unchecked `sysml_block_def()` remains available for callers performing
that validation elsewhere.

Rust primitives map to ScalarValues names. String maps to ScalarValues::String;
Vec values use `[*]`, Option values use `[0..1]`. The existing DateTime-to-String
approximation also works inside either wrapper. It does not preserve timestamp
semantics unless a caller supplies and resolves an explicit modeled type.

Use `#[sysml(type = "Domain::Timestamp")]` on a field to select a domain symbol.
For an outer Vec or Option, the mapping describes the contained value type.
For example a `Vec<Vec<u8>>` field mapped to `Domain::ByteSequence` becomes
`Domain::ByteSequence[*]`: each inner collection is a declared domain value.
This is an authored mapping requiring a corresponding definition, not an
automatically inferred lossless conversion. Without a mapping, unsupported
generics, nested collections, associated types and references fail at compile time.
The original Rust source remains the evidence for those shapes.

Run `cargo test`, `cargo clippy --all-targets -- -D warnings`, and
`cargo fmt --check`. Multi-record mapping/resolution cases are committed JSON
fixtures; integration tests compile generic derives and parse their actual text.
