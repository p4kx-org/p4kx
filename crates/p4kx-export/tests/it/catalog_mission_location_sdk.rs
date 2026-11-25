// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::sdk_full_proof as full_proof;
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogRecordSelection, DatasetAvailability,
    MissionLocationTemplateSnapshot, with_mission_location_templates,
};
use p4kx_export::{CatalogError, CatalogPublicationMode, build_catalog};
use p4kx_extraction::{Archive, CancelToken, DataForgeEntry, Language, SourceValueState};
use p4kx_testkit::{ArchiveBuilder, DataForgeBuilder, TestEntry, TestRecord, TestStruct};
use serde_json::{Value, json};

#[path = "mission_location_source_cells.rs"]
mod source_cells;

#[path = "catalog_mission_location_sdk_markers.rs"]
mod markers;

use super::TestResult;
use crate::commodity_v3_fixture::guid;
use crate::health_origin_assertions as proof;
use crate::spawns_fixture::{self, LOCATION_A, LOCATION_DANGLING_TAG, LOCATION_DEV, TAG_HIJACKED};

#[test]
fn selected_mission_location_templates_match_canonical_rows() -> TestResult {
    let folder = tempfile::tempdir()?;
    let archive = Archive::open_with_key(
        spawns_fixture::write_archive(folder.path())?,
        Some(p4kx_testkit::test_key()),
    )?;
    let mut options = crate::commodity_v3_fixture::options();
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    let built = build_catalog(&archive, options.with_full_provenance())?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let expected = proof::rows(&catalog, "/missionLocationTemplates")?;
    let entry = DataForgeEntry::read(&archive)?;
    let selected_guid = guid(LOCATION_A).parse()?;
    let physical = entry
        .open()?
        .records()
        .find(|record| record.guid() == selected_guid)
        .ok_or("location record")?
        .index();
    let session = full_proof::from_entry(
        &archive,
        &entry,
        CatalogLanguageSelection::AllAvailable,
        None,
    )?;
    let parsed = session.parsed(None)?;
    parsed.with_mission_location_templates(CatalogRecordSelection::All, None, |snapshot| {
        verify_canonical_rows(&snapshot, expected)
    })??;
    for selection in [
        CatalogRecordSelection::Guid(selected_guid),
        CatalogRecordSelection::RecordIndex(physical),
    ] {
        parsed.with_mission_location_templates(selection, None, |snapshot| {
            verify_selected_row(
                &snapshot,
                expected.first().ok_or("canonical row")?,
                physical,
            )
        })??;
    }
    Ok(())
}

fn verify_canonical_rows(
    snapshot: &MissionLocationTemplateSnapshot<'_>,
    expected: &[Value],
) -> TestResult {
    assert!(
        matches!(snapshot.availability, DatasetAvailability::Available),
        "the archive declares location templates"
    );
    assert_eq!(
        snapshot.coverage.skipped_leftovers, 1,
        "the dev template is skipped"
    );
    assert_eq!(
        serde_json::to_value(&snapshot.rows)?,
        json!(expected),
        "all SDK rows equal the canonical producer rows"
    );
    Ok(())
}

fn verify_selected_row(
    snapshot: &MissionLocationTemplateSnapshot<'_>,
    expected: &Value,
    physical: u32,
) -> TestResult {
    let [row] = snapshot.rows.as_slice() else {
        return Err("one location template".into());
    };
    assert_eq!(row.record_index(), physical, "original physical owner");
    assert_eq!(
        row.disabled().value(),
        Some(&false),
        "original disabled flag"
    );
    let tags = row.tags().value().ok_or("general tag list")?;
    let [first, null, repeated, dangling] = tags.as_slice() else {
        return Err("all four original tag slots".into());
    };
    let path = first.path()?;
    let earlier = path.value().ok_or("admitted original tag path")?;
    assert_eq!(earlier.to_string(), "Missions.Hijacked", "canonical path");
    assert_eq!(
        null.path()?.state(),
        SourceValueState::Null,
        "nil slot is retained"
    );
    assert_ne!(
        path.origin().source_uses_refs(),
        repeated.path()?.origin().source_uses_refs(),
        "repeated paths carry distinct physical operands"
    );
    let (missing_guid, _) = dangling.dangling_guid().ok_or("authored dangling target")?;
    assert_eq!(
        missing_guid.to_string(),
        guid(LOCATION_DANGLING_TAG),
        "exact dangling GUID"
    );
    assert_eq!(
        dangling.path()?.state(),
        SourceValueState::Absent,
        "a dangling operand has no readable path"
    );
    assert_eq!(
        &serde_json::to_value(row)?,
        expected,
        "physical SDK row matches Catalog"
    );
    assert_eq!(
        snapshot.available_languages,
        [Language::English, Language::French],
        "the session loads only present languages"
    );
    verify_name_origin(snapshot)
}

