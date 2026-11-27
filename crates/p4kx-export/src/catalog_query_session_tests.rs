// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::path::Path;

use p4kx_extraction::{Archive, CancelToken, DataForgeEntry, Language, ProvenanceMode};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestRecord, TestStruct, TestType, TestValue,
    write_build_manifest,
};

use super::{CatalogLanguageSelection, CatalogQuerySession};
use crate::CatalogError;
use crate::catalog_queries::CatalogRecordSelection;
use crate::mining_locations::MiningLocationOptions;

type TestResult = Result<(), Box<dyn Error>>;

const ORIGINAL_CHANGE: &str = "12345";
const REPLACEMENT_CHANGE: &str = "12346";

#[test]
fn localized_and_unlocalized_queries_reuse_one_archive_bound_session() -> TestResult {
    let folder = tempfile::tempdir()?;
    let archive = open_fixture(folder.path())?;
    let languages = [Language::French, Language::English, Language::English];
    let session = CatalogQuerySession::open(
        &archive,
        CatalogLanguageSelection::Selected(&languages),
        None,
    )?
    .with_provenance_mode(ProvenanceMode::Full);
    let build = session.build().ok_or("known fixture Build")?;
    assert_eq!(
        build.change_number(),
        ORIGINAL_CHANGE,
        "report the opened Build"
    );
    assert_eq!(
        session.languages(),
        &[Language::English, Language::French],
        "normalize duplicates and preserve absent requested languages"
    );
    assert!(
        session.dataforge_origin().matches_archive(&archive),
        "retain the exact opened Archive binding"
    );

    let selection = CatalogRecordSelection::All;
    let categories = session.read_blueprint_categories(selection, None)?;
    let lists = session.read_blueprint_category_lists(selection, None)?;
    assert_eq!(categories.rows.len(), 1, "the selected category exists");
    assert_eq!(
        categories.source_inventory.source_count(),
        1,
        "only the DCB"
    );
    assert!(
        lists.rows.is_empty(),
        "no category list came from the category domain"
    );
    verify_localized_queries(&session, selection)?;
    assert_eq!(
        categories.source_inventory.source_count(),
        1,
        "later localized reads leave the earlier inventory self-contained"
    );
    Ok(())
}

fn verify_localized_queries(
    session: &CatalogQuerySession<'_>,
    selection: CatalogRecordSelection,
) -> TestResult {
    let gameplay = session.read_gameplay_properties(selection, None)?;
    assert_eq!(gameplay.rows.len(), 1, "one authored gameplay property");
    assert_eq!(
        gameplay.public_names.len(),
        1,
        "the original name is consulted"
    );
    assert!(
        gameplay.source_inventory.source_count() > 1,
        "the result uses localization"
    );
    assert_eq!(gameplay.build.as_ref(), session.build(), "same Build");
    assert_eq!(
        gameplay.requested_languages,
        session.languages(),
        "query inherits the session policy"
    );
    assert_eq!(
        gameplay.available_languages,
        [Language::English],
        "only the present locale is available"
    );

    let english =
        session.read_mining_locations(MiningLocationOptions::new(Language::English, None))?;
    let german =
        session.read_mining_locations(MiningLocationOptions::new(Language::German, None))?;
    assert!(
        english.coverage.language_available,
        "mining uses its requested language within the session policy"
    );
    assert!(
        !german.coverage.language_available,
        "mining does not read the excluded encrypted language"
    );
    Ok(())
}

#[test]
fn a_cancelled_request_leaves_owned_results_and_session_available() -> TestResult {
    let folder = tempfile::tempdir()?;
    let archive = open_fixture(folder.path())?;
    let foreign_folder = tempfile::tempdir()?;
    let foreign = open_fixture(foreign_folder.path())?;
    let entry = DataForgeEntry::read(&archive)?;
    let languages = CatalogLanguageSelection::Selected(&[]);
    assert!(
        matches!(
            CatalogQuerySession::from_entry(&foreign, &entry, languages, None),
            Err(CatalogError::SourceOriginMismatch)
        ),
        "the same fixture bytes at another Archive cannot supply source evidence"
    );
    let session = CatalogQuerySession::from_entry(&archive, &entry, languages, None)?;
    let selection = CatalogRecordSelection::RecordIndex(0);
    let previous = session.read_blueprint_categories(selection, None)?;
    let cancel = CancelToken::default();
    cancel.cancel();
    assert!(
        matches!(
            session.read_blueprint_categories(selection, Some(&cancel)),
            Err(CatalogError::Cancelled)
        ),
        "cancellation does not publish a query result"
    );
    let current = session.read_blueprint_categories(selection, None)?;
    assert_eq!(
        current.rows.len(),
        1,
        "the next request can read the session"
    );
    assert_eq!(
        current.rows[0].source.guid.value(),
        previous.rows[0].source.guid.value(),
        "the previous owned result remains available"
    );
    Ok(())
}

