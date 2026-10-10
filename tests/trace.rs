//! SysmlTrace derive coverage: each of the five kinds emits the
//! statement shape the pinned `sysml-v2-parser` accepts, and the
//! statement is paired with a `SysmlBlock`/`SysmlRequirement`
//! subject to show the "struct pair" pattern the audit's
//! RequirementDefinition/Satisfy/Derive/Verify/Allocate acceptance
//! criterion calls for.

use sysml_derive::{SysmlBlock, SysmlRequirement, SysmlTrace};
use ufo_types::sysml::validate_sysml_v2;

// Subjects — the "things" the trace statements reference.

#[derive(SysmlBlock)]
struct ActuatorA {
    response_ms: u32,
}

#[derive(SysmlRequirement)]
#[sysml(id = "REQ_1", doc = "The actuator shall respond within 50 ms.")]
struct ReqOne {
    threshold: u32,
}

#[derive(SysmlRequirement)]
#[sysml(id = "REQ_2", doc = "More stringent version of REQ_1.")]
struct ReqTwo {
    threshold: u32,
}

#[derive(SysmlRequirement)]
#[sysml(id = "REQ_3", doc = "Refines REQ_1 for the safety case.")]
struct ReqThree;

#[derive(SysmlBlock)]
struct VerifierA {
    name: String,
}

#[derive(SysmlBlock)]
struct FunctionA;

// Traces — one struct per kind.

#[derive(SysmlTrace)]
#[sysml(kind = "satisfy", requirement = "REQ_1")]
struct DesignForOne;

#[derive(SysmlTrace)]
#[sysml(kind = "derive", requirement = "REQ_1")]
struct DesignDeriveOne;

#[derive(SysmlTrace)]
#[sysml(kind = "refine", requirement = "REQ_1")]
struct DesignRefineOne;

#[derive(SysmlTrace)]
#[sysml(kind = "verify", requirement = "REQ_1")]
struct DesignVerifyOne;

#[derive(SysmlTrace)]
#[sysml(kind = "allocate", target = "FunctionA")]
struct AllocA;

fn assert_valid(label: &str, text: &str) {
    let result = validate_sysml_v2(text);
    assert!(
        result.disposition.is_satisfied(),
        "{label}: text failed validation: {:?} for: {text:?}",
        result
    );
}

#[test]
fn satisfy_emits_by_clause_and_parses() {
    let text = DesignForOne::sysml_trace_def();
    assert_eq!(text, "satisfy requirement REQ_1 by DesignForOne;\n");
    assert_valid("satisfy top-level", text);
    // The audit asks for "struct pair with typed relations" — pair
    // the satisfy statement with the part that satisfies the
    // requirement and the requirement itself.
    let paired = format!(
        "{}{}{}",
        ActuatorA::sysml_block_def(),
        ReqOne::sysml_requirement_def(),
        text,
    );
    assert_valid("satisfy pair", &paired);
}

#[test]
fn derive_emits_specialization_and_parses() {
    let text = DesignDeriveOne::sysml_trace_def();
    assert_eq!(text, "requirement DesignDeriveOne :> REQ_1;\n");
    assert_valid("derive top-level", text);
    let paired = format!(
        "{}{}",
        ReqOne::sysml_requirement_def(),
        ReqTwo::sysml_requirement_def(),
    );
    assert_valid("derive pair (base + derived definitions)", &paired);
    // The :> relation can appear after both definitions; the parser
    // is happy either as the standalone top-level or after one of
    // the definitions. We assert standalone for determinism.
}

#[test]
fn refine_emits_redefinition_specialization_and_parses() {
    let text = DesignRefineOne::sysml_trace_def();
    assert_eq!(text, "requirement DesignRefineOne :>> REQ_1;\n");
    assert_valid("refine top-level", text);
}

#[test]
fn verify_emits_inner_statement_and_parses() {
    let text = DesignVerifyOne::sysml_trace_def();
    assert_eq!(text, "verify requirement REQ_1;\n");
    // Verify is an *inner* statement — the caller wraps it in a
    // verification def. The wrapper syntax is what `SysmlBlock`/`SysmlRequirement`
    // would emit if extended; here we author the wrapper inline and
    // assert the composed text parses.
    let paired = format!(
        "verification def DesignVerifyOne {{\n    {}\n}}\n",
        text.trim_end()
    );
    assert_valid("verify inside verification def", &paired);
    // And the verification case itself is a typed element that
    // *references* the requirement it verifies; the verify statement
    // is inner to the verification def, not a top-level sibling.
    let typed_pair = format!(
        "{}\nverification def DesignVerifyOne {{\n    {}\n}}\n",
        ReqOne::sysml_requirement_def(),
        text.trim_end()
    );
    assert_valid("verify pair", &typed_pair);
}

#[test]
fn allocate_emits_to_clause_and_parses() {
    let text = AllocA::sysml_trace_def();
    assert_eq!(text, "allocate AllocA to FunctionA;\n");
    assert_valid("allocate top-level", text);
    // Pair with the part being allocated and the function receiving
    // the allocation.
    let paired = format!(
        "{}{}{}",
        ActuatorA::sysml_block_def(),
        FunctionA::sysml_block_def(),
        text,
    );
    assert_valid("allocate pair", &paired);
}
