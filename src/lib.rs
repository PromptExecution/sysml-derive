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
//! - `String` maps to `ScalarValues::String`. Opaque domain types (e.g. `NodeId`, `Confidence`,
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
//! generic field type is likewise a compile error unless an explicit
//! `#[sysml(type = "Domain::Type")]` mapping is supplied. Nested collections
//! are never silently flattened. Generated type references and
//! `sysml_block_def_checked` expose missing library/domain symbols before
//! emission; callers provide the resolved inventory from their selected model.

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Fields, GenericArgument, PathArguments, Type};

#[proc_macro_derive(SysmlBlock, attributes(sysml))]
pub fn derive_sysml_block(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match expand_block(&input) {
        Ok(expanded) => expanded.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

/// `#[derive(SysmlRequirement)]` — like `SysmlBlock`, but emits a
/// `requirement def <id> { doc /* <text> */ attribute ... }` instead of a
/// `part def <Name> { attribute ... }`. SysML v2 requirements are first-class
/// elements with their own keyword, an `id` (often `REQ-XXX`), and a `doc`
/// clause for the requirement statement; this derive produces all three
/// from the same Rust field walk that `SysmlBlock` uses, with two
/// additional `#[sysml(...)]` attributes:
///
/// - `#[sysml(id = "REQ-001")]` — the requirement's identifier text. If
///   absent, the Rust struct's `ident` is used (so `Requirement<Rust>`
///   becomes `requirement def Requirement<Rust>`, but the angle-bracket
///   rule from `SysmlBlock`'s docstring still applies — supply an `id`
///   when the name would contain generics).
/// - `#[sysml(doc = "The system shall ...")]` — the `doc /* ... */` body
///   of the requirement. If absent, the body is empty (still valid
///   `requirement def`; downstream tooling reads the doc separately).
///
/// The required-library constraint is the same as `SysmlBlock`: every
/// type reference must resolve against the caller's selected model
/// before publication; `sysml_requirement_def_checked` makes missing
/// symbols explicit. The kind on round-trip is `RequirementDefinition`
/// (per `ufo_types::ElementKind`), not `PartDefinition`.
#[proc_macro_derive(SysmlRequirement, attributes(sysml))]
pub fn derive_sysml_requirement(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match expand_requirement(&input) {
        Ok(expanded) => expanded.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

/// `#[derive(SysmlTrace)]` — emits a SysML v2 trace-relation statement for
/// a unit struct whose identifier is the role in the relation. The
/// relation kind is selected by a required `#[sysml(kind = "...")]`
/// attribute, one of:
///
/// - `kind = "satisfy"` — emits `satisfy requirement <REQ_ID> by <SelfName>;`
///   (the part named `<SelfName>` satisfies the requirement `<REQ_ID>`).
/// - `kind = "derive"` — emits `requirement <SelfName> :> <REQ_ID>;` (a
///   derived requirement; uses the specialization operator `:>` as the
///   `derive` syntax in the pinned `sysml-v2-parser`).
/// - `kind = "refine"` — emits `requirement <SelfName> :>> <REQ_ID>;`
///   (refinement via redefinition-specialization).
/// - `kind = "verify"` — emits `verify requirement <REQ_ID>;` to be
///   placed inside a `verification def <SelfName> { ... }` block
///   owned by the caller; the derive returns only the inner
///   statement, not the wrapper, because `verification def` is a
///   definition form that the `SysmlRequirement` derive does not yet
///   cover.
/// - `kind = "allocate"` — emits `allocate <SelfName> to <TARGET_ID>;`
///   where `<TARGET_ID>` comes from `#[sysml(target = "...")]`.
///
/// All five require `#[sysml(requirement = "REQ_ID")]` (for the first four)
/// or `#[sysml(target = "TGT_ID")]` (for `allocate`) on the same struct.
/// The kind/requirement/target identifiers use the same identifier
/// rules as `SysmlRequirement::id` — leading letter/underscore, then
/// letters/digits/underscores; hyphens and other punctuation are
/// rejected at compile time. This matches what the pinned
/// `sysml-v2-parser` accepts.
///
/// `SysmlTrace` does not produce a `part def` or `requirement def`
/// wrapper — the caller is expected to compose the trace statement
/// with a `SysmlBlock` or `SysmlRequirement` for the subject element,
/// or to include the verify statement in a `verification def` it
/// authors directly.
#[proc_macro_derive(SysmlTrace, attributes(sysml))]
pub fn derive_sysml_trace(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match expand_trace(&input) {
        Ok(expanded) => expanded.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

fn expand_block(input: &DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let name = &input.ident;
    let (impl_generics, type_generics, where_clause) = input.generics.split_for_impl();

    let named_fields: Vec<&syn::Field> = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(named) => named.named.iter().collect(),
            Fields::Unit => Vec::new(),
            _ => {
                return Err(syn::Error::new_spanned(
                    input,
                    "SysmlBlock only supports structs with named fields or unit structs",
                ));
            }
        },
        _ => {
            return Err(syn::Error::new_spanned(
                input,
                "SysmlBlock only supports structs",
            ));
        }
    };

    let mut attribute_lines = String::new();
    let mut references = std::collections::BTreeSet::new();
    for field in named_fields {
        // Safe: Fields::Named guarantees every field has an ident.
        let field_name = field.ident.as_ref().unwrap().to_string();
        let mapped_type = field_mapping(field)?;
        let (sysml_type, multiplicity) =
            sysml_type_and_multiplicity(&field.ty, mapped_type.as_deref())?;
        references.insert(sysml_type.clone());
        attribute_lines.push_str(&format!(
            "    attribute {field_name} : {sysml_type}{multiplicity};\n"
        ));
    }

    let block_def = format!("part def {name} {{\n{attribute_lines}}}\n");
    let references: Vec<_> = references.into_iter().collect();

    let expanded = quote! {
        impl #impl_generics #name #type_generics #where_clause {
            /// SysML-v2 block definition text for this type, generated at
            /// compile time by `#[derive(SysmlBlock)]` walking its fields.
            pub const fn sysml_block_def() -> &'static str {
                #block_def
            }

            /// Required type symbols, sorted and deduplicated. The caller resolves
            /// these against its selected model and pinned library inventory.
            pub const fn sysml_type_references() -> &'static [&'static str] {
                &[#(#references),*]
            }

            /// Emit only when every required type is present in the caller's
            /// resolved scope. Missing symbols remain explicit, never defaults.
            pub fn sysml_block_def_checked(
                resolved_types: &[&str],
            ) -> Result<&'static str, Vec<&'static str>> {
                let missing: Vec<_> = Self::sysml_type_references().iter().copied()
                    .filter(|reference| !resolved_types.contains(reference)).collect();
                if missing.is_empty() { Ok(Self::sysml_block_def()) } else { Err(missing) }
            }
        }
    };

    Ok(expanded)
}

fn expand_requirement(input: &DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let name = &input.ident;
    let (impl_generics, type_generics, where_clause) = input.generics.split_for_impl();

    let named_fields: Vec<&syn::Field> = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(named) => named.named.iter().collect(),
            Fields::Unit => Vec::new(),
            _ => {
                return Err(syn::Error::new_spanned(
                    input,
                    "SysmlRequirement only supports structs with named fields or unit structs",
                ));
            }
        },
        _ => {
            return Err(syn::Error::new_spanned(
                input,
                "SysmlRequirement only supports structs",
            ));
        }
    };

    let (id_text, doc_text) = read_requirement_meta(&input.attrs, name.to_string())?;

    let mut attribute_lines = String::new();
    let mut references = std::collections::BTreeSet::new();
    for field in named_fields {
        let field_name = field.ident.as_ref().unwrap().to_string();
        let mapped_type = field_mapping(field)?;
        let (sysml_type, multiplicity) =
            sysml_type_and_multiplicity(&field.ty, mapped_type.as_deref())?;
        references.insert(sysml_type.clone());
        attribute_lines.push_str(&format!(
            "    attribute {field_name} : {sysml_type}{multiplicity};\n"
        ));
    }

    // The doc body is rendered as a C-style block comment so it preserves
    // newlines and quotes without escaping; an empty doc still produces a
    // valid `requirement def` (the `doc` clause is optional in the grammar).
    let doc_clause = if doc_text.is_empty() {
        String::new()
    } else {
        format!("    doc /* {doc_text} */\n")
    };

    let requirement_def = format!(
        "requirement def {id_text} {{\n{doc_clause}{attribute_lines}}}\n"
    );
    let references: Vec<_> = references.into_iter().collect();

    let expanded = quote! {
        impl #impl_generics #name #type_generics #where_clause {
            /// SysML-v2 requirement-definition text for this type, generated at
            /// compile time by `#[derive(SysmlRequirement)]` walking its fields.
            pub const fn sysml_requirement_def() -> &'static str {
                #requirement_def
            }

            /// Required type symbols, sorted and deduplicated. The caller resolves
            /// these against its selected model and pinned library inventory.
            pub const fn sysml_type_references() -> &'static [&'static str] {
                &[#(#references),*]
            }

            /// Emit only when every required type is present in the caller's
            /// resolved scope. Missing symbols remain explicit, never defaults.
            pub fn sysml_requirement_def_checked(
                resolved_types: &[&str],
            ) -> Result<&'static str, Vec<&'static str>> {
                let missing: Vec<_> = Self::sysml_type_references().iter().copied()
                    .filter(|reference| !resolved_types.contains(reference)).collect();
                if missing.is_empty() { Ok(Self::sysml_requirement_def()) } else { Err(missing) }
            }
        }
    };

    Ok(expanded)
}

