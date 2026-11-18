// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::Cell;

use p4kx_extraction::{Archive, DataForge, Language, SourceInventory, SourceValueState};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestInstance, TestRecord, TestStruct, TestType,
    TestValue,
};

use super::{Item, item};
use crate::build::original_input::test_support;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_v3::catalog_item_components;
use crate::item_class::ItemClass;
use crate::record_observation::RecordObservation;
use crate::texts::TextSources;
use crate::{CatalogError, ExportAccountingError, Kind};

type TestResult = Result<(), Box<dyn std::error::Error>>;
const ENTITY: &str = "EntityClassDefinition";
const COMPONENT: &str = "DataForgeComponentParams";
const ATTACHMENT: &str = "SAttachableComponentParams";
const DEFINITION: &str = "SItemDefinition";
const LOCALIZATION: &str = "FixtureLocalization";

#[test]
fn english_classification_uses_semantic_input_for_empty_german_and_english_output_requests()
-> TestResult {
    let (_folder, archive) = fixture(true)?;
    let none = [];
    let german = [Language::German];
    let both = [Language::English, Language::German];
    for requested in [none.as_slice(), german.as_slice(), both.as_slice()] {
        let sources = TextSources::read_item_languages(&archive, requested)?;
        assert!(
            sources.consulted(Language::English),
            "English was genuinely consulted as a semantic dependency"
        );
        read_row(&archive, &sources, |row, inventory| {
            assert_eq!(
                row.class.value(),
                Some(&ItemClass::Military),
                "requested output language never changes the English rule"
            );
            assert_eq!(
                row.class.origin().contains_encrypted_source(),
                Some(true),
                "the actual encrypted English contributor is immediate"
            );
            assert!(
                row.class.origin().source_set_complete(),
                "the chosen Description and lookup are complete"
            );
            assert_eq!(
                row.header.guid.origin().contains_encrypted_source(),
                Some(false),
                "the plain owner header keeps its independent origin"
            );
            closure(row, inventory);
        })?;
    }
    Ok(())
}

#[test]
fn an_unconsulted_english_source_cannot_be_declared_absent() -> TestResult {
    let (_folder, archive) = fixture(true)?;
    let sources = TextSources::read_languages(&archive, &[Language::German])?;
    let inspected = Cell::new(false);
    let result = read_row(&archive, &sources, |_, _| inspected.set(true));
    assert!(
        !inspected.get(),
        "missing semantic admission cannot expose a classified Item"
    );
    assert!(
        matches!(
            result,
            Err(CatalogError::Accounting(
                ExportAccountingError::MissingFieldInspection
            ))
        ),
        "the original semantic read is mandatory before negative classification"
    );
    Ok(())
}

#[test]
fn genuinely_absent_english_retains_the_original_directory_negative_proof() -> TestResult {
    let (_folder, archive) = fixture(false)?;
    let sources = TextSources::read_item_languages(&archive, &[Language::German])?;
    assert!(
        sources.consulted(Language::English),
        "actual absence followed a real English admission"
    );
    read_row(&archive, &sources, |row, inventory| {
        assert_eq!(
            row.class.state(),
            SourceValueState::Absent,
            "the German class never substitutes for absent English"
        );
        assert_eq!(
            row.class.origin().contains_encrypted_source(),
            Some(false),
            "only the actual plain source and intact directory contribute"
        );
        assert!(
            row.class.origin().source_set_complete(),
            "the negative lookup is proved from the original directory"
        );
        assert!(
            !row.class.origin().source_uses_refs().is_empty(),
            "absence is not a sourceless default"
        );
        closure(row, inventory);
    })?;
    Ok(())
}

fn read_row(
    archive: &Archive,
    sources: &TextSources,
    inspect: impl for<'source> FnOnce(&Item<'source>, &SourceInventory),
) -> Result<(), CatalogError> {
    with_context(archive, sources, |forge, projection| {
        let record = forge
            .record(0)
            .ok_or(ExportAccountingError::MissingFieldInspection)?;
        let tree = record.tree().map_err(CatalogError::DataForge)?;
        let admission = RecordObservation::capture(projection.original(), &record)?;
        let (_, _, (row, _)) = catalog_item_components::read(
            &tree,
            projection,
            &admission,
            None,
            |definition, originals, _| item(&tree, definition, originals, projection),
        )?;
        let inventory = projection.origins().uses.freeze()?;
        inspect(&row, &inventory);
        Ok(())
    })
}

