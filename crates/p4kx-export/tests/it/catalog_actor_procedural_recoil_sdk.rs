// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::sdk_full_proof as full_proof;
use p4kx_export::catalog_queries::equipment::actor_procedural_recoil::{
    ActorProceduralRecoilConfigSnapshot, ActorProceduralRecoilModifierSnapshot,
};
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, DatasetAvailability,
};
use p4kx_export::{CatalogError, build_catalog};
use p4kx_extraction::CancelToken;
use serde_json::{Value, json};

use super::fixture::{
    self, ACTOR_MODIFIER_INDEX, ACTOR_RECOIL_ALIAS_INDEX, ACTOR_RECOIL_GUID, ACTOR_RECOIL_INDEX,
};
use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::health_origin_assertions as proof;

const CONFIGS: &str = "/definitions/actorProceduralRecoilConfigs";
const MODIFIERS: &str = "/definitions/actorProceduralRecoilModifiers";

#[test]
fn actor_recoil_sdk_matches_unreachable_original_tables_and_their_sources() -> TestResult {
    let database = fixture::database_with_actor_recoil();
    for (origin, encrypted) in [(DcbOrigin::Plain, false), (DcbOrigin::Encrypted, true)] {
        let (_folder, archive) = common::open(&database, origin)?;
        let catalog: Value = serde_json::from_slice(
            build_catalog(&archive, common::options().with_full_provenance())?.json(),
        )?;
        let operating = proof::at(
            &catalog,
            "/configurations/weaponActions/rows/0/operatingConfiguration/actorProceduralRecoilConfig/value",
        )?;
        assert_eq!(
            operating["status"], "dangling",
            "the weapon does not reach these definitions"
        );
        let session = full_proof::open(&archive, CatalogLanguageSelection::Selected(&[]), None)?;
        let parsed = session.parsed(None)?;
        let target = parsed.with_actor_procedural_recoil_configs(
            CatalogRecordSelection::All,
            None,
            |snapshot| -> TestResult<u32> {
                let target = check_configs(&snapshot, encrypted)?;
                let scoped = json!({
                    "definitions": {"actorProceduralRecoilConfigs": {
                        "availability": snapshot.availability(), "rows": snapshot.rows()
                    }},
                    "provenance": snapshot.source_inventory(),
                });
                compare_sources(&catalog, &scoped, CONFIGS)?;
                Ok(target)
            },
        )??;
        parsed.with_actor_procedural_recoil_modifiers(
            CatalogRecordSelection::RecordIndex(target),
            None,
            |snapshot| -> TestResult {
                check_modifier(&snapshot, encrypted)?;
                let scoped = json!({
                    "definitions": {"actorProceduralRecoilModifiers": {
                        "availability": snapshot.availability(), "rows": snapshot.rows()
                    }},
                    "provenance": snapshot.source_inventory(),
                });
                compare_sources(&catalog, &scoped, MODIFIERS)
            },
        )??;
    }
    Ok(())
}

fn check_configs(
    snapshot: &ActorProceduralRecoilConfigSnapshot<'_>,
    encrypted: bool,
) -> TestResult<u32> {
    let [original, alias] = snapshot.rows() else {
        return Err("all physical actors, including the unreferenced alias".into());
    };
    assert_eq!(
        original.source().record_index().value(),
        Some(&ACTOR_RECOIL_INDEX),
        "the original setup uses its physical Record index"
    );
    assert_eq!(
        alias.source().record_index().value(),
        Some(&ACTOR_RECOIL_ALIAS_INDEX),
        "the second physical occurrence is preserved"
    );
    check_config_scope(snapshot);
    let setups = original
        .setups()
        .value()
        .ok_or("original ordered setup slots")?;
    assert_eq!(setups.len(), 2, "both authored setup positions survive");
    let mut target = None;
    for setup in setups {
        let setup = setup.value().ok_or("original setup")?;
        assert_eq!(
            setup.filter().stance_state.value().copied(),
            Some("FixtureStand"),
            "the authored posture filter is preserved"
        );
        assert_eq!(
            setup.modifier().origin().contains_encrypted_source(),
            Some(encrypted),
            "the original modifier reference retains its encryption marker"
        );
        assert_eq!(
            setup.additional_fields().roots().len(),
            1,
            "the unprojected authored setup field remains"
        );
        let physical = setup
            .modifier()
            .value()
            .and_then(|link| link.resolved_index());
        assert_eq!(
            physical,
            Some(ACTOR_MODIFIER_INDEX),
            "each setup resolves its own original modifier link"
        );
        target = physical;
    }
    target.ok_or_else(|| "original modifier target index".into())
}

fn check_config_scope(snapshot: &ActorProceduralRecoilConfigSnapshot<'_>) {
    assert!(
        matches!(snapshot.availability(), DatasetAvailability::Available),
        "the original type declaration is present"
    );
    assert_eq!(
        snapshot.available_languages(),
        [],
        "a recoil query loads no locale sources"
    );
    assert_eq!(
        snapshot.coverage().incomplete_fields,
        [] as [u32; 0],
        "both complete setup owners were read"
    );
}