/// Extract the `id = "..."` and `doc = "..."` attribute values from a
/// struct-level `#[sysml(...)]` attribute on a `SysmlRequirement` derive.
/// Defaults: id = struct ident, doc = empty. The id is restricted to the
/// same qualified-name shape as `field_mapping`'s type override; the
/// doc is opaque text rendered inside `/* ... */` so the caller is
/// responsible for not putting `*/` in it.
fn read_requirement_meta(
    attrs: &[syn::Attribute],
    default_id: String,
) -> syn::Result<(String, String)> {
    let mut id: Option<String> = None;
    let mut doc: Option<String> = None;
    for attribute in attrs.iter().filter(|a| a.path().is_ident("sysml")) {
        attribute.parse_nested_meta(|meta| {
            if meta.path.is_ident("id") {
                if id.is_some() {
                    return Err(meta.error("duplicate SysmlRequirement id"));
                }
                let literal: syn::LitStr = meta.value()?.parse()?;
                let value = literal.value();
                if value.is_empty()
                    || value.contains(char::is_whitespace)
                    || value.contains("*/")
                    || value.contains("\"")
                    || value.contains('\n')
                    || value
                            .chars()
                            .next()
                            .map(|c| !c.is_ascii_alphabetic() && c != '_')
                            .unwrap_or(true)
                    || value
                            .chars()
                            .any(|c| !(c.is_ascii_alphanumeric() || c == '_'))
                {
                    return Err(syn::Error::new_spanned(
                        literal,
                        "SysML requirement id must be an identifier: leading letter/underscore, then letters/digits/underscores only (no hyphens, whitespace, quotes, or newlines)",
                    ));
                }
                id = Some(value);
                Ok(())
            } else if meta.path.is_ident("doc") {
                if doc.is_some() {
                    return Err(meta.error("duplicate SysmlRequirement doc"));
                }
                let literal: syn::LitStr = meta.value()?.parse()?;
                let value = literal.value();
                if value.contains("*/") {
                    return Err(syn::Error::new_spanned(
                        literal,
                        "doc text may not contain the block-comment terminator */",
                    ));
                }
                doc = Some(value);
                Ok(())
            } else if meta.path.is_ident("type") {
                // `#[sysml(type = "...")]` is a per-field attribute on
                // SysmlBlock; on SysmlRequirement's struct level it's a
                // typo we want to surface, not silently accept.
                Err(meta.error(
                    "use #[sysml(type = \"...\")] on each field, not on the struct",
                ))
            } else {
                Err(meta.error(
                    "expected #[sysml(id = \"...\")] and/or #[sysml(doc = \"...\")]",
                ))
            }
        })?;
    }
    Ok((id.unwrap_or(default_id), doc.unwrap_or_default()))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TraceKind {
    Satisfy,
    Derive,
    Refine,
    Verify,
    Allocate,
}

fn trace_kind_from_str(s: &str) -> Option<TraceKind> {
    match s {
        "satisfy" => Some(TraceKind::Satisfy),
        "derive" => Some(TraceKind::Derive),
        "refine" => Some(TraceKind::Refine),
        "verify" => Some(TraceKind::Verify),
        "allocate" => Some(TraceKind::Allocate),
        _ => None,
    }
}

fn validate_identifier(value: &str, field: &str) -> syn::Result<()> {
    if value.is_empty()
        || value.contains(char::is_whitespace)
        || value.contains("*/")
        || value.contains('"')
        || value.contains('\n')
        || value
            .chars()
            .next()
            .map(|c| !c.is_ascii_alphabetic() && c != '_')
            .unwrap_or(true)
        || value
            .chars()
            .any(|c| !(c.is_ascii_alphanumeric() || c == '_'))
    {
        return Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            format!(
                "{field} must be a SysML identifier: leading letter/underscore, then letters/digits/underscores only"
            ),
        ));
    }
    Ok(())
}

