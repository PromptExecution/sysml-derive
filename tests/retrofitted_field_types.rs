//! Regression coverage for field types introduced by the retrofit of
//! `#[derive(SysmlBlock)]` onto pre-existing `arc-kit-au::node` structs
//! (ledgrrr#193) and the earlier `Requirement`/`Decision`/`Cost` addition
//! (ledgrrr#184) — `rust_decimal::Decimal`, a custom `Confidence` type,
//! `bool`, `usize`, and `chrono::DateTime<Utc>`. None of these were
//! exercised by `tests/basic.rs` (only `String`/`Option<String>`/
//! `Vec<NodeId>`), and `DateTime<Utc>` previously emitted invalid SysML v2
//! syntax (`attribute x : DateTime<Utc>;` — SysML v2's grammar has no
//! angle-bracket generic syntax). See ledgrrr#195.

use sysml_derive::SysmlBlock;

// Stand-ins — only the type names matter for the macro's output.
#[allow(dead_code)]
struct NodeId(String);
#[allow(dead_code)]
struct Confidence(f64);
#[allow(dead_code)]
struct Decimal(String);
#[allow(dead_code)]
struct Utc;
#[allow(dead_code)]
struct DateTime<T>(T);

// Mirrors arc-kit-au::node::ExtractedRow.
#[allow(dead_code)]
#[derive(SysmlBlock)]
struct ExtractedRow {
    amount: Decimal,
    source_document: NodeId,
    extraction_confidence: Confidence,
}

// Mirrors arc-kit-au::node::ModelProposal / OperatorApproval / ValidationIssue.
#[allow(dead_code)]
#[derive(SysmlBlock)]
struct ModelProposal {
    validated: bool,
    proposed_at: DateTime<Utc>,
}

// Mirrors arc-kit-au::node::WorkbookRow.
#[allow(dead_code)]
#[derive(SysmlBlock)]
struct WorkbookRow {
    row_index: usize,
}

#[test]
fn opaque_domain_types_pass_through_as_bare_type_names() {
    let block = ExtractedRow::sysml_block_def();
    // Decimal/Confidence/NodeId have no ScalarValues equivalent — they're
    // assumed to resolve to sibling declarations elsewhere in the model.
    assert!(block.contains("    attribute amount : Decimal;\n"));
    assert!(block.contains("    attribute source_document : NodeId;\n"));
    assert!(block.contains("    attribute extraction_confidence : Confidence;\n"));
}

#[test]
fn bool_maps_to_scalar_values_boolean() {
    let block = ModelProposal::sysml_block_def();
    assert!(block.contains("    attribute validated : ScalarValues::Boolean;\n"));
}

#[test]
fn datetime_maps_to_scalar_values_string_never_emits_generic_brackets() {
    let block = ModelProposal::sysml_block_def();
    assert!(block.contains("    attribute proposed_at : ScalarValues::String;\n"));
    assert!(
        !block.contains('<'),
        "SysML v2 has no angle-bracket generic syntax; generated block must never contain \
         `<` or `>`:\n{block}"
    );
}

#[test]
fn usize_maps_to_scalar_values_natural() {
    let block = WorkbookRow::sysml_block_def();
    assert!(block.contains("    attribute row_index : ScalarValues::Natural;\n"));
}
