//! Validates `#[derive(SysmlBlock)]`'s generated `sysml_block_def()` text
//! against a real SysML v2 grammar implementation (`ufo_types::sysml`,
//! wired to the `sysml-v2-parser` crate), instead of only checking "no
//! angle brackets" or other hand-rolled heuristics.
//!
//! This is the concrete validation ledgrrr#195 asked for and the
//! prioritized follow-up after the scalar-type-mapping fix
//! (`fix_datetime_and_primitive_field_types_now_pass_real_parser` below is
//! the test that would have failed before that fix — `DateTime<Utc>` used
//! to emit literally invalid syntax).

use sysml_derive::SysmlBlock;
use ufo_types::sysml::validate_sysml_v2;

struct NodeId(String);
struct Confidence(f64);
struct Decimal(String);
struct Utc;
struct DateTime<T>(T);

// Mirrors arc-kit-au::node::Transaction (ledgrrr#184's original struct).
#[derive(SysmlBlock)]
struct Transaction {
    tx_id: String,
    source_rows: Vec<NodeId>,
}

// Mirrors arc-kit-au::node::Requirement (ledgrrr#184) -- has a `DateTime<Utc>` field.
#[derive(SysmlBlock)]
struct Requirement {
    requirement_id: String,
    rationale: Option<String>,
    related_decisions: Vec<NodeId>,
    imported_at: DateTime<Utc>,
}

// Mirrors arc-kit-au::node::ExtractedRow (ledgrrr#193 retrofit) -- Decimal/Confidence.
#[derive(SysmlBlock)]
struct ExtractedRow {
    amount: Decimal,
    source_document: NodeId,
    extraction_confidence: Confidence,
}

// Mirrors arc-kit-au::node::ModelProposal (ledgrrr#193 retrofit) -- bool + DateTime.
#[derive(SysmlBlock)]
struct ModelProposal {
    validated: bool,
    proposed_at: DateTime<Utc>,
}

// Mirrors arc-kit-au::node::WorkbookRow (ledgrrr#193 retrofit) -- usize.
#[derive(SysmlBlock)]
struct WorkbookRow {
    row_index: usize,
}

fn assert_valid_sysml_v2(label: &str, text: &str) {
    let result = validate_sysml_v2(text);
    assert!(
        result.disposition.is_satisfied(),
        "{label}: generated text failed real SysML v2 grammar validation: {:?}\n---\n{text}",
        result.disposition
    );
}

#[test]
fn transaction_block_def_is_valid_sysml_v2() {
    assert_valid_sysml_v2("Transaction", Transaction::sysml_block_def());
}

#[test]
fn datetime_and_primitive_field_types_now_pass_real_parser() {
    // Before the scalar-type-mapping fix, `imported_at : DateTime<Utc>`
    // emitted literally invalid syntax (angle-bracket generics don't exist
    // in SysML v2's grammar) -- this would have failed here.
    assert_valid_sysml_v2("Requirement", Requirement::sysml_block_def());
}

#[test]
fn opaque_domain_types_produce_valid_sysml_v2() {
    // Decimal/Confidence/NodeId pass through as bare type-name references
    // (documented modeling assumption) -- confirmed that's still
    // syntactically valid (referencing an undeclared name is not a syntax
    // error in SysML v2, only a semantic one this validator doesn't check).
    assert_valid_sysml_v2("ExtractedRow", ExtractedRow::sysml_block_def());
}

#[test]
fn bool_and_datetime_field_combination_is_valid_sysml_v2() {
    assert_valid_sysml_v2("ModelProposal", ModelProposal::sysml_block_def());
}

#[test]
fn usize_field_is_valid_sysml_v2() {
    assert_valid_sysml_v2("WorkbookRow", WorkbookRow::sysml_block_def());
}
