// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogRecordSelection, with_contracts,
};
use p4kx_extraction::{Archive, Language, SourceLocation};
use p4kx_testkit::{ArchiveBuilder, DataForgeBuilder, TestEntry, TestRecord, TestStruct};

use super::TestResult;
use crate::commodity_v3_fixture::{self, DcbOrigin, guid};

use super::query_source_fixture as fixture;

#[test]
fn contract_sdk_selected_root_keeps_outside_intro_without_unrelated_projectors() -> TestResult {
    let (_folder, archive) =
        commodity_v3_fixture::open_bytes(fixture::database(false, true), DcbOrigin::Plain)?;
    with_contracts(
        &archive,
        CatalogRecordSelection::Guid(guid(fixture::ROOT).parse()?),
        CatalogLanguageSelection::Selected(&[]),
        None,
        |snapshot| {
            assert_eq!(
                snapshot.rows.len(),
                2,
                "the selected parent and valid sub-contract remain"
            );
            assert_eq!(
                snapshot.dependency_generators.len(),
                1,
                "only the related outside owner is fully projected"
            );
            assert_eq!(
                snapshot.coverage.dependencies.related_generators,
                [2],
                "the intro and duplicate owner is a dependency"
            );
            assert!(
                snapshot.coverage.projection_failures.is_empty(),
                "an unrelated malformed source is only surveyed"
            );
            let parent = snapshot
                .rows
                .iter()
                .find(|row| row.base.sub_contract_guid.value().is_none())
                .ok_or("the selected parent row")?;
            let [required] = parent
                .required_intros
                .value()
                .ok_or("authored intro requirements")?
                .as_slice()
            else {
                return Err("one required completion tag".into());
            };
            let [intro] = required
                .intro_contracts
                .value()
                .ok_or("the original grant join")?
                .as_slice()
            else {
                return Err("the outside intro is retained".into());
            };
            assert_eq!(
                intro.value().map(|guid| guid.get().to_string()),
                Some(fixture::INTRO_TEXT.to_owned()),
                "a narrow query resolves the relevant grant outside the selected root"
            );
            assert!(intro.origin().source_uses_refs().iter().any(|reference| snapshot.source_inventory.iter_uses().any(|used| used.id() == reference && matches!(used.location(), SourceLocation::Subresource { namespace, indices, .. } if namespace == "dataforgeValue" && indices.first() == Some(&2)))), "the joined value cites its actual outside source owner");
            Ok::<_, Box<dyn std::error::Error>>(())
        },
    )??;
    Ok(())
}

#[test]
fn contract_sdk_shadowed_root_does_not_become_a_local_winner() -> TestResult {
    let (_folder, archive) =
        commodity_v3_fixture::open_bytes(fixture::database(false, false), DcbOrigin::Plain)?;
    with_contracts(
        &archive,
        CatalogRecordSelection::RecordIndex(2),
        CatalogLanguageSelection::Selected(&[]),
        None,
        |snapshot| {
            let [row] = snapshot.rows.as_slice() else {
                return Err("only the globally surviving intro row".into());
            };
            assert_eq!(
                row.base.guid.value().map(|guid| guid.get().to_string()),
                Some(fixture::INTRO_TEXT.to_owned()),
                "the root's collided parent stays shadowed by the external canonical winner"
            );
            let [shadowed] = snapshot.coverage.shadowed_candidates.as_slice() else {
                return Err("one explicit losing root candidate".into());
            };
            assert_eq!(
                shadowed.selected_record_index, 2,
                "the requested physical candidate is identified"
            );
            assert_eq!(
                shadowed.winner_record_index, 1,
                "the global canonical owner remains identified"
            );
            Ok::<_, Box<dyn std::error::Error>>(())
        },
    )??;
    Ok(())
}

#[test]
fn contract_sdk_failed_related_generator_cannot_supply_positive_intro_grants() -> TestResult {
    let (_folder, archive) =
        commodity_v3_fixture::open_bytes(fixture::database(true, false), DcbOrigin::Plain)?;
    with_contracts(
        &archive,
        CatalogRecordSelection::RecordIndex(1),
        CatalogLanguageSelection::Selected(&[]),
        None,
        |snapshot| {
            assert_eq!(
                snapshot.rows.len(),
                2,
                "the independent selected source remains usable"
            );
            assert_eq!(
                snapshot.coverage.projection_failures.len(),
                1,
                "the related whole-source failure stays explicit"
            );
            let parent = snapshot
                .rows
                .iter()
                .find(|row| row.base.sub_contract_guid.value().is_none())
                .ok_or("the selected parent")?;
            let [required] = parent
                .required_intros
                .value()
                .ok_or("the authored prerequisite")?
                .as_slice()
            else {
                return Err("one authored prerequisite".into());
            };
            assert!(
                required.intro_contracts.value().is_some_and(Vec::is_empty),
                "a source-only positive survey cannot bypass canonical source failure"
            );
            assert!(
                !required.intro_contracts.origin().source_set_complete(),
                "the incomplete dependency inventory is not reported as complete absence"
            );
            Ok::<_, Box<dyn std::error::Error>>(())
        },
    )??;
    Ok(())
}

#[test]
fn contract_sdk_absent_root_does_not_open_locked_localization_or_world_entries() -> TestResult {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("scope.p4k");
    let source = DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("ContractGenerator"))
        .record(TestRecord::new(
            fixture::ROOT,
            "ContractGenerator",
            "ContractGenerator.Empty",
            "libs/foundry/records/contracts/contractgenerator/empty.xml",
        ));
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", source.build()))
        .entry(TestEntry::stored(Language::English.global_ini_path(), b"locked=text\n").encrypted())
        .entry(TestEntry::stored("Data/ObjectContainers/locked.soc", b"locked world").encrypted())
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    for selection in [
        CatalogRecordSelection::RecordIndex(u32::MAX),
        CatalogRecordSelection::RecordIndex(0),
    ] {
        with_contracts(
            &archive,
            selection,
            CatalogLanguageSelection::AllAvailable,
            None,
            |snapshot| {
                assert!(snapshot.rows.is_empty(), "no source contract is admitted");
                assert!(
                    snapshot.available_languages.is_empty(),
                    "no localization payload was prepared"
                );
            },
        )?;
    }
    assert_eq!(
        archive.key_demand().snapshot().unread(),
        0,
        "locked localization and world entries remain outside the empty query"
    );
    Ok(())
}
