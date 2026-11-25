// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::sdk_full_proof as full_proof;
use p4kx_export::catalog_queries::starmap::{
    StarmapDefinitionFamily, StarmapDefinitionRows, StarmapDefinitionSelection,
    StarmapDefinitionSnapshot, StarmapObjectDefinition,
};
use p4kx_export::catalog_queries::{CatalogLanguageSelection, CatalogRecordSelection};
use p4kx_extraction::{DataForgeEntry, Language, SourceLocation};
use p4kx_testkit::{DataForgeBuilder, TestEntry, TestRecord, TestStruct, TestType, TestValue};

use super::{LANGUAGES, TestResult, open_fixture, verify_encrypted_header};

const OBJECT_TYPE: &str = "StarMapObject";
const PU: &str = "libs/foundry/records/starmap/pu/fixture";
const OTHER: &str = "libs/foundry/records/starmap/arena/fixture";

pub(super) fn extend(database: DataForgeBuilder) -> DataForgeBuilder {
    database
        .structure(
            TestStruct::new(OBJECT_TYPE)
                .field("name", TestType::Locale)
                .field("hideInStarmap", TestType::Bool),
        )
        .record(object(0x1067_0811, PU, "@shared"))
        .record(object(0x1067_0812, OTHER, "@outside_pu"))
}

fn object(guid: u128, path: &str, name: &str) -> TestRecord {
    TestRecord::new(
        guid,
        OBJECT_TYPE,
        format!("{OBJECT_TYPE}.Fixture{guid:x}"),
        format!("{path}.xml"),
    )
    .value("name", TestValue::text(name))
    .value("hideInStarmap", TestValue::Bool(true))
}

pub(super) fn verify_pu_objects(
    snapshot: &StarmapDefinitionSnapshot<'_>,
    rows: &[StarmapObjectDefinition<'_>],
) -> TestResult {
    let [row] = rows else {
        return Err("one starmap object under the PU path".into());
    };
    verify_encrypted_header(&row.source);
    assert_eq!(
        row.source.record_path().value(),
        Some(&PU),
        "only the original PU object is returned"
    );
    assert_eq!(
        row.visibility.hide_in_starmap.value(),
        Some(&true),
        "the typed authored visibility flag remains accessible"
    );
    assert!(
        row.visibility
            .hide_in_starmap
            .origin()
            .contains_encrypted_source()
            == Some(true),
        "the original visibility field belongs to the encrypted payload"
    );
    assert!(
        row.text.name.value().is_some()
            && snapshot
                .game_texts
                .iter()
                .any(|text| text.key().value() == Some(&"@shared")),
        "selected object name has its protected translation identity"
    );
    let index = row
        .source
        .record_index()
        .value()
        .copied()
        .ok_or("original PU Record index")?;
    let mut has_owner_evidence = false;
    for used in snapshot.source_inventory.iter_uses() {
        if let SourceLocation::Subresource {
            namespace, indices, ..
        } = used.location()
            && namespace == "dataforgeValue"
        {
            assert_eq!(
                indices.first(),
                Some(&u64::from(index)),
                "every original body occurrence belongs to the admitted PU object"
            );
            has_owner_evidence = true;
        }
    }
    assert!(
        has_owner_evidence,
        "the query retains original payload evidence"
    );
    Ok(())
}

#[test]
fn starmap_object_scope_rejects_the_exact_out_of_pu_index() -> TestResult {
    let german = b"shared=Unused\n";
    let (_folder, archive) = open_fixture(
        TestEntry::stored(Language::German.global_ini_path(), german)
            .with_uncompressed_size(u64::try_from(german.len())? + 1),
    )?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let index = forge
        .records()
        .find(|row| row.record_path().is_ok_and(|path| path == OTHER))
        .ok_or("physical out-of-PU object")?
        .index();
    let selection = StarmapDefinitionSelection::new(
        StarmapDefinitionFamily::Objects,
        CatalogRecordSelection::RecordIndex(index),
    );
    full_proof::open(&archive, CatalogLanguageSelection::AllAvailable, None)?
        .with_starmap_definitions(selection, None, |snapshot| -> TestResult {
            let StarmapDefinitionRows::Objects(rows) = snapshot.rows else {
                return Err("typed starmap Objects selection".into());
            };
            assert!(
                rows.is_empty(),
                "exact physical index does not bypass the PU path"
            );
            assert!(
                snapshot.available_languages.is_empty() && snapshot.game_texts.is_empty(),
                "the excluded Record does not open the unreadable locale"
            );
            Ok(())
        })??;
    let selection = StarmapDefinitionSelection::new(
        StarmapDefinitionFamily::Objects,
        CatalogRecordSelection::All,
    );
    full_proof::open(
        &archive,
        CatalogLanguageSelection::Selected(LANGUAGES),
        None,
    )?
    .with_starmap_definitions(selection, None, |snapshot| {
        let StarmapDefinitionRows::Objects(rows) = &snapshot.rows else {
            return Err("typed starmap Objects selection".into());
        };
        assert_eq!(
            snapshot.available_languages, LANGUAGES,
            "the selected object reads only the requested text locale"
        );
        verify_pu_objects(&snapshot, rows)
    })?
}
