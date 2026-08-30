//! First concrete instance of "b00t bicep digests into SysML-v2" (the
//! condition tracked by PromptExecution/infrastructure#139's /goal check):
//! a real Bicep module's parameter/output interface, expressed as a Rust
//! struct and run through `#[derive(SysmlBlock)]`, validated against a real
//! SysML v2 grammar the same way `real_grammar_validation.rs` does.
//!
//! Source: `PromptExecution/infrastructure`'s
//! `msft-corp/modules/agent-identity/main.bicep` (PR #141) - a reusable
//! module that creates one Entra `Microsoft.Graph/applications` +
//! `servicePrincipals` pair plus a Key Vault role assignment. Field names
//! and types below are hand-transcribed from that module's `param`/`output`
//! declarations, not generated - an actual Bicep-AST-to-Rust-struct
//! generator (so this doesn't need hand-transcription for every module) is
//! the next step in this line of work, not this spike.
//!
//! This intentionally only proves the pipeline shape (Bicep interface ->
//! Rust struct -> SysML-v2 text -> validated by a real grammar), not a
//! generator. `sysml-derive`'s flat attribute list also doesn't distinguish
//! Bicep's params (inputs) from outputs - both land as plain `attribute`
//! lines here, which is a known modeling gap worth a follow-up (SysML v2
//! has `in`/`out` feature directionality that a real generator should use).

use sysml_derive::SysmlBlock;
use ufo_types::sysml::validate_sysml_v2;

// Mirrors msft-corp/modules/agent-identity/main.bicep's params + outputs.
// All-`String` because Bicep's own type system for this module is
// string-only (agentId, displayNamePrefix, keyVaultName,
// vaultRoleDefinitionId as params; appId, objectId,
// servicePrincipalObjectId as outputs) - doesn't exercise the
// Vec/Option/DateTime branches, see real_grammar_validation.rs for those.
#[derive(SysmlBlock)]
struct AgentIdentity {
    agent_id: String,
    display_name_prefix: String,
    key_vault_name: String,
    vault_role_definition_id: String,
    app_id: String,
    object_id: String,
    service_principal_object_id: String,
}

#[test]
fn agent_identity_bicep_module_digests_to_valid_sysml_v2() {
    let block = AgentIdentity::sysml_block_def();
    let result = validate_sysml_v2(block);
    assert!(
        result.disposition.is_satisfied(),
        "AgentIdentity: generated text failed real SysML v2 grammar validation: {:?}\n---\n{block}",
        result.disposition
    );
}
