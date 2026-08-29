//! `#[derive(SysmlBlock)]` — walks a struct's fields via its AST (`syn`) and
//! generates a `sysml_block_def()` associated function returning the
//! equivalent SysML-v2 `part def` textual definition, computed at compile
//! time from the field list. (The derive and function are named after the
//! informal "block definition" concept, not the literal SysML v1 `Block`/
//! `block def` keyword — SysML v2 renamed that construct to `part def`; see
//! below.)
//!
//! Spike for the systems-modeling epic — see
//! `docs/systems-modeling-registry-rescope.md` §2a and §6 task 1. This
//! proves the "walk the Rust AST, generate SysML content via macro"
//! direction the user proposed as an alternative/complement to LinkML,
//! before either is wired into real `ArtifactKind`/`NodeType` node types.
//! Not conformance-checked against a real SysML-v2 grammar/parser (Part 1's
//! Tier 0 candidates) end-to-end, but the specific field-type mappings below
//! were checked against SysML v2's `ScalarValues` standard-library package
//! and its textual grammar (ledgrrr#195):
//!
//! - `Vec<T>` -> `T[*]`, `Option<T>` -> `T[0..1]` (multiplicity suffixes —
//!   unaffected by the scalar mapping below, applied to the inner `T`).
//! - Rust primitive scalars are mapped to their `ScalarValues` equivalent
//!   rather than emitted as bare Rust keywords (`bool`/`usize`/etc. aren't
//!   SysML v2 type names and would be dangling references): `bool` ->
//!   `ScalarValues::Boolean`; `u8..u128`/`usize` -> `ScalarValues::Natural`;
//!   `i8..i128`/`isize` -> `ScalarValues::Integer`; `f32`/`f64` ->
//!   `ScalarValues::Rational`.
//! - `chrono::DateTime<Tz>` (any `Tz`) -> `ScalarValues::String`. SysML v2
//!   has no native date/time scalar, and critically, SysML v2's textual
//!   grammar has **no angle-bracket generic-parameter syntax** — before this
//!   fix, a `DateTime<Utc>` field emitted the literal, invalid text
//!   `attribute x : DateTime<Utc>;`, which does not parse under any
//!   conformant SysML v2 grammar. The `Vec`/`Option` cases don't have this
//!   problem because their generic parameter is consumed into a
//!   multiplicity suffix, never rendered as `<...>` text; any other
//!   generic type (single type argument, not `Vec`/`Option`/`DateTime`) is
//!   therefore rejected as a compile error rather than silently emitting
//!   the same class of invalid syntax.
//! - `String` and opaque domain types (e.g. `NodeId`, `Confidence`,
//!   `rust_decimal::Decimal`) pass through as bare type-name references,
//!   under the standard SysML modeling assumption that they resolve to a
//!   sibling `part def`/`attribute def`/`datatype` declared elsewhere in
//!   the same model or an imported package — the same assumption every
//!   `part def` referencing another `part def` by name already relies on.
//!   This is a documented modeling assumption, not a bug: unlike the
//!   primitives/`DateTime` case above, there is no single universally-right
//!   SysML mapping for a project-specific newtype to invent here.
//! - The outer wrapper emits `part def {Name} { ... }`, not `block def` —
//!   SysML v1 called this construct `Block`; SysML v2 renamed the
//!   equivalent concept to `part def`, and `block` is not a SysML v2
//!   keyword at all. Confirmed against the real `sysml-v2-parser` crate via
//!   `ufo_types::sysml::validate_sysml_v2` (see
//!   `crates/sysml-derive/tests/real_grammar_validation.rs`) — the same bug
//!   `holon-viz`'s `SysmlV2Emitter` had (ledgrrr#197).
//!
//! Only supports structs with named fields; anything else is a compile
//! error via `syn::Error::to_compile_error`, not a panic. An unsupported
//! generic field type (see above) is likewise a compile error, not a
//! silent invalid-syntax emission.

use proc_macro::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields, GenericArgument, PathArguments, Type, parse_macro_input};

