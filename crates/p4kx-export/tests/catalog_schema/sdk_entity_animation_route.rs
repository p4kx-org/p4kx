// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::sdk_full_proof as full_proof;
use std::time::UNIX_EPOCH;

use p4kx_export::catalog_queries::equipment::entity_animation::EntityAnimationControllerSnapshot;
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, DatasetAvailability,
};
use p4kx_export::{CatalogOptions, build_catalog_from};
use p4kx_extraction::{Archive, DataForgeEntry, Filter, InputScope, SourceLocation};
use p4kx_testkit::{ArchiveBuilder, DataForgeBuilder, TestEntry, TestRecord};
use serde_json::Value;

use super::component_parity::parity;
use super::{Route, TestResult};

use super::weapon_operating_fixture as fixture;

pub(in super::super) const DATASET: &str = "/configurations/entityAnimationControllers";

const OWNER: u32 = 2;
const COMPONENT_SLOT: u32 = 2;
const UNRELATED: u32 = 3;
const OWNER_SELECTION: CatalogRecordSelection = CatalogRecordSelection::RecordIndex(OWNER);

pub(super) fn read() -> TestResult<Route> {
    let (_directory, archive) = open_fixture(&fixture::database())?;
    let entry = DataForgeEntry::read(&archive)?;
    let filter = Filter::new("*/entities/scitem/*");
    let options = CatalogOptions {
        filter: Some(&filter),
        ..CatalogOptions::new(UNIX_EPOCH)
    };
    let catalog: Value = serde_json::from_slice(
        build_catalog_from(&archive, &entry, options.with_full_provenance())?.json(),
    )?;
    full_proof::from_entry(&archive, &entry, full_proof::NO_LANGUAGES, None)?
        .with_entity_animation_controllers(OWNER_SELECTION, None, |snapshot| {
            verify_fields(&snapshot)?;
            parity(DATASET, &snapshot, &catalog)?;
            Route::queried_rows(DATASET, snapshot.rows(), None)
        })?
}

fn verify_fields(snapshot: &EntityAnimationControllerSnapshot<'_>) -> TestResult<()> {
    let [row] = snapshot.rows() else {
        return Err("one physical animation controller".into());
    };
    assert_eq!(
        row.owner().record_index().value(),
        Some(&OWNER),
        "physical controller owner"
    );
    assert_eq!(
        row.owner().component_ordinal().value(),
        Some(&COMPONENT_SLOT),
        "original component slot including preceding components"
    );
    for (field, expected) in [
        (
            row.animation_controller(),
            "Animations/fixture-controller.xml",
        ),
        (row.animation_database(), "Animations/fixture.adb"),
    ] {
        assert_eq!(field.value().copied(), Some(expected), "original locator");
        let origin = field.origin();
        assert_eq!(
            origin.contains_encrypted_source(),
            Some(true),
            "the actual locator comes from the encrypted DataForge Entry"
        );
        assert!(
            origin.source_set_complete() && snapshot.source_inventory().iter_uses().any(|used| {
                used.scope() == InputScope::Payload
                    && origin.source_uses_refs().contains(used.id())
                    && matches!(used.location(), SourceLocation::Subresource { namespace, indices, field }
                        if namespace == "dataforgeValue"
                        && indices.first() == Some(&u64::from(OWNER))
                        && field == "$value")
            }),
            "each locator retains its original physical owner field use"
        );
    }
    Ok(())
}

#[test]
fn selected_animation_controller_skips_an_unrelated_unreadable_body() -> TestResult<()> {
    let database = fixture::database().record(
        TestRecord::new(
            0x0011_6601,
            p4kx_assembly::ENTITY_TYPE,
            "EntityClassDefinition.FixtureUnrelatedControllerOwner",
            "libs/foundry/records/fixture/unrelated_controller_owner.xml",
        )
        .with_instance_index(u16::MAX),
    );
    let (_directory, archive) = open_fixture(&database)?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    assert!(
        forge
            .record(UNRELATED)
            .ok_or("unreadable physical decoy")?
            .tree()
            .is_err(),
        "the unrelated owner really has an unreadable body"
    );
    let session = CatalogQuerySession::from_entry(
        &archive,
        &entry,
        CatalogLanguageSelection::Selected(&[]),
        None,
    )?
    .with_provenance_mode(full_proof::MODE);
    let parsed = session.parsed(None)?;
    parsed.with_entity_animation_controllers(OWNER_SELECTION, None, |snapshot| {
        assert!(
            snapshot.coverage().unreadable_bodies.is_empty(),
            "the foreign unreadable body is outside the selected scope"
        );
        verify_fields(&snapshot)
    })??;
    parsed.with_entity_animation_controllers(
        CatalogRecordSelection::RecordIndex(0),
        None,
        |snapshot| {
            assert!(
                snapshot.rows().is_empty(),
                "the procedural definition is a different family"
            );
            assert!(
                matches!(snapshot.availability(), DatasetAvailability::Available),
                "the controller type is still declared in DataForge"
            );
            assert!(
                snapshot.source_inventory().iter_uses().all(|used| {
                    used.scope() == InputScope::ArchiveDirectoryMetadata
                        && matches!(used.location(), SourceLocation::Metadata { .. })
                }),
                "the unrelated selection retains the DataForge Entry directory metadata"
            );
        },
    )?;
    Ok(())
}

fn open_fixture(database: &DataForgeBuilder) -> TestResult<(tempfile::TempDir, Archive)> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("entity-animation.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", database.build()).encrypted())
        .entry(TestEntry::stored(
            "Data/Libs/CosmoTools/LayerRecovery/LayerBackups.xml",
            b"<LayerBackups/>",
        ))
        .write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?;
    Ok((folder, archive))
}
