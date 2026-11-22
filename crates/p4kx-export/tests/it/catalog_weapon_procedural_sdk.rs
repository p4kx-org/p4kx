// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::build_catalog;
use p4kx_export::catalog_queries::equipment::weapon_procedural::{
    WeaponProceduralAnimationSnapshot, WeaponProceduralClipSnapshot,
    with_weapon_procedural_animations_from, with_weapon_procedural_clips,
};
use p4kx_export::catalog_queries::{CatalogRecordSelection, DatasetAvailability};
use p4kx_extraction::DataForgeEntry;
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::health_origin_assertions as proof;

#[path = "catalog_weapon_procedural_sdk_fixture.rs"]
mod fixture;

const ANIMATIONS: &str = "/definitions/weaponProceduralAnimations";
const CLIPS: &str = "/definitions/weaponProceduralClips";

#[test]
fn weapon_procedural_sdk_matches_original_rows_and_origins() -> TestResult {
    let database = fixture::database();
    for (origin, encrypted) in [(DcbOrigin::Plain, false), (DcbOrigin::Encrypted, true)] {
        let (_folder, archive) = common::open(&database, origin)?;
        let entry = DataForgeEntry::read(&archive)?;
        let catalog: Value =
            serde_json::from_slice(build_catalog(&archive, &common::options())?.json())?;
        let guids = CatalogRecordSelection::Guid(common::guid(fixture::ANIMATION_GUID).parse()?);
        let clip =
            with_weapon_procedural_animations_from(&archive, &entry, guids, None, |snapshot| {
                let clip = check_animations(&snapshot, encrypted)?;
                let scoped = json!({
                    "definitions": { "weaponProceduralAnimations": {
                        "availability": snapshot.availability(),
                        "rows": snapshot.rows()
                    }},
                    "provenance": snapshot.source_inventory()
                });
                compare(&catalog, &scoped, ANIMATIONS)?;
                Ok::<_, Box<dyn std::error::Error>>(clip)
            })??;
        with_weapon_procedural_clips(
            &archive,
            CatalogRecordSelection::RecordIndex(clip),
            None,
            |snapshot| {
                check_clip(&snapshot, encrypted)?;
                let scoped = json!({
                    "definitions": { "weaponProceduralClips": {
                        "availability": snapshot.availability(),
                        "rows": snapshot.rows()
                    }},
                    "provenance": snapshot.source_inventory()
                });
                compare(&catalog, &scoped, CLIPS)
            },
        )??;
    }
    Ok(())
}

fn check_clip(snapshot: &WeaponProceduralClipSnapshot<'_>, encrypted: bool) -> TestResult {
    let [row] = snapshot.rows() else {
        return Err("one selected original clip".into());
    };
    assert_eq!(
        row.source().record_index().value(),
        Some(&fixture::CLIP_INDEX),
        "the Clip query joins through the original physical index"
    );
    assert!(
        snapshot.available_languages().is_empty(),
        "clip field capture needs no localization Entry"
    );
    let raw = serde_json::to_value(row.fields())?;
    assert_eq!(
        raw.pointer("/roots/0/name/value"),
        Some(&json!("fixtureScale")),
        "the clip retains its original root member name"
    );
    assert_eq!(
        raw.pointer("/roots/0/name/containsEncryptedSource"),
        Some(&json!(encrypted)),
        "the original leaf field retains its encryption marker"
    );
    Ok(())
}

fn check_animations(
    snapshot: &WeaponProceduralAnimationSnapshot<'_>,
    encrypted: bool,
) -> TestResult<u32> {
    let [original, alias] = snapshot.rows() else {
        return Err("two distinct physical animation owners for the selected GUID".into());
    };
    assert_eq!(
        [
            original.source().record_index().value(),
            alias.source().record_index().value()
        ],
        [
            Some(&fixture::ANIMATION_INDEX),
            Some(&fixture::ANIMATION_ALIAS_INDEX)
        ],
        "the physical alias follows its original declaration order"
    );
    check_animation_scope(snapshot);
    let authored = original.setups().value().ok_or("original ordered setups")?;
    assert_eq!(
        authored.len(),
        2,
        "both authored setup occurrences are kept"
    );
    for setup in authored {
        let setup = setup.value().ok_or("one original setup value")?;
        assert_eq!(
            setup.filter().stance_state.value().copied(),
            Some("FixtureStand"),
            "the typed predicate keeps the original stance value"
        );
        assert_eq!(
            setup.additional_fields().roots().len(),
            1,
            "the extra authored setup member remains in the source arena"
        );
        let [clip] = setup
            .clips()
            .value()
            .ok_or("original clip references")?
            .as_slice()
        else {
            return Err("one original clip link for each setup".into());
        };
        assert_eq!(
            clip.origin().contains_encrypted_source(),
            Some(encrypted),
            "each clip reference uses the original DataForge encryption state"
        );
        assert_eq!(
            clip.value().and_then(|link| link.resolved_index()),
            Some(fixture::CLIP_INDEX),
            "the typed setup exposes the actual clip Record"
        );
    }
    Ok(fixture::CLIP_INDEX)
}

fn check_animation_scope(snapshot: &WeaponProceduralAnimationSnapshot<'_>) {
    assert!(
        matches!(snapshot.availability(), DatasetAvailability::Available),
        "the admitted physical animation definitions establish availability"
    );
    assert!(
        snapshot.available_languages().is_empty(),
        "the animation setup query has no localization dependency"
    );
    assert!(
        snapshot.coverage().unreadable_bodies.is_empty(),
        "both selected physical animation bodies are readable"
    );
}

fn compare(catalog: &Value, scoped: &Value, path: &str) -> TestResult {
    let actual = proof::with_origins(proof::at(scoped, path)?.clone(), scoped)?;
    let expected = proof::with_origins(proof::at(catalog, path)?.clone(), catalog)?;
    assert_eq!(
        actual, expected,
        "canonical source rows and origins at {path}"
    );
    proof::closure(scoped)
}
