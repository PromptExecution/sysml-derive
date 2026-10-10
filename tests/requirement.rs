//! SysmlRequirement derive coverage: every case both (a) generates the
//! expected substring and (b) survives `validate_sysml_v2` so the
//! emitted text is not just plausible-looking but parseable under the
//! pinned `sysml-v2-parser`. End-to-end round-trip through the OMG
//! server (kind retention, fetch→emit→fetch equivalence) is a separate
//! P2 acceptance gate that lives in `kr0ki-behavioral-gate`'s
//! `assurance_a05_live.rs`; the derive-side tests here are the
//! pre-condition that emission produces valid syntax.

use sysml_derive::SysmlRequirement;
use ufo_types::sysml::validate_sysml_v2;

#[derive(SysmlRequirement)]
#[sysml(id = "REQ_1", doc = "The system shall produce a result.")]
struct ReqOne {
    name: String,
    count: u32,
}

#[derive(SysmlRequirement)]
struct ReqNoMeta {
    kind: String,
}

#[derive(SysmlRequirement)]
#[sysml(id = "REQ_Vec")]
struct ReqWithVector {
    items: Vec<String>,
}

#[derive(SysmlRequirement)]
#[sysml(id = "REQ_Opt")]
struct ReqWithOption {
    value: Option<String>,
}

#[derive(SysmlRequirement)]
#[sysml(doc = "x")]
struct ReqOnlyDoc {
    n: u32,
}

#[test]
fn emits_requirement_def_with_doc_and_attributes() {
    let text = ReqOne::sysml_requirement_def();
    assert!(
        text.starts_with("requirement def REQ_1 {\n"),
        "expected `requirement def REQ_1 {{` header, got: {text:?}"
    );
    assert!(
        text.contains("doc /* The system shall produce a result. */"),
        "doc clause missing in: {text:?}"
    );
    assert!(
        text.contains("attribute name : ScalarValues::String;"),
        "name attribute missing in: {text:?}"
    );
    assert!(
        text.contains("attribute count : ScalarValues::Natural;"),
        "count attribute missing in: {text:?}"
    );
    let validation = validate_sysml_v2(text);
    assert!(
        validation.disposition.is_satisfied(),
        "emitted text failed grammar validation: {:?} for text: {text:?}",
        validation
    );
}

#[test]
fn defaults_id_to_struct_name_and_doc_to_empty() {
    let text = ReqNoMeta::sysml_requirement_def();
    assert!(
        text.starts_with("requirement def ReqNoMeta {\n"),
        "expected struct name as default id, got: {text:?}"
    );
    assert!(
        !text.contains("doc"),
        "expected no `doc` clause when not provided, got: {text:?}"
    );
    let validation = validate_sysml_v2(text);
    assert!(
        validation.disposition.is_satisfied(),
        "default-id/empty-doc text failed validation: {:?} for text: {text:?}",
        validation
    );
}

#[test]
fn vector_field_gets_multiplicity_suffix() {
    let text = ReqWithVector::sysml_requirement_def();
    assert!(
        text.contains("attribute items : ScalarValues::String[*];"),
        "vector multiplicity [*] missing in: {text:?}"
    );
    assert!(
        validate_sysml_v2(text).disposition.is_satisfied(),
        "vector-field text failed validation: {text:?}"
    );
}

#[test]
fn option_field_gets_zero_one_multiplicity() {
    let text = ReqWithOption::sysml_requirement_def();
    assert!(
        text.contains("attribute value : ScalarValues::String[0..1];"),
        "option multiplicity [0..1] missing in: {text:?}"
    );
    assert!(
        validate_sysml_v2(text).disposition.is_satisfied(),
        "option-field text failed validation: {text:?}"
    );
}

#[test]
fn type_references_include_all_scalar_symbols() {
    let refs = ReqOne::sysml_type_references();
    assert!(refs.contains(&"ScalarValues::String"), "refs: {refs:?}");
    assert!(refs.contains(&"ScalarValues::Natural"), "refs: {refs:?}");
}

#[test]
fn checked_variant_surfaces_missing_symbols() {
    let resolved = ["ScalarValues::String"]; // Natural missing
    let missing = ReqOne::sysml_requirement_def_checked(&resolved)
        .expect_err("expected Err when Natural is missing from scope");
    assert!(
        missing.contains(&"ScalarValues::Natural"),
        "missing list: {missing:?}"
    );
    let full = ReqOne::sysml_requirement_def_checked(&[
        "ScalarValues::String",
        "ScalarValues::Natural",
    ])
    .expect("expected Ok when all refs resolved");
    assert!(
        validate_sysml_v2(full).disposition.is_satisfied(),
        "checked variant text failed validation: {full:?}"
    );
}

#[test]
fn doc_only_no_id_still_emits() {
    let text = ReqOnlyDoc::sysml_requirement_def();
    assert!(text.starts_with("requirement def ReqOnlyDoc {"), "{text:?}");
    assert!(text.contains("doc /* x */"), "{text:?}");
    assert!(
        validate_sysml_v2(text).disposition.is_satisfied(),
        "doc-only text failed validation: {text:?}"
    );
}
