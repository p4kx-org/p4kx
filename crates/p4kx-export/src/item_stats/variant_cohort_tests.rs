// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_extraction::{
    Archive, DataForge, Filter, SourceLocation, SourceUseDescription, SourceUseRole,
    SourceValueState, Value,
};
use p4kx_testkit::{ArchiveBuilder, TestEntry, TestRecord, TestType, TestValue};

use super::{
    AMMO, AMMO_CONTAINER, ENTITY, FIRST_MAGAZINE, PRIMARY_MAGAZINE_FIELD, SECOND_MAGAZINE,
    TestResult, WEAPON_COMPONENT, ammo, fixture_types, magazine, weapon_at,
};
use crate::catalog_queries::CatalogRecordSelection;
use crate::catalog_queries::equipment::{ItemSnapshot, with_items};
use crate::item_variant::{self, ItemComponentKey};
use crate::record_fields::component;
use crate::{
    CatalogError, CatalogOptions, CatalogPublicationMode, ExportDiagnosticCode,
    ExportDiagnosticStage, Kind, build_catalog,
};

const BASE: u32 = 0;
const SELECTED: u32 = 1;
const PEER: u32 = 2;
const UNREADABLE_MAGAZINE: u32 = 4;
const UNREADABLE_AMMO: u32 = 6;
const AMMO_FIELD: &str = "ammoParamsRecord";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ComponentCohort {
    Different,
    Matching,
}

#[test]
fn unequal_variant_components_do_not_read_the_competing_ammunition() -> TestResult {
    with_selected(ComponentCohort::Different, |snapshot| {
        let [selected] = snapshot.rows() else {
            return Err("exact physical selected Item".into());
        };
        let choice = selected.variant_of();
        let base = choice.value().ok_or("known matching Item base")?;
        assert_eq!(
            base.target_record_index().value().copied(),
            Some(BASE),
            "the readable equal Item still supplies the physical variant base"
        );
        assert!(
            choice.origin().source_set_complete(),
            "a competing Item with different components cannot affect the group"
        );
        assert!(
            snapshot.coverage().incomplete_fields.is_empty(),
            "the selected physical Item is fully covered"
        );
        assert!(
            !snapshot
                .source_inventory()
                .iter_uses()
                .any(attempted_ammo_body),
            "the pruned peer's unreadable physical ammo Record never enters source accounting"
        );
        Ok(())
    })
}

#[test]
fn matching_variant_components_keep_an_unreadable_ammunition_operand() -> TestResult {
    with_selected(ComponentCohort::Matching, |snapshot| {
        let [selected] = snapshot.rows() else {
            return Err("exact physical selected Item".into());
        };
        let choice = selected.variant_of();
        assert_eq!(
            choice.state(),
            SourceValueState::Unreadable,
            "an equal component key can change the winner when its selected ammo is unreadable"
        );
        assert!(
            choice.value().is_none() && !choice.origin().source_set_complete(),
            "a readable candidate cannot substitute for the unreadable competing operand"
        );
        assert_eq!(
            snapshot.coverage().incomplete_fields.as_slice(),
            [SELECTED],
            "the covered owner retains the incomplete decision"
        );
        assert!(
            snapshot.source_inventory().iter_uses().any(|used| {
                attempted_ammo_body(used)
                    && selected.variant_failed_source_uses().contains(used.id())
                    && !choice.origin().source_uses_refs().contains(used.id())
            }),
            "the incomplete decision retains the attempted read at that exact physical ammo Record"
        );
        Ok(())
    })
}

#[test]
fn normal_variant_failure_retains_the_exact_attempt_and_diagnostic_row() -> TestResult {
    let (_folder, archive) = open_cohort(ComponentCohort::Matching)?;
    let filter = Filter::new("*/fixture/C_Selected");
    let mut options = CatalogOptions {
        filter: Some(&filter),
        ..CatalogOptions::new(std::time::UNIX_EPOCH)
    };
    let Err(CatalogError::IncompleteExport(refusal)) =
        build_catalog(&archive, options.with_full_provenance())
    else {
        return Err("Normal must refuse an undecidable selected Item variant".into());
    };
    let health = refusal.health();
    let item_health = health.kind(Kind::Items).ok_or("Item publication health")?;
    assert_eq!(
        item_health.failed_derived_projections, 1,
        "the selected Item is counted once independently of ordinary source reads"
    );
    let witness = health
        .diagnostics()
        .iter()
        .filter(|diagnostic| {
            diagnostic.kind == Some(Kind::Items)
                && diagnostic.stage == ExportDiagnosticStage::DerivedProjection
                && diagnostic.code == ExportDiagnosticCode::DerivedProjectionFailure
        })
        .flat_map(crate::ExportDiagnostic::examples)
        .find(|example| example.field_text().value() == Some("variantOf"))
        .ok_or("the accounted Item variant decision")?;
    assert_eq!(
        witness.record_index().value().copied(),
        Some(SELECTED),
        "the retained row is the failure subject"
    );
    let origin = witness.field().origin();
    assert!(
        !origin.source_set_complete(),
        "a failed derivation cannot certify its evidence"
    );
    assert!(
        refusal.source_inventory().iter_uses().any(|used| {
            attempted_ammo_body(used)
                && witness.failed_source_uses().contains(used.id())
                && !origin.source_uses_refs().contains(used.id())
        }),
        "the refusal retains the same original attempted physical ammunition read"
    );

    let receipt = serde_json::to_value(witness)?;
    assert!(
        receipt["failedSourceUses"]
            .as_array()
            .is_some_and(|attempts| !attempts.is_empty()),
        "the report serializes the original attempts independently of the field origin"
    );

    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    check_partial_publication(&archive, &options)
}