pub(super) fn verify_wire_tags(catalog: &Value, row: &Value) -> TestResult {
    for (field, expected) in [
        ("/tags/value/0/path/value", "Missions.Hijacked"),
        ("/tags/value/2/path/value", "Missions.Hijacked"),
        (
            "/producesTags/value/positiveTags/value/0/path/value",
            "ShipModel",
        ),
        (
            "/consumesTags/value/negativeTags/value/0/path/value",
            "Kill",
        ),
        ("/aiSpawnTags/value/0/path/value", "ShipModel"),
    ] {
        assert_eq!(proof::at(row, field)?, expected, "canonical tag at {field}");
    }
    for field in [
        "/producesTags/value/negativeTags/value",
        "/consumesTags/value/positiveTags/value",
    ] {
        assert_eq!(
            proof::at(row, field)?,
            &json!([]),
            "authored empty list at {field}"
        );
    }
    let first = proof::at(row, "/tags/value/0/path")?;
    let repeated = proof::at(row, "/tags/value/2/path")?;
    check_tag_uses(catalog, first, repeated)?;
    assert_eq!(
        proof::at(first, "/containsEncryptedSource")?,
        true,
        "encrypted operands"
    );
    assert_eq!(
        proof::at(first, "/sourceSetComplete")?,
        true,
        "established contributor set"
    );
    assert_eq!(
        proof::at(row, "/tags/value/1/path/state")?,
        "null",
        "stored null slot"
    );
    let dangling = proof::at(row, "/tags/value/3/danglingGuid/value")?;
    assert_eq!(
        dangling,
        &json!(guid(LOCATION_DANGLING_TAG)),
        "retains missing source GUID"
    );
    proof::closure(catalog)
}

fn check_tag_uses(catalog: &Value, first: &Value, repeated: &Value) -> TestResult {
    let guid = json!(guid(TAG_HIJACKED));
    let tag = proof::rows(catalog, "/projections/tagPaths/rows")?
        .iter()
        .find(|row| row.pointer("/guid/value") == Some(&guid))
        .ok_or("selected source Tag definition")?;
    let lineage = proof::uses(catalog, proof::at(tag, "/path")?)?;
    let operands = [
        proof::uses(catalog, first)?,
        proof::uses(catalog, repeated)?,
    ];
    let caller = |uses: &[&Value]| {
        let local: Vec<_> = uses.iter().filter(|item| !lineage.contains(item)).collect();
        assert_eq!(
            local.len(),
            1,
            "each repeated slot adds one original operand"
        );
        assert!(
            lineage.iter().all(|input| uses.contains(input)),
            "the chosen path's exact lineage"
        );
        local.first().and_then(|item| item.get("id")).cloned()
    };
    let first = caller(&operands[0]).ok_or("first caller")?;
    let repeated = caller(&operands[1]).ok_or("repeated caller")?;
    assert_ne!(
        first, repeated,
        "original references differ even for the same rendered path"
    );
    Ok(())
}

