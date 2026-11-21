// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::ExportDiagnosticCode;
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogRecordSelection, ContractGeneratorSnapshot, SourcePayload,
    SourceText, with_contract_generators, with_contract_generators_from,
};
use p4kx_extraction::{Archive, DataForgeEntry, Language, SourceLocation, SourceValueState};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestInstance, TestRecord, TestStruct, TestType,
    TestValue,
};

use super::TestResult;
use crate::commodity_v3_fixture::guid;

const OWNER: u128 = 0x1100_0000_0000_4000_8000_0000_0000_00a0;
const OTHER: u128 = 0x1100_0000_0000_4000_8000_0000_0000_00a1;
const DEV: u128 = 0x1100_0000_0000_4000_8000_0000_0000_00a2;
const PREFIX: &str = "libs/foundry/records/contracts/contractgenerator/";

#[test]
fn generator_selection_keeps_physical_duplicates_without_opening_other_payloads() -> TestResult {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("fixture.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(
            "Data/Game2.dcb",
            scope_database().build(),
        ))
        .entry(
            TestEntry::stored(Language::English.global_ini_path(), b"locked=locale\n").encrypted(),
        )
        .entry(TestEntry::stored("Data/ObjectContainers/broken.soc", b"locked world").encrypted())
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    let entry = DataForgeEntry::read(&archive)?;
    with_contract_generators_from(
        &archive,
        &entry,
        CatalogRecordSelection::Guid(guid(OWNER).parse()?),
        CatalogLanguageSelection::AllAvailable,
        None,
        |snapshot| verify_scope(&snapshot),
    )??;
    with_contract_generators(
        &archive,
        CatalogRecordSelection::RecordIndex(5),
        CatalogLanguageSelection::AllAvailable,
        None,
        |snapshot| {
            assert_eq!(
                snapshot.rows.len(),
                1,
                "raw development owners remain present"
            );
            assert_eq!(
                snapshot.coverage.skipped_leftovers, 0,
                "source-definition selection preserves raw development declarations"
            );
        },
    )?;
    assert_eq!(
        archive.key_demand().snapshot().unread(),
        0,
        "localization and world payloads stay outside the raw source query"
    );
    Ok(())
}

fn verify_scope(snapshot: &ContractGeneratorSnapshot<'_>) -> TestResult {
    assert_eq!(
        snapshot.rows.len(),
        2,
        "both physical declarations of the GUID survive"
    );
    assert!(
        snapshot.coverage.incomplete_fields.is_empty(),
        "an unselected malformed Generator creates no projection gaps"
    );
    assert!(
        snapshot.projection_failures.is_empty(),
        "unselected projectors remain inactive"
    );
    for (index, (row, expected)) in snapshot.rows.iter().zip(["first", "second"]).enumerate() {
        assert_eq!(
            row.header().record_index().value(),
            Some(&u32::try_from(index)?),
            "equal GUIDs retain physical source order"
        );
        let field = row
            .source()
            .roots()
            .first()
            .ok_or("the original marker field")?;
        let node = row
            .source()
            .value(field.value())
            .ok_or("the marker occurrence")?;
        assert!(
            matches!(node.value().value(),
            Some(SourcePayload::String { value: SourceText::GameText { source_text } })
                if *source_text == expected),
            "the API exposes the original protected source text directly"
        );
    }
    for used in snapshot.source_inventory.iter_uses() {
        match used.location() {
            SourceLocation::DataForge { record_index, .. } => {
                assert!(*record_index < 2, "no unrelated Record header is projected");
            }
            SourceLocation::Subresource {
                namespace, indices, ..
            } if namespace == "dataforgeValue" => assert!(
                indices.first().is_some_and(|index| *index < 2),
                "no unrelated Generator or sibling body is traversed"
            ),
            _ => {}
        }
    }
    Ok(())
}