fn check_partial_publication(archive: &Archive, options: &CatalogOptions<'_>) -> TestResult {
    let partial = build_catalog(archive, options.with_full_provenance())?;
    let catalog: serde_json::Value = serde_json::from_slice(partial.json())?;
    assert_eq!(
        catalog
            .pointer("/items/0/variantOf/state")
            .and_then(serde_json::Value::as_str),
        Some("unreadable"),
        "diagnostic publication keeps the physical Item and its unavailable variant"
    );
    Ok(())
}

fn attempted_ammo_body(used: &SourceUseDescription) -> bool {
    used.role() == SourceUseRole::Attempted
        && matches!(used.location(), SourceLocation::DataForge { record_index, field }
            if *record_index == UNREADABLE_AMMO && field.is_empty())
}

fn with_selected(
    cohort: ComponentCohort,
    inspect: impl FnOnce(ItemSnapshot<'_>) -> TestResult,
) -> TestResult {
    let (_folder, archive) = open_cohort(cohort)?;
    with_items(
        &archive,
        CatalogRecordSelection::RecordIndex(SELECTED),
        None,
        inspect,
    )?
}

fn open_cohort(cohort: ComponentCohort) -> Result<(tempfile::TempDir, Archive), Box<dyn Error>> {
    let bytes = fixture(cohort);
    check_original_operands(&bytes, cohort)?;
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("Data.p4k");
    ArchiveBuilder::v2()
        .entry(TestEntry::stored("Data/Game2.dcb", bytes).encrypted())
        .write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?;
    Ok((folder, archive))
}

fn fixture(cohort: ComponentCohort) -> Vec<u8> {
    let grade = match cohort {
        ComponentCohort::Different => 2,
        ComponentCohort::Matching => 1,
    };
    fixture_types(TestType::WeakPointer(ENTITY.to_owned()))
        .record(candidate(1, "B_Base", FIRST_MAGAZINE, 1))
        .record(candidate(2, "C_Selected", FIRST_MAGAZINE, 1))
        .record(candidate(3, "A_Peer", SECOND_MAGAZINE, grade))
        .record(magazine(FIRST_MAGAZINE, 21))
        .record(magazine(SECOND_MAGAZINE, 22))
        .record(ammo(21, "FixtureAmmoFirst", 100.0))
        .record(ammo(22, "FixtureAmmoSecond", 100.0).with_instance_index(u16::MAX))
        .build()
}

fn candidate(guid: u128, name: &str, magazine: &str, grade: i32) -> TestRecord {
    weapon_at(
        guid,
        name,
        format!("libs/foundry/records/entities/scitem/fixture/{name}.xml"),
        TestValue::pointer(magazine),
        grade,
    )
}

fn check_original_operands(bytes: &[u8], cohort: ComponentCohort) -> TestResult {
    let forge = DataForge::open(bytes)?;
    let peer = forge.record(PEER).ok_or("competing Item Record")?.tree()?;
    let peer_key = item_variant::component_key(&peer).ok_or("comparable competing Item")?;
    let selected = item_key(&forge, SELECTED)?;
    assert_eq!(
        item_key(&forge, BASE)?,
        selected,
        "the readable base and selected Item have the same component key"
    );
    assert_eq!(
        peer_key == selected,
        cohort == ComponentCohort::Matching,
        "the two cases isolate the original component key comparison"
    );
    assert_target(
        component(&peer, WEAPON_COMPONENT).and_then(|weapon| weapon.field(PRIMARY_MAGAZINE_FIELD)),
        UNREADABLE_MAGAZINE,
    )?;
    let magazine = forge
        .record(UNREADABLE_MAGAZINE)
        .ok_or("competing Magazine Record")?
        .tree()?;
    assert_target(
        component(&magazine, AMMO_CONTAINER).and_then(|container| container.field(AMMO_FIELD)),
        UNREADABLE_AMMO,
    )?;
    let ammo = forge
        .record(UNREADABLE_AMMO)
        .ok_or("original Ammo Record")?;
    assert_eq!(
        ammo.type_name()?,
        AMMO,
        "the physical target is accepted by the stat selector"
    );
    assert!(
        ammo.tree().is_err(),
        "the original Ammo body cannot be read"
    );
    Ok(())
}

fn item_key<'source>(
    forge: &'source DataForge<'source>,
    index: u32,
) -> Result<ItemComponentKey<'source>, Box<dyn Error>> {
    let tree = forge.record(index).ok_or("fixture Item Record")?.tree()?;
    item_variant::component_key(&tree).ok_or_else(|| "comparable fixture Item components".into())
}

fn assert_target(value: Option<&Value<'_>>, index: u32) -> TestResult {
    let Some(Value::Reference(reference)) = value else {
        return Err("the original linked stat target decodes as a reference".into());
    };
    assert_eq!(
        reference.original_target().record_index(),
        index,
        "the authored physical link reaches the exact competing operand"
    );
    Ok(())
}
