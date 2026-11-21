// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::equipment::{
    AmmoBinding, WeaponMagazineDependencySnapshot, WeaponMagazineOutcome,
    with_weapon_magazine_dependencies_from,
};
use p4kx_export::{CatalogError, CatalogOptions, CatalogPublicationMode, build_catalog_from};
use p4kx_extraction::{CancelToken, DataForgeEntry, Filter};
use serde_json::{Value, json};

use super::{Case, DcbOrigin, TestResult, common, fixture, proof};

const CONTAINERS: &str = "/configurations/ammoContainers/rows";
const BINDINGS: &str = "/declarations/ammoBindings/rows";

#[test]
fn scoped_magazine_dependency_matches_canonical_rows_and_original_alias_selection() -> TestResult {
    for (case, origin, encrypted) in [
        (Case::Unique, DcbOrigin::Plain, false),
        (Case::Unique, DcbOrigin::Encrypted, true),
        (Case::DuplicateGuid, DcbOrigin::Encrypted, true),
    ] {
        let (_folder, archive) = common::open(&fixture::build(case), origin)?;
        let entry = DataForgeEntry::read(&archive)?;
        let forge = entry.open()?;
        let weapon = forge
            .record_by_guid(common::guid(fixture::WEAPON).parse()?)
            .ok_or("weapon fixture")?
            .index();
        let magazine = forge
            .record_by_guid(common::guid(fixture::MAGAZINE).parse()?)
            .ok_or("first physical magazine fixture")?
            .index();
        let filter = Filter::parse(fixture::FILTER)?;
        let catalog: Value = serde_json::from_slice(
            build_catalog_from(
                &archive,
                &entry,
                &CatalogOptions {
                    filter: Some(&filter),
                    publication_mode: CatalogPublicationMode::DiagnosticPartial,
                    ..common::options()
                },
            )?
            .json(),
        )?;
        with_weapon_magazine_dependencies_from(
            &archive,
            &entry,
            weapon,
            Some(&filter),
            None,
            |snapshot| compare(&snapshot, &catalog, magazine, encrypted),
        )??;
    }
    Ok(())
}

fn compare(
    snapshot: &WeaponMagazineDependencySnapshot<'_>,
    catalog: &Value,
    magazine: u32,
    encrypted: bool,
) -> TestResult {
    let [owner] = snapshot.rows() else {
        return Err("one primary route for the selected weapon".into());
    };
    let [dependency] = owner.dependencies.as_slice() else {
        return Err("exactly one original off-owner container and binding".into());
    };
    let Some(WeaponMagazineOutcome::LinkedContainer {
        magazine_record_index,
        container_ordinal,
    }) = owner.resolution.outcome().value()
    else {
        return Err("the resolved primary route supplies the physical slot".into());
    };
    assert_eq!(
        magazine_record_index.value(),
        Some(&magazine),
        "the canonical route keeps the selected physical alias"
    );
    assert_eq!(
        dependency.container.owner().record_index().value(),
        Some(&magazine),
        "the container is owned by the reached magazine"
    );
    assert_eq!(
        dependency.container.owner().component_ordinal().value(),
        container_ordinal.value(),
        "the container exposes the exact original slot"
    );
    let AmmoBinding::ContainerSlots { value } = &dependency.binding else {
        return Err("the dependency binding belongs to the same container".into());
    };
    assert_eq!(
        value.owner().record_index().value(),
        Some(&magazine),
        "both rows identify the same physical owner"
    );
    assert_eq!(
        value.primary_ammo().origin().contains_encrypted_source(),
        Some(encrypted),
        "the original linked container field carries its source encryption state"
    );
    let scoped = json!({
        "configurations": {"ammoContainers": {"rows": [&dependency.container]}},
        "declarations": {"ammoBindings": {"rows": [&dependency.binding]}},
        "provenance": snapshot.source_inventory(),
    });
    compare_wire(&scoped, catalog, magazine)
}

fn compare_wire(scoped: &Value, catalog: &Value, magazine: u32) -> TestResult {
    for pointer in [CONTAINERS, BINDINGS] {
        let [observed] = proof::rows(scoped, pointer)? else {
            return Err("one scoped dependency row".into());
        };
        let expected = proof::rows(catalog, pointer)?
            .iter()
            .find(|row| {
                row.pointer("/ownerRecordIndex/value")
                    .or_else(|| row.pointer("/value/ownerRecordIndex/value"))
                    .and_then(Value::as_u64)
                    == Some(u64::from(magazine))
            })
            .ok_or("canonical dependency row")?;
        assert_eq!(
            proof::with_origins(observed.clone(), scoped)?,
            proof::with_origins(expected.clone(), catalog)?,
            "canonical value and original source uses at {pointer}"
        );
    }
    proof::closure(scoped)
}

#[test]
fn scoped_magazine_dependency_reports_absence_and_honors_cancellation() -> TestResult {
    let (_folder, archive) =
        common::open(&fixture::build(Case::NoContainer), DcbOrigin::Encrypted)?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let weapon = forge
        .record_by_guid(common::guid(fixture::WEAPON).parse()?)
        .ok_or("weapon fixture")?
        .index();
    with_weapon_magazine_dependencies_from(
        &archive,
        &entry,
        weapon,
        None,
        None,
        |snapshot| -> TestResult {
            let [owner] = snapshot.rows() else {
                return Err("the exact primary attempt remains visible".into());
            };
            assert!(
                owner.dependencies.is_empty(),
                "a proved absence yields no fabricated rows"
            );
            assert!(
                owner.dependency_coverage.unreadable_bodies.is_empty(),
                "the consulted body was readable"
            );
            assert!(
                matches!(
                    owner.resolution.outcome().value(),
                    Some(WeaponMagazineOutcome::LinkedWithoutContainer { .. }),
                ),
                "the primary target has a proved absence of a container"
            );
            assert!(
                owner.resolution.outcome().origin().source_set_complete(),
                "absence comes from complete source evidence"
            );
            Ok(())
        },
    )??;

    let cancel = CancelToken::new();
    cancel.cancel();
    let result = with_weapon_magazine_dependencies_from(
        &archive,
        &entry,
        weapon,
        None,
        Some(&cancel),
        |_| {},
    );
    assert!(
        matches!(result, Err(CatalogError::Cancelled)),
        "the query stops before providing a snapshot"
    );
    Ok(())
}