fn with_context(
    archive: &Archive,
    sources: &TextSources,
    inspect: impl for<'source, 'context> FnOnce(
        &'source DataForge<'source>,
        &CatalogProjection<'context, 'source>,
    ) -> Result<(), CatalogError>,
) -> Result<(), CatalogError> {
    test_support::with_archive_projection(archive, sources, |forge, projection| {
        inspect(forge, &projection.for_kind(Kind::Items))
    })
}

fn closure(row: &Item<'_>, inventory: &SourceInventory) {
    for reference in row.class.origin().source_uses_refs() {
        assert!(
            inventory
                .iter_uses()
                .any(|observed| observed.id() == reference),
            "every immediate class contribution resolves in this source inventory"
        );
    }
    for observed in inventory.iter_uses() {
        assert!(
            inventory
                .iter_sources()
                .any(|source| source.id() == observed.source()),
            "every retained use has its actual original source"
        );
    }
}

fn fixture(english: bool) -> Result<(tempfile::TempDir, Archive), Box<dyn std::error::Error>> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("item-semantic-language.p4k");
    let mut builder = ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", database().build()))
        .entry(
            TestEntry::stored(
                Language::German.global_ini_path(),
                b"description=Class: Stealth\nname=Fixture Name\n",
            )
            .encrypted(),
        );
    if english {
        builder = builder.entry(
            TestEntry::stored(
                Language::English.global_ini_path(),
                b"description=Class: Military\nname=Fixture name\n",
            )
            .encrypted(),
        );
    }
    builder.write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?;
    Ok((folder, archive))
}

fn database() -> DataForgeBuilder {
    let structs = [
        TestStruct::new(ENTITY).array("Components", TestType::StrongPointer(COMPONENT.to_owned())),
        TestStruct::new(COMPONENT),
        TestStruct::new(ATTACHMENT)
            .parent(COMPONENT)
            .field("AttachDef", TestType::Class(DEFINITION.to_owned())),
        TestStruct::new(DEFINITION).field("Localization", TestType::Class(LOCALIZATION.to_owned())),
        TestStruct::new(LOCALIZATION)
            .field("Name", TestType::Locale)
            .field("Description", TestType::Locale),
    ];
    structs
        .into_iter()
        .fold(
            DataForgeBuilder::new().version(8),
            DataForgeBuilder::structure,
        )
        .record(record("SemanticA"))
        .record(record("SemanticB"))
}

fn record(name: &str) -> TestRecord {
    let localization = TestValue::Struct(
        TestInstance::new(LOCALIZATION)
            .value("Name", TestValue::text("@name"))
            .value("Description", TestValue::text("@description")),
    );
    let definition =
        TestValue::Struct(TestInstance::new(DEFINITION).value("Localization", localization));
    let attachment =
        TestValue::Struct(TestInstance::new(ATTACHMENT).value("AttachDef", definition));
    TestRecord::new(
        0x9879,
        ENTITY,
        format!("{ENTITY}.{name}"),
        format!("libs/foundry/records/entities/scitem/fixture/{name}.xml"),
    )
    .value("Components", TestValue::Array(vec![attachment]))
}

#[test]
fn an_actual_same_guid_definition_of_another_physical_record_is_rejected() -> TestResult {
    let (_folder, archive) = fixture(true)?;
    let sources = TextSources::read_item_languages(&archive, &[])?;
    with_context(&archive, &sources, |forge, projection| {
        let foreign = forge
            .record(1)
            .ok_or(ExportAccountingError::MissingFieldInspection)?;
        let foreign_tree = foreign.tree().map_err(CatalogError::DataForge)?;
        let foreign_admission = RecordObservation::capture(projection.original(), &foreign)?;
        let (definition, _, ()) = catalog_item_components::read(
            &foreign_tree,
            projection,
            &foreign_admission,
            None,
            |_, _, _| Ok(()),
        )?;
        let target = forge
            .record(0)
            .ok_or(ExportAccountingError::MissingFieldInspection)?;
        assert_eq!(
            target.guid(),
            foreign.guid(),
            "the actual aliases share their nominal GUID"
        );
        assert_ne!(
            target.index(),
            foreign.index(),
            "their physical original indices differ"
        );
        let tree = target.tree().map_err(CatalogError::DataForge)?;
        let admission = RecordObservation::capture(projection.original(), &target)?;
        let result = catalog_item_components::read(
            &tree,
            projection,
            &admission,
            None,
            |_, originals, _| item(&tree, &definition, originals, projection),
        );
        assert!(
            matches!(
                result,
                Err(CatalogError::Accounting(
                    ExportAccountingError::ConflictingSourceDatum
                ))
            ),
            "a source-owned definition cannot be attached to a different physical owner"
        );
        Ok(())
    })?;
    Ok(())
}