fn verify_name_origin(snapshot: &MissionLocationTemplateSnapshot<'_>) -> TestResult {
    let row = snapshot.rows.first().ok_or("location template row")?;
    let names = row.names().value().ok_or("original name collection")?;
    let [first, second, placeholder] = names.as_slice() else {
        return Err("three authored name variants".into());
    };
    let first = first.value().ok_or("first original name slot")?;
    let second = second.value().ok_or("second original name slot")?;
    let placeholder = placeholder.value().ok_or("third original name slot")?;
    let id = first
        .display_name
        .value()
        .ok_or("protected name reference")?;
    assert_eq!(
        second.display_name.value(),
        Some(id),
        "repeated name shares the text definition"
    );
    assert!(
        placeholder.display_name.value().is_none(),
        "placeholder stays unprojected"
    );
    assert_ne!(
        first.display_name.origin().source_uses_refs(),
        second.display_name.origin().source_uses_refs(),
        "repeated name has a distinct authored occurrence"
    );
    let name = snapshot.game_text(id).ok_or("protected text row")?;
    assert_eq!(
        name.key().value().copied(),
        Some("@place_name"),
        "original text key"
    );
    let tag = first.tag.path()?;
    for origin in [
        first.display_name.origin(),
        second.display_name.origin(),
        name.key().origin(),
        tag.origin(),
    ] {
        assert_source_uses(snapshot, origin.source_uses_refs());
    }
    assert_eq!(
        first.display_name.origin().contains_encrypted_source(),
        Some(true),
        "source encryption"
    );
    let translations = name.translations().value().ok_or("translations")?;
    assert!(
        translations.contains_key("en"),
        "English translation present"
    );
    assert!(
        translations.contains_key("fr"),
        "French translation present"
    );
    Ok(())
}

#[test]
fn empty_mission_location_template_queries_skip_tags_and_locked_locales() -> TestResult {
    let folder = tempfile::tempdir()?;
    let archive = locked_locale_archive(folder.path())?;
    for (selection, skipped) in [
        (CatalogRecordSelection::All, 1),
        (CatalogRecordSelection::RecordIndex(0), 0),
        (CatalogRecordSelection::RecordIndex(1), 1),
        (CatalogRecordSelection::RecordIndex(u32::MAX), 0),
    ] {
        with_mission_location_templates(
            &archive,
            selection,
            CatalogLanguageSelection::Selected(&[Language::English]),
            None,
            |snapshot| {
                assert!(
                    matches!(snapshot.availability, DatasetAvailability::Available),
                    "the source declares the type"
                );
                assert_eq!(
                    snapshot.coverage.skipped_leftovers, skipped,
                    "only in-scope leftovers"
                );
                assert_eq!(
                    snapshot.requested_languages,
                    [Language::English],
                    "caller language policy preserved"
                );
                assert!(
                    snapshot.available_languages.is_empty(),
                    "nothing needs opened locale data"
                );
                assert!(snapshot.rows.is_empty(), "nothing is admitted");
                assert!(snapshot.game_texts.is_empty(), "no selected text");
                assert!(snapshot.tag_issues.is_none(), "no Tag graph built");
            },
        )?;
    }
    assert_eq!(
        archive.key_demand().snapshot().unread(),
        0,
        "the locked locale remains unread"
    );
    let cancel = CancelToken::new();
    cancel.cancel();
    assert!(
        matches!(
            with_mission_location_templates(
                &archive,
                CatalogRecordSelection::All,
                CatalogLanguageSelection::Selected(&[]),
                Some(&cancel),
                |_| panic!("cancelled query must not call the reader"),
            ),
            Err(CatalogError::Cancelled)
        ),
        "a cancelled query yields no snapshot"
    );
    Ok(())
}

fn locked_locale_archive(folder: &std::path::Path) -> Result<Archive, Box<dyn std::error::Error>> {
    let path = folder.join("fixture.p4k");
    let owner = "MissionLocationTemplate";
    let unrelated = "Unrelated";
    let path_prefix = "libs/foundry/records/missiondata/pu_locations/";
    let source = DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new(owner))
        .structure(TestStruct::new(unrelated))
        .record(TestRecord::new(
            LOCATION_A,
            unrelated,
            "Unrelated.Other",
            format!("{path_prefix}other.xml"),
        ))
        .record(TestRecord::new(
            LOCATION_DEV,
            owner,
            "MissionLocationTemplate.dev_temp",
            format!("{path_prefix}dev_temp.xml"),
        ))
        .build();
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", source))
        .entry(
            TestEntry::stored(Language::English.global_ini_path(), b"locked=fixture\n").encrypted(),
        )
        .write_to(&path)?;
    Ok(Archive::open(path)?)
}

fn assert_source_uses(
    snapshot: &MissionLocationTemplateSnapshot<'_>,
    references: &[p4kx_extraction::SourceUseRef],
) {
    assert!(
        !references.is_empty(),
        "the operand and chosen Tag contribute"
    );
    for reference in references {
        assert!(
            snapshot
                .source_inventory
                .iter_uses()
                .any(|used| used.id() == reference),
            "known evidence stays reachable for every encryption state"
        );
    }
}
