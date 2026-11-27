// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, Record, RecordTree, SourceValueState, SourcedValue};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestInstance, TestRecord, TestStruct, TestType,
    TestValue,
};

use super::definition_read;
use super::item_definition::PURCHASABLE;
use super::read_item_purchase_rank;
use crate::build::original_input::test_support;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_read::original::OriginalFlowFields;

use crate::playability_rules::PurchaseRank;
use crate::record_observation::RecordObservation;

use crate::texts::TextSources;
use crate::{CatalogError, ExportAccountingError, Kind, Playability};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const ENTITY: &str = "EntityClassDefinition";
const COMPONENT: &str = "DataForgeComponentParams";
const PLAYER: &str = "@fixture_shop";
const PLACEHOLDER: &str = "@LOC_PLACEHOLDER";

#[test]
fn purchase_rank_shares_exact_item_evidence_without_a_translation_lookup() -> TestResult {
    let (_folder, archive) = archive(&locale_fixture())?;
    let no_languages = TextSources::read_languages(&archive, &[])?;
    test_support::with_archive_projection(&archive, &no_languages, |forge, projection| {
        let projection = projection.for_kind(Kind::Items);
        let expected = [
            (Some(PurchaseRank::Player), true),
            (Some(PurchaseRank::NonPlayer), true),
            (Some(PurchaseRank::Player), false),
            (Some(PurchaseRank::NonPlayer), true),
            (None, false),
        ];
        let mut ranks = Vec::new();
        for (record, expectation) in forge.records().zip(expected) {
            let rank = inspect_rank(&projection, &record, expectation)?;
            ranks.push((record, rank));
        }
        let texts = projection.finish_rows()?;
        assert!(
            texts.public_names.is_empty(),
            "the original rank reads no public display texts"
        );
        assert!(
            texts.game_texts.is_empty(),
            "the original rank reads no protected texts"
        );
        for (record, rank) in ranks {
            check_full_playability(&projection, &record, &rank)?;
        }
        Ok::<_, CatalogError>(())
    })?;
    Ok(())
}

fn inspect_rank<'source>(
    projection: &CatalogProjection<'_, 'source>,
    record: &Record<'source>,
    expected: (Option<PurchaseRank>, bool),
) -> Result<SourcedValue<PurchaseRank>, CatalogError> {
    let tree = record.tree().map_err(CatalogError::DataForge)?;
    let local = read_rank(projection, record, &tree)?;
    assert_eq!(
        local.value().copied(),
        expected.0,
        "the original first slot controls rank"
    );
    assert_eq!(
        local.origin().source_set_complete(),
        expected.1,
        "only a proven prefix and usable input can certify the observed rank"
    );
    assert_eq!(
        local.origin().contains_encrypted_source(),
        Some(true),
        "the rank retains the exact encrypted original prefix or key"
    );
    if local.value().is_none() {
        assert_eq!(
            local.state(),
            SourceValueState::Unreadable,
            "the original Locale could not be read"
        );
    }
    Ok(local)
}

fn read_rank<'source>(
    projection: &CatalogProjection<'_, 'source>,
    record: &Record<'source>,
    tree: &RecordTree<'source>,
) -> Result<SourcedValue<PurchaseRank>, CatalogError> {
    let input = projection.original();
    let scope = input.record_scope(record, tree)?;
    let originals = OriginalFlowFields::read(&scope, None)?;
    read_item_purchase_rank(input, tree, &originals)
}