#[proc_macro_derive(SysmlBlock)]
pub fn derive_sysml_block(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    let named_fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(named) => &named.named,
            _ => {
                return syn::Error::new_spanned(
                    &input,
                    "SysmlBlock only supports structs with named fields",
                )
                .to_compile_error()
                .into();
            }
        },
        _ => {
            return syn::Error::new_spanned(&input, "SysmlBlock only supports structs")
                .to_compile_error()
                .into();
        }
    };

    let mut attribute_lines = String::new();
    for field in named_fields {
        // Safe: Fields::Named guarantees every field has an ident.
        let field_name = field.ident.as_ref().unwrap().to_string();
        let (sysml_type, multiplicity) = match sysml_type_and_multiplicity(&field.ty) {
            Ok(pair) => pair,
            Err(err) => return err.to_compile_error().into(),
        };
        attribute_lines.push_str(&format!(
            "    attribute {field_name} : {sysml_type}{multiplicity};\n"
        ));
    }

    let block_def = format!("part def {name} {{\n{attribute_lines}}}\n");

    let expanded = quote! {
        impl #name {
            /// SysML-v2 block definition text for this type, generated at
            /// compile time by `#[derive(SysmlBlock)]` walking its fields.
            pub const fn sysml_block_def() -> &'static str {
                #block_def
            }
        }
    };

    expanded.into()
}

/// Approximate a Rust field type as a SysML-v2 attribute type + multiplicity
/// suffix: `Vec<T>` -> `(T, "[*]")`, `Option<T>` -> `(T, "[0..1]")`,
/// `DateTime<_>` -> `(ScalarValues::String, "")` (no generic-parameter
/// syntax exists in SysML v2's grammar, so the parameter is dropped, not
/// rendered), everything else -> `(scalar-mapped-or-bare-name, "")`. Any
/// other single-type-argument generic is rejected at compile time rather
/// than silently emitting the same invalid `Outer<Inner>` text `DateTime`
/// used to produce.
fn sysml_type_and_multiplicity(ty: &Type) -> syn::Result<(String, String)> {
    if let Type::Path(type_path) = ty {
        if let Some(segment) = type_path.path.segments.last() {
            let ident = segment.ident.to_string();
            if ident == "Vec" || ident == "Option" {
                if let PathArguments::AngleBracketed(args) = &segment.arguments {
                    if let Some(GenericArgument::Type(inner)) = args.args.first() {
                        let suffix = if ident == "Vec" { "[*]" } else { "[0..1]" };
                        return Ok((sysml_scalar_name(inner), suffix.to_string()));
                    }
                }
            }
            if ident == "DateTime" {
                return Ok(("ScalarValues::String".to_string(), String::new()));
            }
            if matches!(segment.arguments, PathArguments::AngleBracketed(_)) {
                return Err(syn::Error::new_spanned(
                    ty,
                    format!(
                        "SysmlBlock has no SysML-v2 mapping for generic type `{ident}<..>` \
                         (SysML v2's grammar has no angle-bracket generic syntax); add an \
                         explicit case to sysml_type_and_multiplicity in sysml-derive/src/lib.rs"
                    ),
                ));
            }
        }
    }
    Ok((sysml_scalar_name(ty), String::new()))
}

/// Map a Rust primitive scalar to its SysML-v2 `ScalarValues` equivalent;
/// everything else (`String`, and opaque domain types like `NodeId`,
/// `Confidence`, `rust_decimal::Decimal`) passes through as a bare
/// type-name reference, assumed to resolve to a sibling declaration
/// elsewhere in the model.
fn sysml_scalar_name(ty: &Type) -> String {
    let raw = type_to_string(ty);
    match raw.as_str() {
        "bool" => "ScalarValues::Boolean".to_string(),
        "u8" | "u16" | "u32" | "u64" | "u128" | "usize" => "ScalarValues::Natural".to_string(),
        "i8" | "i16" | "i32" | "i64" | "i128" | "isize" => "ScalarValues::Integer".to_string(),
        "f32" | "f64" => "ScalarValues::Rational".to_string(),
        _ => raw,
    }
}

fn type_to_string(ty: &Type) -> String {
    quote!(#ty).to_string().replace(' ', "")
}
