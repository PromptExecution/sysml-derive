# Changelog

## 0.2.0 — Unreleased

- Generated implementations retain Rust generic parameters and where clauses.
- `sysml_type_references()` returns the sorted required symbols;
  `sysml_block_def_checked(scope)` returns unresolved symbols before emission.
- Explicit field mappings use `#[sysml(type = "Domain::Type")]`. Unsupported
  generic/associated/reference types and nested collections require a mapping;
  nested collection structure is never silently flattened.
- Nested DateTime values use the existing String approximation without emitting
  Rust generic syntax. A domain mapping can preserve a modeled timestamp type.
- **Output change:** String and qualified standard String now reference
  `ScalarValues::String`, requiring that library symbol in the selected scope.
- Parser tests pin canonical ufo-types commit
  `e900190e4be8b3380c9f4a44be0c85dda4f14b47`.

The checked scope API validates exact referenced names. It does not fetch
libraries, prove metaclass compatibility, or establish a model-server round trip.