fn check_modifier(
    snapshot: &ActorProceduralRecoilModifierSnapshot<'_>,
    encrypted: bool,
) -> TestResult {
    let [row] = snapshot.rows() else {
        return Err("one selected original actor recoil modifier".into());
    };
    assert_eq!(
        row.source().record_index().value(),
        Some(&ACTOR_MODIFIER_INDEX),
        "the modifier query returns the exact original Record"
    );
    assert_eq!(
        snapshot.coverage().unreadable_bodies,
        [] as [u32; 0],
        "unselected record bodies are not read"
    );
    let fields = serde_json::to_value(row.fields())?;
    let raw = proof::at(&fields, "/roots/0")?;
    assert_eq!(
        raw.pointer("/name/value"),
        Some(&json!("kickScale")),
        "the original modifier field name is retained"
    );
    assert_eq!(
        raw.pointer("/name/containsEncryptedSource"),
        Some(&json!(encrypted)),
        "the raw modifier field retains its immediate encryption marker"
    );
    Ok(())
}

fn compare_sources(catalog: &Value, scoped: &Value, path: &str) -> TestResult {
    let actual = proof::with_origins(proof::at(scoped, path)?.clone(), scoped)?;
    let expected = proof::with_origins(proof::at(catalog, path)?.clone(), catalog)?;
    assert_eq!(
        actual, expected,
        "ordered canonical data and exact origins at {path}"
    );
    proof::closure(scoped)
}

#[test]
fn actor_recoil_sdk_limits_record_bodies_to_the_exact_type_and_selection() -> TestResult {
    const UNREADABLE: u32 = ACTOR_RECOIL_ALIAS_INDEX + 1;
    let database = fixture::database_with_actor_recoil()
        .record(fixture::actor_recoil_record("Unreadable").with_instance_index(u16::MAX));
    let (_folder, archive) = common::open(&database, DcbOrigin::Plain)?;
    let session =
        CatalogQuerySession::open(&archive, CatalogLanguageSelection::Selected(&[]), None)?;
    let parsed = session.parsed(None)?;
    let original = CatalogRecordSelection::RecordIndex(ACTOR_RECOIL_INDEX);
    parsed.with_actor_procedural_recoil_configs(original, None, |snapshot| {
        assert_eq!(
            snapshot.rows().len(),
            1,
            "one physical Record is selected by index"
        );
        assert_eq!(
            snapshot.coverage().unreadable_bodies,
            [] as [u32; 0],
            "unselected record bodies are not read"
        );
    })?;

    parsed.with_actor_procedural_recoil_configs(
        CatalogRecordSelection::Guid(common::guid(ACTOR_RECOIL_GUID).parse()?),
        None,
        |snapshot| {
            assert_eq!(
                snapshot.rows().len(),
                2,
                "readable physical GUID aliases remain"
            );
            assert_eq!(
                snapshot.coverage().unreadable_bodies,
                [UNREADABLE],
                "the unreadable GUID occurrence is reported"
            );
        },
    )?;
    check_distinct_types(&parsed, original)?;
    parsed.with_actor_procedural_recoil_modifiers(
        CatalogRecordSelection::All,
        None,
        |snapshot| check_modifier(&snapshot, false),
    )??;

    let cancelled = CancelToken::default();
    cancelled.cancel();
    assert!(
        matches!(
            parsed.with_actor_procedural_recoil_configs(original, Some(&cancelled), |_| ()),
            Err(CatalogError::Cancelled)
        ),
        "a pre-cancelled request yields no snapshot"
    );
    Ok(())
}

fn check_distinct_types(
    parsed: &p4kx_export::catalog_queries::ParsedCatalogQueries<'_>,
    original: CatalogRecordSelection,
) -> TestResult {
    let action_recoil = CatalogRecordSelection::RecordIndex(super::RECOIL);
    parsed.with_actor_procedural_recoil_configs(action_recoil, None, |snapshot| {
        assert!(
            snapshot.rows().is_empty(),
            "the action recoil type is distinct"
        );
        assert!(
            matches!(snapshot.availability(), DatasetAvailability::Available),
            "the original type declaration is present"
        );
        assert_eq!(
            snapshot.coverage().unreadable_bodies,
            [] as [u32; 0],
            "unselected record bodies are not read"
        );
    })?;
    parsed.with_actor_procedural_recoil_modifiers(original, None, |snapshot| {
        assert!(
            snapshot.rows().is_empty(),
            "a setup owner is not a modifier"
        );
        assert!(
            matches!(snapshot.availability(), DatasetAvailability::Available),
            "the original type declaration is present"
        );
    })?;
    Ok(())
}

#[test]
fn actor_recoil_sdk_reports_declared_empty_and_absent_families_separately() -> TestResult {
    let (_folder, archive) = common::open(&fixture::database(), DcbOrigin::Plain)?;
    let session =
        CatalogQuerySession::open(&archive, CatalogLanguageSelection::Selected(&[]), None)?;
    let parsed = session.parsed(None)?;
    let all = CatalogRecordSelection::All;
    parsed.with_actor_procedural_recoil_configs(all, None, |snapshot| {
        assert!(
            snapshot.rows().is_empty(),
            "no physical Record of the selected family is declared"
        );
        assert!(
            matches!(snapshot.availability(), DatasetAvailability::Available),
            "the original type declaration is present"
        );
    })?;
    parsed.with_actor_procedural_recoil_modifiers(all, None, |snapshot| {
        assert!(
            snapshot.rows().is_empty(),
            "no physical Record of the selected family is declared"
        );
        assert!(
            matches!(snapshot.availability(), DatasetAvailability::Absent),
            "the original type declaration is absent"
        );
    })?;
    Ok(())
}