fn expand_trace(input: &DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    if !matches!(input.data, Data::Struct(_)) {
        return Err(syn::Error::new_spanned(
            input,
            "SysmlTrace only supports structs (unit or otherwise)",
        ));
    }

    let (impl_generics, type_generics, where_clause) = input.generics.split_for_impl();
    let (kind, requirement, target) = read_trace_meta(&input.attrs)?;

    let name = &input.ident;
    let statement = match kind {
        TraceKind::Satisfy => {
            // `satisfy requirement <REQ> by <Self>;`
            format!("satisfy requirement {requirement} by {name};\n")
        }
        TraceKind::Derive => {
            // `requirement <Self> :> <REQ>;` — the derived requirement
            // (named by Self) is a specialization of <REQ>.
            format!("requirement {name} :> {requirement};\n")
        }
        TraceKind::Refine => {
            // `requirement <Self> :>> <REQ>;` — refinement.
            format!("requirement {name} :>> {requirement};\n")
        }
        TraceKind::Verify => {
            // `verify requirement <REQ>;` — the inner statement for a
            // `verification def <Self> { ... }` block owned by the caller.
            format!("verify requirement {requirement};\n")
        }
        TraceKind::Allocate => {
            // `allocate <Self> to <Target>;`
            format!("allocate {name} to {target};\n")
        }
    };

    let expanded = quote! {
        impl #impl_generics #name #type_generics #where_clause {
            /// SysML-v2 trace-relation statement for this type, generated at
            /// compile time by `#[derive(SysmlTrace)]`. Composes with
            /// `SysmlBlock`/`SysmlRequirement` for the subject element.
            pub const fn sysml_trace_def() -> &'static str {
                #statement
            }
        }
    };

    Ok(expanded)
}