fn scope_database() -> DataForgeBuilder {
    let source = DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("ContractGenerator").field("marker", TestType::String))
        .structure(TestStruct::new("MissionLocationTemplate").field("broken", TestType::Bool));
    let record = |guid, name: &str, path: String, marker| {
        TestRecord::new(
            guid,
            "ContractGenerator",
            format!("ContractGenerator.{name}"),
            path,
        )
        .value("marker", marker)
    };
    source
        .record(record(
            OWNER,
            "first",
            format!("{PREFIX}first.xml"),
            TestValue::text("first"),
        ))
        .record(record(
            OWNER,
            "second",
            format!("{PREFIX}second.xml"),
            TestValue::text("second"),
        ))
        .record(record(
            OTHER,
            "broken",
            format!("{PREFIX}broken.xml"),
            TestValue::Raw(u32::MAX.to_le_bytes().to_vec()),
        ))
        .record(record(
            OTHER,
            "elsewhere",
            "libs/foundry/records/elsewhere/owner.xml".into(),
            TestValue::text("outside"),
        ))
        .record(
            TestRecord::new(
                OTHER,
                "MissionLocationTemplate",
                "MissionLocationTemplate.Broken",
                "libs/foundry/records/missiondata/pu_locations/broken.xml",
            )
            .value("broken", TestValue::Raw(vec![2])),
        )
        .record(record(
            DEV,
            "dev_fixture",
            format!("{PREFIX}dev_fixture.xml"),
            TestValue::text("development source"),
        ))
}

#[test]
fn generator_projection_notes_keep_the_complete_original_source() -> TestResult {
    let (_folder, archive) = crate::commodity_v3_fixture::open(
        &interpretation_failure_database(),
        crate::commodity_v3_fixture::DcbOrigin::Plain,
    )?;
    with_contract_generators(
        &archive,
        CatalogRecordSelection::All,
        CatalogLanguageSelection::Selected(&[]),
        None,
        |snapshot| verify_projection_notes(&snapshot),
    )??;
    Ok(())
}

fn verify_projection_notes(snapshot: &ContractGeneratorSnapshot<'_>) -> TestResult {
    let [row] = snapshot.rows.as_slice() else {
        return Err("one original source".into());
    };
    assert!(
        !row.source().values().is_empty(),
        "the complete source arena survives"
    );
    assert_eq!(
        row.completion_tag_results().len(),
        1,
        "the typed result stays visible"
    );
    let result = row
        .completion_tag_results()
        .first()
        .ok_or("one typed result")?;
    assert_eq!(
        result.split_points_for_party().state(),
        SourceValueState::Unsupported,
        "the typed accessor preserves the unsupported authored declaration"
    );
    assert!(
        row.source().value(result.source()).is_some(),
        "the typed result links to its original occurrence"
    );
    assert_eq!(
        snapshot.coverage.incomplete_fields.len(),
        1,
        "the owner is incomplete"
    );
    let [note] = snapshot.projection_notes.as_slice() else {
        return Err("one typed note".into());
    };
    assert!(
        note.blocks_projection(),
        "the source declaration is unsupported"
    );
    assert_eq!(
        note.code(),
        ExportDiagnosticCode::UnsupportedValue,
        "the canonical diagnostic classification is retained"
    );
    assert!(
        note.field().ends_with("splitPointsForParty"),
        "the exact field is identified"
    );
    verify_projection_failure(snapshot)?;

    Ok(())
}

fn verify_projection_failure(snapshot: &ContractGeneratorSnapshot<'_>) -> TestResult {
    assert_eq!(
        snapshot.projection_failures.len(),
        1,
        "the failed item interpretation is explicit"
    );
    assert!(
        snapshot
            .projection_failures
            .first()
            .ok_or("one typed interpretation failure")?
            .error()
            .to_string()
            .contains("contractParams"),
        "the original interpretation failure stays inspectable"
    );
    Ok(())
}

fn interpretation_failure_database() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new("ContractGenerator")
                .array("generators", TestType::StrongPointer("Handler".to_owned()))
                .field(
                    "contractResults",
                    TestType::StrongPointer("ContractResult_CompletionTags".to_owned()),
                ),
        )
        .structure(TestStruct::new("Handler").field("contractParams", TestType::UInt32))
        .structure(
            TestStruct::new("ContractResult_CompletionTags")
                .field("splitPointsForParty", TestType::String),
        )
        .record(
            TestRecord::new(
                OWNER,
                "ContractGenerator",
                "ContractGenerator.Typed",
                format!("{PREFIX}typed.xml"),
            )
            .value(
                "generators",
                TestValue::Array(vec![TestValue::Struct(
                    TestInstance::new("Handler").value("contractParams", TestValue::UInt32(3)),
                )]),
            )
            .value(
                "contractResults",
                TestValue::Struct(
                    TestInstance::new("ContractResult_CompletionTags")
                        .value("splitPointsForParty", TestValue::text("unsupported")),
                ),
            ),
        )
}
