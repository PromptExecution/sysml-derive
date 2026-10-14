//! Compile the generic derive and validate actual generated text and scope closure.

use sysml_derive::SysmlBlock;
use ufo_types::sysml::validate_sysml_v2;

#[allow(dead_code)]
struct DateTime<T>(T);
struct Utc;

#[allow(dead_code)]
#[derive(SysmlBlock)]
struct GenericModel<T>
where
    T: Clone,
{
    #[sysml(type = "Domain::Node")]
    node: T,
    observed: Option<DateTime<Utc>>,
    #[sysml(type = "Domain::Sequence")]
    nested: Vec<Vec<u8>>,
}

#[test]
fn checked_generic_authoring_requires_selected_library_and_domain_symbols() {
    let cases: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("fixtures/symbol_resolution_cases.json")).unwrap();
    for case in cases {
        let resolved: Vec<&str> = case["resolved"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        let missing: Vec<&str> = case["missing"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        let result = GenericModel::<String>::sysml_block_def_checked(&resolved);
        if missing.is_empty() {
            let emitted = result.unwrap();
            assert_eq!(emitted, GenericModel::<String>::sysml_block_def());
            assert!(validate_sysml_v2(emitted).disposition.is_satisfied());
        } else {
            assert_eq!(result.unwrap_err(), missing);
        }
    }
}