fn read_trace_meta(attrs: &[syn::Attribute]) -> syn::Result<(TraceKind, String, String)> {
    let mut kind: Option<TraceKind> = None;
    let mut requirement: Option<String> = None;
    let mut target: Option<String> = None;
    for attribute in attrs.iter().filter(|a| a.path().is_ident("sysml")) {
        attribute.parse_nested_meta(|meta| {
            if meta.path.is_ident("kind") {
                if kind.is_some() {
                    return Err(meta.error("duplicate SysmlTrace kind"));
                }
                let literal: syn::LitStr = meta.value()?.parse()?;
                let value = literal.value();
                let parsed = trace_kind_from_str(&value).ok_or_else(|| {
                    meta.error(
                        "kind must be one of: \"satisfy\", \"derive\", \"refine\", \"verify\", \"allocate\"",
                    )
                })?;
                kind = Some(parsed);
                Ok(())
            } else if meta.path.is_ident("requirement") {
                if requirement.is_some() {
                    return Err(meta.error("duplicate SysmlTrace requirement"));
                }
                let literal: syn::LitStr = meta.value()?.parse()?;
                let value = literal.value();
                validate_identifier(&value, "SysmlTrace requirement")?;
                requirement = Some(value);
                Ok(())
            } else if meta.path.is_ident("target") {
                if target.is_some() {
                    return Err(meta.error("duplicate SysmlTrace target"));
                }
                let literal: syn::LitStr = meta.value()?.parse()?;
                let value = literal.value();
                validate_identifier(&value, "SysmlTrace target")?;
                target = Some(value);
                Ok(())
            } else {
                Err(meta.error(
                    "expected #[sysml(kind = \"...\")] and/or #[sysml(requirement = \"...\")]/#[sysml(target = \"...\")]",
                ))
            }
        })?;
    }
    let kind = kind.ok_or_else(|| {
        syn::Error::new(
            proc_macro2::Span::call_site(),
            "SysmlTrace requires #[sysml(kind = \"satisfy\"|\"derive\"|\"refine\"|\"verify\"|\"allocate\")]",
        )
    })?;
    if matches!(kind, TraceKind::Allocate) {
        let target = target.ok_or_else(|| {
            syn::Error::new(
                proc_macro2::Span::call_site(),
                "SysmlTrace with kind = \"allocate\" requires #[sysml(target = \"...\")]",
            )
        })?;
        Ok((kind, String::new(), target))
    } else {
        let requirement = requirement.ok_or_else(|| {
            syn::Error::new(
                proc_macro2::Span::call_site(),
                "SysmlTrace with the given kind requires #[sysml(requirement = \"...\")]",
            )
        })?;
        Ok((kind, requirement, target.unwrap_or_default()))
    }
}

