// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Sparse proof validation and opaque contribution checks.

use p4kx_extraction::{
    Archive, PhysicalSource, SourceCoordinateOmission, SourceDetails, SourceEvidenceError,
    SourceScope, SourceUse, SourceUseRole, SourceUses, SourceValue,
};
use p4kx_testkit::{ArchiveBuilder, TestEntry};
use serde_json::{Value, json};

use crate::source_minimal_proof as support;
use crate::source_minimal_proof::TestResult;

pub(super) fn opaque_contributors_force_incompleteness_but_attempts_and_references_do_not_taint_literals()
-> TestResult {
    for encrypted in [false, true] {
        let entry = TestEntry::stored("source.bin", b"same");
        let (_directory, archive) = support::open(&ArchiveBuilder::new().entry(if encrypted {
            entry.encrypted()
        } else {
            entry
        }))?;
        let sources = SourceUses::new();
        let scope = support::source(&sources, &archive, SourceDetails::CaptureLimited)?;
        for role in [
            SourceUseRole::Payload,
            SourceUseRole::Selector,
            SourceUseRole::Conversion,
        ] {
            opaque_role(&sources, &scope, role, encrypted)?;
        }
        let attempts = [
            scope.at_opaque(
                SourceUseRole::Attempted,
                SourceCoordinateOmission::CaptureLimit,
            )?,
            scope.at_opaque(
                SourceUseRole::Reference,
                SourceCoordinateOmission::CaptureLimit,
            )?,
        ];
        let literal = sources.value(SourceValue::Present("tool literal"), &attempts, true);
        assert!(
            literal.origin().source_set_complete(),
            "noncontributors do not make a complete literal incomplete"
        );
        assert_eq!(
            literal.origin().contains_encrypted_source(),
            Some(false),
            "untouched encrypted targets and failed inputs do not taint literals"
        );
        assert!(
            literal.origin().source_uses_refs().is_empty(),
            "attempt/reference inventory rows never become actual field contributors"
        );
        let foreign_literal =
            SourceUses::new().value(SourceValue::Present("tool literal"), &attempts, true);
        assert!(
            foreign_literal.origin().source_set_complete(),
            "foreign attempts and references also cannot taint a complete literal"
        );
        assert_eq!(
            foreign_literal.origin().contains_encrypted_source(),
            Some(false),
            "foreign noncontributors supply no encrypted value origin"
        );
    }
    Ok(())
}

pub(super) fn opaque_conversion_preserves_original_owner_domain_role_and_known_encrypted_witness()
-> TestResult {
    let (_directory, archive) = support::open(
        &ArchiveBuilder::new().entry(TestEntry::stored("source.bin", b"same").encrypted()),
    )?;
    let sources = SourceUses::new();
    let scope = support::source(&sources, &archive, SourceDetails::CaptureLimited)?;
    let actual = support::used(&scope)?;
    let opaque = sources.opaque_use(&actual, SourceCoordinateOmission::CaptureLimit)?;
    let repeated = sources.opaque_use(&actual, SourceCoordinateOmission::CaptureLimit)?;
    assert_eq!(
        opaque.reference(),
        repeated.reference(),
        "repeated omissions retain one genuine use with no capture-cap consumption"
    );
    assert_eq!(
        opaque.description().source(),
        actual.description().source(),
        "fallback preserves actual original source"
    );
    assert_eq!(
        opaque.description().scope(),
        actual.description().scope(),
        "fallback preserves original payload domain"
    );
    assert_eq!(
        opaque.role(),
        actual.role(),
        "fallback preserves actual role"
    );
    let value = sources.value(
        SourceValue::Present(()),
        std::slice::from_ref(&opaque),
        true,
    );
    assert_eq!(
        value.origin().contains_encrypted_source(),
        Some(true),
        "opaque fallback never loses a known encrypted input"
    );
    assert!(
        !value.origin().source_set_complete(),
        "missing coordinate stays explicit even when caller asks for true completeness"
    );
    foreign_opaque(&archive, &actual, opaque)?;
    Ok(())
}

pub(super) fn sparse_schema(wire: &Value) -> TestResult {
    let schema = serde_json::to_value(schemars::schema_for!(PhysicalSource))?;
    let validator = jsonschema::validator_for(&schema)?;
    let source = wire
        .get("sources")
        .and_then(|sources| sources.get(0))
        .ok_or("physical source rows")?;
    assert!(
        validator.is_valid(source),
        "actual sparse producer proof validates"
    );
    let mut false_full = source.clone();
    *false_full
        .get_mut("details")
        .ok_or("physical source rows")? = json!("full");
    assert!(
        !validator.is_valid(&false_full),
        "minimal proof cannot claim complete stored detail retention"
    );
    let mut numeric = source.clone();
    *numeric
        .get_mut("entryIndex")
        .ok_or("physical source rows")? = json!(0);
    assert!(
        !validator.is_valid(&numeric),
        "new original ordinals are canonical decimal strings on the wire"
    );
    Ok(())
}

fn opaque_role(
    sources: &SourceUses,
    scope: &SourceScope,
    role: SourceUseRole,
    encrypted: bool,
) -> TestResult {
    let opaque = scope.at_opaque(role, SourceCoordinateOmission::CaptureLimit)?;
    let value = sources.value(
        SourceValue::Present(9_u32),
        std::slice::from_ref(&opaque),
        true,
    );
    assert!(
        !value.origin().source_set_complete(),
        "caller true cannot override an actual omitted coordinate for {role:?}"
    );
    assert_eq!(
        value.origin().contains_encrypted_source(),
        if encrypted { Some(true) } else { None },
        "known encrypted dominates incompleteness while plain omitted coordinates stay unknown"
    );
    assert_eq!(
        value.origin().source_uses_refs(),
        std::slice::from_ref(opaque.reference()),
        "the real input witness remains directly referenced"
    );
    let location = serde_json::to_value(opaque.description().location())?;
    assert_eq!(
        location,
        json!({"type":"opaque","reason":"captureLimit"}),
        "omitted coordinates do not invent unsupported, unreadable, absent or source paths"
    );
    Ok(())
}

fn foreign_opaque(archive: &Archive, actual: &SourceUse, opaque: SourceUse) -> TestResult {
    let other = SourceUses::new();
    let other_scope = support::source(&other, archive, SourceDetails::CaptureLimited)?;
    let other_actual = support::used(&other_scope)?;
    assert_eq!(
        actual.reference(),
        other_actual.reference(),
        "fixture has deliberately colliding document-local refs"
    );
    assert!(
        matches!(
            other.opaque_use(actual, SourceCoordinateOmission::CaptureLimit),
            Err(SourceEvidenceError::ForeignEvidence)
        ),
        "equal serialized IDs cannot restore authority in another owner"
    );
    let foreign = other.value(SourceValue::Present(()), &[opaque], true);
    assert!(
        foreign.origin().source_uses_refs().is_empty() && !foreign.origin().source_set_complete(),
        "foreign fallback contributes neither original ref nor encrypted claim"
    );
    assert_eq!(
        foreign.origin().contains_encrypted_source(),
        None,
        "foreign encrypted evidence cannot establish this owner's encrypted marker"
    );
    Ok(())
}