#[test]
fn an_archive_change_observed_at_query_end_permanently_invalidates_the_session() -> TestResult {
    let folder = tempfile::tempdir()?;
    let archive = open_fixture(folder.path())?;
    let session =
        CatalogQuerySession::open(&archive, CatalogLanguageSelection::Selected(&[]), None)?;
    let selection = CatalogRecordSelection::RecordIndex(0);
    let previous = session.read_blueprint_categories(selection, None)?;
    let mut updated = Ok(());
    let failed: Result<(), CatalogError> = session.project(None, |_, _| {
        updated = write_manifest(folder.path(), REPLACEMENT_CHANGE);
        Err(CatalogError::SourceOriginMismatch)
    });
    updated?;
    assert!(
        matches!(failed, Err(CatalogError::ArchiveChanged(_))),
        "the session observes a manifest change even when work failed"
    );
    write_manifest(folder.path(), ORIGINAL_CHANGE)?;
    assert!(
        matches!(
            session.read_blueprint_category_lists(selection, None),
            Err(CatalogError::ArchiveChanged(_))
        ),
        "restoring a manifest cannot make an invalidated session current"
    );
    assert_eq!(
        previous.rows.len(),
        1,
        "already returned data remains usable"
    );
    Ok(())
}

pub(super) fn open_fixture(folder: &Path) -> Result<Archive, Box<dyn Error>> {
    let category = TestRecord::new(
        0x1100_0000_0000_4000_8000_0000_0000_0001,
        "BlueprintCategoryRecord",
        "BlueprintCategoryRecord.Fixture",
        "libs/foundry/records/crafting/blueprintcategories/fixture.xml",
    );
    let property = TestRecord::new(
        0x1100_0000_0000_4000_8000_0000_0002,
        "CraftingGameplayPropertyDef",
        "CraftingGameplayPropertyDef.Fixture",
        "libs/foundry/records/crafting/craftedproperties/fixture.xml",
    )
    .value("propertyName", TestValue::text("@fixture"));
    let database = DataForgeBuilder::new()
        .structure(TestStruct::new("BlueprintCategoryRecord"))
        .structure(
            TestStruct::new("CraftingGameplayPropertyDef").field("propertyName", TestType::Locale),
        )
        .record(category)
        .record(property)
        .build();
    let path = folder.join("Data.p4k");
    write_manifest(folder, ORIGINAL_CHANGE)?;
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", database))
        .entry(TestEntry::stored(
            Language::English.global_ini_path(),
            b"fixture=Example\n",
        ))
        .entry(
            TestEntry::stored(Language::German.global_ini_path(), b"fixture=Beispiel\n")
                .encrypted(),
        )
        .write_to(&path)?;
    Ok(Archive::open(path)?)
}

fn write_manifest(folder: &Path, change: &str) -> std::io::Result<()> {
    write_build_manifest(folder, "test-main", "5.0.0", change, "fixture")
}

#[test]
fn parsed_queries_share_one_physical_view_and_keep_checking_archive_freshness() -> TestResult {
    let folder = tempfile::tempdir()?;
    let archive = open_fixture(folder.path())?;
    let session =
        CatalogQuerySession::open(&archive, CatalogLanguageSelection::Selected(&[]), None)?;
    let parsed = session.parsed(None)?;
    let category =
        parsed.read_blueprint_categories(CatalogRecordSelection::RecordIndex(0), None)?;
    let properties =
        parsed.read_gameplay_properties(CatalogRecordSelection::RecordIndex(1), None)?;
    assert_eq!(category.rows.len(), 1, "one selected category");
    assert_eq!(properties.rows.len(), 1, "one selected gameplay property");
    for _ in 0..2 {
        parsed.project(None, |_, entry, forge| {
            assert!(
                std::ptr::eq(forge, std::ptr::from_ref(&parsed.forge)),
                "the same parsed DataForge"
            );
            assert!(
                entry.entry().origin_of(forge).is_some(),
                "the retained view borrows the same decoded input",
            );
            Ok(())
        })?;
    }
    write_manifest(folder.path(), REPLACEMENT_CHANGE)?;
    assert!(
        matches!(
            parsed.read_blueprint_category_lists(CatalogRecordSelection::All, None),
            Err(CatalogError::ArchiveChanged(_))
        ),
        "parsed access checks whether the Archive is still current"
    );
    assert_eq!(category.rows.len(), 1, "completed snapshots remain owned");
    Ok(())
}