fn field_mapping(field: &syn::Field) -> syn::Result<Option<String>> {
    let mut mapping = None;
    for attribute in field.attrs.iter().filter(|a| a.path().is_ident("sysml")) {
        attribute.parse_nested_meta(|meta| {
            if !meta.path.is_ident("type") {
                return Err(meta.error("expected sysml(type = \"Package::Type\")"));
            }
            if mapping.is_some() {
                return Err(meta.error("duplicate SysML type mapping"));
            }
            let literal: syn::LitStr = meta.value()?.parse()?;
            let symbol = literal.value();
            if symbol.contains("r#")
                || symbol
                    .split("::")
                    .any(|part| syn::parse_str::<syn::Ident>(part).is_err())
            {
                return Err(syn::Error::new_spanned(
                    literal,
                    "expected a qualified SysML type name",
                ));
            }
            mapping = Some(symbol);
            Ok(())
        })?;
    }
    Ok(mapping)
}

/// Approximate a Rust field type as a SysML-v2 attribute type + multiplicity
/// suffix: `Vec<T>` -> `(T, "[*]")`, `Option<T>` -> `(T, "[0..1]")`,
/// `DateTime<_>` -> `(ScalarValues::String, "")` (no generic-parameter
/// syntax exists in SysML v2's grammar, so the parameter is dropped, not
/// rendered), everything else -> `(scalar-mapped-or-bare-name, "")`.
/// A supplied mapping names the value type after the outer Option/Vec wrapper;
/// nested collections need an explicit domain value type instead of flattening.
fn sysml_type_and_multiplicity(ty: &Type, mapping: Option<&str>) -> syn::Result<(String, String)> {
    if let Type::Path(type_path) = ty {
        if let Some(segment) = type_path.path.segments.last() {
            let path_name = type_path
                .path
                .segments
                .iter()
                .map(|s| s.ident.to_string())
                .collect::<Vec<_>>()
                .join("::");
            let vector = matches!(
                path_name.as_str(),
                "Vec" | "std::vec::Vec" | "alloc::vec::Vec"
            );
            let optional = matches!(
                path_name.as_str(),
                "Option" | "std::option::Option" | "core::option::Option"
            );
            if vector || optional {
                if let PathArguments::AngleBracketed(args) = &segment.arguments {
                    if args.args.len() == 1 {
                        if let Some(GenericArgument::Type(inner)) = args.args.first() {
                            let suffix = if vector { "[*]" } else { "[0..1]" };
                            return Ok((sysml_scalar_name(inner, mapping)?, suffix.to_string()));
                        }
                    }
                }
            }
        }
    }
    Ok((sysml_scalar_name(ty, mapping)?, String::new()))
}

