//! Spike proof: derive a SysML-v2 block definition from a struct that
//! mirrors `arc-kit-au::Transaction`'s field shape (name + type, not a real
//! dependency on `arc-kit-au` — wiring the derive onto the real production
//! struct is task 3 in `docs/systems-modeling-registry-rescope.md` §6, once
//! this spike and the LinkML spike have been compared).

use sysml_derive::SysmlBlock;

// Mirrors arc-kit-au::node::Transaction's exact field shape.
#[allow(dead_code)]
#[derive(SysmlBlock)]
struct Transaction {
    tx_id: String,
    account_id: String,
    date: String,
    amount: String,
    description: String,
    source_rows: Vec<NodeId>,
}

// Stand-in for arc-kit-au::node::NodeId — only the type name matters for the
// macro's output, not the real definition.
#[allow(dead_code)]
struct NodeId(String);

// Mirrors arc-kit-au::node::Classification's Option field, to exercise the
// `Option<T>` -> `T[0..1]` branch alongside Vec's `T[*]`.
#[allow(dead_code)]
#[derive(SysmlBlock)]
struct Classification {
    tx_id: String,
    category: String,
    sub_category: Option<String>,
}

#[test]
fn emits_block_def_with_scalar_and_vec_attributes() {
    let block = Transaction::sysml_block_def();
    assert!(block.starts_with("part def Transaction {\n"));
    assert!(block.contains("    attribute tx_id : String;\n"));
    assert!(block.contains("    attribute source_rows : NodeId[*];\n"));
    assert!(block.ends_with("}\n"));
}

#[test]
fn emits_optional_attribute_with_zero_to_one_multiplicity() {
    let block = Classification::sysml_block_def();
    assert!(block.contains("    attribute sub_category : String[0..1];\n"));
    assert!(block.contains("    attribute category : String;\n"));
}