fn check_full_playability<'source>(
    projection: &CatalogProjection<'_, 'source>,
    record: &Record<'source>,
    rank: &SourcedValue<PurchaseRank>,
) -> Result<(), CatalogError> {
    let tree = record.tree().map_err(CatalogError::DataForge)?;
    let admission = RecordObservation::capture(projection.original(), record)?;
    let full = definition_read::with_definition(
        &tree,
        projection,
        &admission,
        None,
        |definition, originals| crate::item::source(&tree, definition, originals, projection),
    )?;
    let (_, row) = full.value;
    let playability = row.playability();
    let expected = match rank.value() {
        Some(PurchaseRank::Player) => Some(Playability::Player),
        Some(PurchaseRank::NonPlayer) => Some(Playability::Special),
        None => None,
    };
    assert_eq!(
        playability.value().copied(),
        expected,
        "full Item playability preserves the rank or applies the original fallback"
    );
    if rank.value() == Some(&PurchaseRank::NonPlayer) {
        assert!(
            rank.origin()
                .source_uses_refs()
                .iter()
                .all(|used| { playability.origin().source_uses_refs().contains(used) }),
            "the original fallback includes the purchase evidence"
        );
    } else {
        assert_eq!(
            playability.origin(),
            rank.origin(),
            "both paths use the exact same proof"
        );
    }
    Ok(())
}

#[test]
fn readable_non_locale_purchase_field_stays_unavailable() -> TestResult {
    let fixture =
        structures(TestType::String).record(record(1, vec![purchase(TestValue::text(PLAYER))]));
    let (_folder, archive) = archive(&fixture)?;
    let no_languages = TextSources::read_languages(&archive, &[])?;
    test_support::with_archive_projection(&archive, &no_languages, |forge, projection| {
        let record = forge
            .record(0)
            .ok_or(ExportAccountingError::MissingFieldInspection)?;
        let projection = projection.for_kind(Kind::Items);
        let tree = record.tree().map_err(CatalogError::DataForge)?;
        let rank = read_rank(&projection, &record, &tree)?;
        assert_eq!(
            rank.state(),
            SourceValueState::Invalid,
            "String is not an admitted purchase Locale"
        );
        assert_eq!(
            rank.value(),
            None,
            "malformed storage cannot decide nonplayer rank"
        );
        check_full_playability(&projection, &record, &rank)
    })?;
    Ok(())
}

fn archive(
    database: &DataForgeBuilder,
) -> Result<(tempfile::TempDir, Archive), Box<dyn std::error::Error>> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("item-purchase-rank.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", database.build()).encrypted())
        .write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?;
    Ok((folder, archive))
}

fn structures(storage: TestType) -> DataForgeBuilder {
    DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new(ENTITY)
                .array("Components", TestType::StrongPointer(COMPONENT.to_owned())),
        )
        .structure(TestStruct::new(COMPONENT))
        .structure(
            TestStruct::new(PURCHASABLE)
                .parent(COMPONENT)
                .field("displayName", storage),
        )
}

fn locale_fixture() -> DataForgeBuilder {
    structures(TestType::Locale)
        .record(record(
            1,
            vec![
                TestValue::Null,
                purchase(TestValue::text(PLAYER)),
                bad_component(),
            ],
        ))
        .record(record(
            2,
            vec![
                purchase(TestValue::text(PLACEHOLDER)),
                purchase(TestValue::text(PLAYER)),
            ],
        ))
        .record(record(
            3,
            vec![bad_component(), purchase(TestValue::text(PLAYER))],
        ))
        .record(record(4, vec![]))
        .record(record(
            5,
            vec![purchase(TestValue::Raw(
                (u32::MAX - 1).to_le_bytes().to_vec(),
            ))],
        ))
}

fn record(guid: u128, components: Vec<TestValue>) -> TestRecord {
    TestRecord::new(
        guid,
        ENTITY,
        format!("Fixture.Rank{guid}"),
        format!("fixture/rank{guid}.xml"),
    )
    .value("Components", TestValue::Array(components))
}

fn purchase(display_name: TestValue) -> TestValue {
    TestValue::Struct(TestInstance::new(PURCHASABLE).value("displayName", display_name))
}

fn bad_component() -> TestValue {
    TestValue::Raw([(u32::MAX - 1).to_le_bytes(), 2_u32.to_le_bytes()].concat())
}
