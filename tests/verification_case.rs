//! SysmlVerificationCase derive coverage: emits a `verification def
//! <id> { verify requirement <req>; }` block (the wrapper + the inner
//! trace in one) and the composed text survives the real SysML v2
//! grammar validator. Pairs with a `SysmlRequirement` subject to
//! show the "struct pair with typed relations" the audit requires.

use sysml_derive::{SysmlRequirement, SysmlVerificationCase};
use ufo_types::sysml::validate_sysml_v2;

#[derive(SysmlRequirement)]
#[sysml(id = "REQ_1", doc = "The actuator shall respond within 50 ms.")]
struct ReqOne {
    threshold: u32,
}

#[derive(SysmlVerificationCase)]
#[sysml(id = "VC_1", requirement = "REQ_1")]
struct VerifierOne;

#[derive(SysmlVerificationCase)]
#[sysml(requirement = "REQ_1")]
struct VerifierDefaultId;

fn assert_valid(label: &str, text: &str) {
    let result = validate_sysml_v2(text);
    assert!(
        result.disposition.is_satisfied(),
        "{label}: text failed validation: {:?} for: {text:?}",
        result
    );
}

#[test]
fn emits_verification_def_with_inner_verify_and_parses() {
    let text = VerifierOne::sysml_verification_case_def();
    assert_eq!(
        text,
        "verification def VC_1 {\n    verify requirement REQ_1;\n}\n"
    );
    assert_valid("verification def standalone", text);
}

#[test]
fn defaults_id_to_struct_name() {
    let text = VerifierDefaultId::sysml_verification_case_def();
    assert_eq!(
        text,
        "verification def VerifierDefaultId {\n    verify requirement REQ_1;\n}\n"
    );
    assert_valid("default-id verification def", text);
}

#[test]
fn exposes_target_requirement_id() {
    assert_eq!(VerifierOne::sysml_verification_target(), "REQ_1");
}

#[test]
fn pairs_with_requirement_subject() {
    // The audit's "struct pair with typed relations" — a
    // verification def that targets a real SysmlRequirement subject.
    let paired = format!(
        "{}{}",
        ReqOne::sysml_requirement_def(),
        VerifierOne::sysml_verification_case_def(),
    );
    assert_valid("requirement + verification def pair", &paired);
}
