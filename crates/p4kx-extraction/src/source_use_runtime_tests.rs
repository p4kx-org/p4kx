// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeSet, HashSet};
use std::ptr;
use std::sync::Arc;

use serde_json::{Value, json};

use crate::{
    InputScope, SourceInput, SourceLocation, SourceUseRef, SourceUseRole, SourceUses, SourceValue,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn field_provenance_shares_reference_text_in_all_registry_modes() -> TestResult {
    for (sources, expected) in [
        (SourceUses::new(), "u2:u0:payload:selector:d4:4:name"),
        (SourceUses::with_compact_coordinates(), "c2:u0psd4:name"),
        (SourceUses::with_compact_use_refs(), "u0"),
        (SourceUses::with_batched_use_refs(), "u0.0"),
    ] {
        let scope = sources.observe(SourceInput::Unknown, InputScope::Payload)?;
        let field = SourceLocation::DataForge {
            record_index: 4,
            field: "name".to_owned(),
        };
        let first = scope.at(field.clone(), SourceUseRole::Selector)?;
        let second = scope.at(field, SourceUseRole::Conversion)?;
        let reference = first.reference();
        let clone = reference.clone();
        assert!(
            ptr::eq(reference.as_str(), clone.as_str()),
            "a reference clone keeps the registered text allocation"
        );
        let sourced = sources.value(
            SourceValue::Present("value"),
            &[second.clone(), first],
            true,
        );
        let origin = sourced.origin();
        let distinct = origin.source_uses_refs();
        let retained = distinct
            .iter()
            .find(|candidate| candidate.as_str() == expected)
            .ok_or("retained selector reference")?;
        assert!(
            ptr::eq(retained.as_str(), clone.as_str()),
            "actual field evidence shares the registered reference text"
        );
        assert_ne!(retained, second.reference(), "roles remain distinct");
        assert_eq!(distinct.len(), 2, "both contributors remain");
        assert!(distinct.is_sorted(), "reference order stays lexical");
        assert_eq!(
            serde_json::to_value(retained)?,
            json!(expected),
            "reference wire bytes"
        );
    }
    Ok(())
}

#[test]
fn decoded_reference_storage_preserves_wire_identity_and_inline_size() -> TestResult {
    assert!(
        size_of::<SourceUseRef>() <= size_of::<String>(),
        "reference storage must not grow the reader carrier"
    );
    let expected = "reference with é";
    let registered = SourceUseRef::registered(expected.to_owned());
    let decoded = SourceUseRef::decoded(expected.into());
    let smaller = SourceUseRef::registered("earlier reference".to_owned());
    assert_eq!(
        registered, decoded,
        "storage mode has no effect on equality"
    );
    assert_eq!(
        HashSet::from([registered.clone(), decoded.clone()]).len(),
        1,
        "storage mode has no effect on hashing"
    );
    assert_eq!(
        BTreeSet::from([registered.clone(), decoded.clone(), smaller.clone()])
            .into_iter()
            .collect::<Vec<_>>(),
        [smaller, registered.clone()],
        "ordering and duplicate detection use the same UTF-8 text"
    );
    assert_eq!(
        serde_json::to_vec(&registered)?,
        serde_json::to_vec(&decoded)?,
        "storage mode leaves the wire representation identical"
    );
    Ok(())
}

#[test]
fn genuine_handles_share_the_original_row_and_keep_identical_freeze_bytes() -> TestResult {
    let sources = SourceUses::new();
    let scope = sources.observe(SourceInput::Unknown, InputScope::Payload)?;
    let at = || {
        scope.at(
            SourceLocation::Subresource {
                namespace: "synthetic".to_owned(),
                indices: vec![3, 7],
                field: "name".to_owned(),
            },
            SourceUseRole::Selector,
        )
    };
    let first = at()?;
    let before = serde_json::to_vec(&sources.freeze()?)?;
    let repeated = at()?;
    let cloned = first.clone();
    assert!(
        Arc::ptr_eq(&first.description, &repeated.description),
        "repeat observation borrows the same registered row"
    );
    assert!(
        Arc::ptr_eq(&first.description, &cloned.description),
        "cloning a handle never copies its reference or location strings"
    );
    assert_eq!(
        before,
        serde_json::to_vec(&sources.freeze()?)?,
        "runtime sharing changes no physical source or use wire bytes"
    );
    assert_eq!(
        first.reference(),
        first.description().id(),
        "the ref getter borrows the original row"
    );
    assert_eq!(
        first.role(),
        first.description().role(),
        "the role getter borrows the original row"
    );
    assert_eq!(
        first.description().source(),
        scope.source_ref(),
        "the original physical owner remains bound"
    );
    assert_eq!(
        first.description().scope(),
        InputScope::Payload,
        "the original byte domain remains bound"
    );
    let other = SourceUses::new().value(SourceValue::Present(()), &[cloned], true);
    assert!(
        !other.origin().source_set_complete(),
        "a cheap clone cannot acquire another inventory owner"
    );
    assert!(
        other.origin().source_uses_refs().is_empty(),
        "foreign handle refs remain excluded"
    );
    Ok(())
}

#[test]
fn subresource_ordinals_keep_full_u64_values_and_stable_handles_on_the_wire() -> TestResult {
    let sources = SourceUses::new();
    let scope = sources.observe(SourceInput::Unknown, InputScope::Payload)?;
    let indices = [0, 9_007_199_254_740_993, u64::MAX];
    let at = || {
        scope.at(
            SourceLocation::Subresource {
                namespace: "synthetic".to_owned(),
                indices: indices.to_vec(),
                field: "name".to_owned(),
            },
            SourceUseRole::Selector,
        )
    };
    let first = at()?;
    let repeated = at()?;
    assert!(
        Arc::ptr_eq(&first.description, &repeated.description),
        "full-width coordinates deduplicate to the same original registered row"
    );
    assert_eq!(
        first.reference().as_str(),
        "u2:u0:payload:selector:s9:synthetic3:0.9007199254740993.18446744073709551615.4:name",
        "wire formatting does not change the established logical use identity"
    );
    let SourceLocation::Subresource {
        indices: original, ..
    } = first.description().location()
    else {
        return Err("the original handle lost its subresource location".into());
    };
    assert_eq!(
        original.as_slice(),
        &indices,
        "the runtime API retains each original u64 without a text round trip"
    );
    let inventory = sources.freeze()?;
    assert_eq!(
        inventory.uses().len(),
        1,
        "repeated coordinates retain one use"
    );
    assert_subresource_ordinal_wire(&serde_json::to_value(inventory)?)?;
    Ok(())
}

fn assert_subresource_ordinal_wire(wire: &Value) -> TestResult {
    let location = wire
        .pointer("/uses/0/location")
        .ok_or("frozen subresource location")?;
    assert_eq!(
        location.get("indices").ok_or("frozen original ordinals")?,
        &json!(["0", "9007199254740993", "18446744073709551615"]),
        "every source ordinal is an exact canonical decimal string"
    );
    let schema = serde_json::to_value(schemars::schema_for!(SourceLocation))?;
    let validator = jsonschema::validator_for(&schema)?;
    assert!(
        validator.is_valid(location),
        "the actual producer location validates"
    );
    let mut numeric = location.clone();
    *numeric
        .get_mut("indices")
        .ok_or("ordinal mutation target")? = json!([0, 9_007_199_254_740_993_u64, u64::MAX]);
    assert!(
        !validator.is_valid(&numeric),
        "the generated schema rejects JSON-number ordinals that consumers can round"
    );
    Ok(())
}