/// Map a Rust primitive scalar to its SysML-v2 `ScalarValues` equivalent;
/// everything else (opaque domain types like `NodeId`,
/// `Confidence`, `rust_decimal::Decimal`) passes through as a bare
/// type-name reference, assumed to resolve to a sibling declaration
/// elsewhere in the model.
fn sysml_scalar_name(ty: &Type, mapping: Option<&str>) -> syn::Result<String> {
    if let Some(mapping) = mapping {
        return Ok(mapping.to_owned());
    }
    let Type::Path(path) = ty else {
        return Err(syn::Error::new_spanned(
            ty,
            "this Rust type requires an explicit sysml(type = \"Package::Type\") mapping",
        ));
    };
    if path.qself.is_some() {
        return Err(syn::Error::new_spanned(
            ty,
            "associated types require an explicit SysML type mapping",
        ));
    }
    if let Some(last) = path.path.segments.last() {
        if last.ident == "DateTime"
            && matches!(&last.arguments, PathArguments::AngleBracketed(args) if args.args.len() == 1)
            && path
                .path
                .segments
                .iter()
                .take(path.path.segments.len() - 1)
                .all(|segment| matches!(segment.arguments, PathArguments::None))
        {
            return Ok("ScalarValues::String".to_owned());
        }
    }
    if path
        .path
        .segments
        .iter()
        .any(|segment| !matches!(segment.arguments, PathArguments::None))
    {
        return Err(syn::Error::new_spanned(ty, "generic and nested collection types require an explicit SysML type mapping; they cannot be flattened without losing structure"));
    }
    let raw = type_to_string(ty);
    let scalar = raw
        .strip_prefix("std::primitive::")
        .or_else(|| raw.strip_prefix("core::primitive::"))
        .unwrap_or(&raw);
    Ok(match scalar {
        "String" | "std::string::String" | "alloc::string::String" => {
            "ScalarValues::String".to_string()
        }
        "bool" => "ScalarValues::Boolean".to_string(),
        "u8" | "u16" | "u32" | "u64" | "u128" | "usize" => "ScalarValues::Natural".to_string(),
        "i8" | "i16" | "i32" | "i64" | "i128" | "isize" => "ScalarValues::Integer".to_string(),
        "f32" | "f64" => "ScalarValues::Rational".to_string(),
        _ => raw,
    })
}

fn type_to_string(ty: &Type) -> String {
    quote!(#ty).to_string().replace(' ', "")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mapped_source_cases_emit_valid_value_shapes_or_explicit_errors() {
        let cases: Vec<serde_json::Value> =
            serde_json::from_str(include_str!("../tests/fixtures/type_mapping_cases.json"))
                .unwrap();
        for case in cases {
            let input: DeriveInput = syn::parse_str(case["source"].as_str().unwrap()).unwrap();
            let result = expand_block(&input);
            if let Some(error) = case["error"].as_str() {
                assert!(result.unwrap_err().to_string().contains(error), "{case}");
            } else {
                let expanded = syn::parse2::<syn::ItemImpl>(result.unwrap()).unwrap();
                let method = expanded
                    .items
                    .iter()
                    .find_map(|item| match item {
                        syn::ImplItem::Fn(method) if method.sig.ident == "sysml_block_def" => {
                            Some(method)
                        }
                        _ => None,
                    })
                    .unwrap();
                let syn::Stmt::Expr(syn::Expr::Lit(literal), _) = &method.block.stmts[0] else {
                    panic!("expected generated definition literal");
                };
                let syn::Lit::Str(text) = &literal.lit else {
                    panic!("expected string");
                };
                assert!(
                    text.value().contains(case["attribute"].as_str().unwrap()),
                    "{case}"
                );
                assert!(
                    ufo_types::sysml::validate_sysml_v2(&text.value())
                        .disposition
                        .is_satisfied(),
                    "{case}"
                );
            }
        }
    }
}
