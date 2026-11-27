// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::CatalogError;
use p4kx_export::catalog_queries::equipment::interactions::InteractionSnapshot;
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordCoverage, CatalogRecordSelection,
    DatasetAvailability,
};
use p4kx_extraction::{Archive, CancelToken, DataForgeEntry, Language, SourceValueState};
use p4kx_testkit::{DataForgeBuilder, TestEntry, TestRecord, TestType, TestValue};
use serde_json::{Value, json};

use super::{DcbOrigin, TestResult, common, fixture, kiosk, proof};
use crate::sdk_full_proof as full_proof;

const ROWS: &str = "/configurations/interactions/rows";
const ORIGINAL: u32 = 0;
const ALIAS: u32 = 3;
const LOCALES: CatalogLanguageSelection<'static> =
    CatalogLanguageSelection::Selected(&[Language::French]);
const DISPLAY: &str = "Interaction de test";

fn database() -> DataForgeBuilder {
    fixture::database().record(
        TestRecord::new(
            fixture::OWNER,
            "EntityClassDefinition",
            "EntityClassDefinition.InteractionVisitAlias",
            "libs/foundry/records/entities/scitem/fixture/interaction_visit_alias.xml",
        )
        .with_instance_index(0),
    )
}

fn localized_archive() -> TestResult<(tempfile::TempDir, Archive)> {
    common::open_bytes_with_entries(
        database().build(),
        DcbOrigin::Encrypted,
        [TestEntry::stored(
            Language::French.global_ini_path(),
            format!("fixture_interaction={DISPLAY}\n"),
        )],
    )
}

#[test]
fn interaction_visits_match_normal_rows_and_their_physical_origins() -> TestResult {
    let (_directory, archive) = localized_archive()?;
    let entry = DataForgeEntry::read(&archive)?;
    let catalog: Value = serde_json::from_slice(
        p4kx_export::build_catalog_from(
            &archive,
            &entry,
            common::options().with_full_provenance(),
        )?
        .json(),
    )?;
    assert_eq!(
        catalog
            .pointer("/exportHealth/publicationMode")
            .and_then(Value::as_str),
        Some("normal"),
        "compare the completed Normal v3 output"
    );
    let mut all = Vec::new();
    let summary = full_proof::open(&archive, LOCALES, None)?.visit_interactions(
        CatalogRecordSelection::All,
        None,
        |index, snapshot| -> TestResult {
            assert_eq!(snapshot.build(), archive.build(), "snapshot Build");
            assert_eq!(
                snapshot.coverage(),
                &CatalogRecordCoverage::default(),
                "complete owner"
            );
            check_owner(index, snapshot, &catalog)?;
            all.push(index);
            Ok(())
        },
    )?;
    assert_eq!(
        all,
        [ORIGINAL, ALIAS],
        "a duplicate GUID retains both physical owners"
    );
    assert_eq!(
        (summary.owner_count(), summary.row_count()),
        (2, 4),
        "two distinct component slots per owner"
    );
    assert_eq!(
        summary.coverage(),
        &CatalogRecordCoverage::default(),
        "complete selection"
    );
    assert_eq!(
        serde_json::to_value(summary.availability())?,
        *proof::at(&catalog, "/configurations/interactions/availability")?,
        "declared family availability agrees with Normal"
    );
    check_selected(&archive, &entry, &catalog)
}

fn check_owner(index: u32, snapshot: &InteractionSnapshot<'_>, catalog: &Value) -> TestResult {
    let expected: Vec<_> = proof::rows(catalog, ROWS)?
        .iter()
        .filter(|row| {
            row.pointer("/ownerRecordIndex/value")
                .and_then(Value::as_u64)
                == Some(u64::from(index))
        })
        .collect();
    let rows = serde_json::to_value(snapshot.rows())?;
    let expected = serde_json::to_value(expected)?;
    assert_eq!(
        serde_json::to_vec(&rows)?,
        serde_json::to_vec(&expected)?,
        "the original component order and all wire cells agree with Normal at Record {index}"
    );
    let owner_inventory = json!({"provenance": snapshot.source_inventory()});
    assert_eq!(
        proof::with_origins(rows, &owner_inventory)?,
        proof::with_origins(expected, catalog)?,
        "each reachable source use and physical source agrees with Normal"
    );

    assert_eq!(
        snapshot.available_languages(),
        [Language::French],
        "only the selected locale is loaded"
    );
    let text = snapshot
        .game_texts()
        .first()
        .ok_or("owner-local protected text")?;
    assert_eq!(
        text.translations()
            .value()
            .and_then(|languages| languages.get("fr"))
            .and_then(|forms| forms.source_text.value())
            .map(std::borrow::Cow::as_ref),
        Some(DISPLAY),
        "the owner-local registry retains its selected protected translation"
    );
    Ok(())
}

fn check_selected(archive: &Archive, entry: &DataForgeEntry, catalog: &Value) -> TestResult {
    let session = full_proof::from_entry(archive, entry, LOCALES, None)?;
    let mut selected = Vec::new();
    let by_guid = session.visit_interactions(
        CatalogRecordSelection::Guid(common::guid(fixture::OWNER).parse()?),
        None,
        |index, snapshot| -> TestResult {
            check_owner(index, snapshot, catalog)?;
            selected.push(index);
            Ok(())
        },
    )?;
    assert_eq!(
        selected,
        [ORIGINAL, ALIAS],
        "GUID selection retains physical order"
    );
    assert_eq!(by_guid.row_count(), 4, "all same-GUID rows");
    let point = CatalogRecordSelection::RecordIndex(ALIAS);
    let expected = full_proof::from_entry(archive, entry, LOCALES, None)?.with_interactions(
        point,
        None,
        |snapshot| serde_json::to_value(snapshot.source_inventory()),
    )??;
    let by_index = full_proof::from_entry(archive, entry, LOCALES, None)?.visit_interactions(
        point,
        None,
        |index, snapshot| -> TestResult {
            assert_eq!(index, ALIAS, "the requested physical owner");
            assert_eq!(
                serde_json::to_value(snapshot.source_inventory())?,
                expected,
                "the same owner-local inventory as the existing point SDK"
            );
            Ok(())
        },
    )?;
    assert_eq!(
        (by_index.owner_count(), by_index.row_count()),
        (1, 2),
        "only the selected physical owner"
    );
    Ok(())
}

#[test]
fn interaction_visits_stop_on_callback_error_or_cancellation() -> TestResult {
    let (_directory, archive) = common::open(&database(), DcbOrigin::Plain)?;
    let session =
        CatalogQuerySession::open(&archive, CatalogLanguageSelection::Selected(&[]), None)?;
    let parsed = session.parsed(None)?;
    let mut observed = Vec::new();
    let stopped = parsed.visit_interactions(
        CatalogRecordSelection::All,
        None,
        |index, _| -> TestResult {
            observed.push(index);
            Err("consumer stopped".into())
        },
    );
    assert_eq!(
        observed,
        [ORIGINAL],
        "after an error there is no second callback"
    );
    assert_eq!(
        stopped.err().map(|error| error.to_string()).as_deref(),
        Some("consumer stopped"),
        "the caller can diagnose the original failure"
    );

    let cancel = CancelToken::new();
    observed.clear();
    let cancelled = parsed.visit_interactions(
        CatalogRecordSelection::All,
        Some(&cancel),
        |index, _| -> Result<(), CatalogError> {
            observed.push(index);
            cancel.cancel();
            Ok(())
        },
    );
    assert_eq!(
        observed,
        [ORIGINAL],
        "cancellation stops before the next physical owner"
    );
    assert!(
        matches!(cancelled, Err(CatalogError::Cancelled)),
        "a cancelled traversal has no terminal summary"
    );
    Ok(())
}

#[test]
fn interaction_visits_report_partial_and_empty_coverage_separately() -> TestResult {
    let (_directory, archive) = common::open(
        &kiosk::database(Some((TestType::String, TestValue::text("invalid flag")))),
        DcbOrigin::Plain,
    )?;
    let session =
        CatalogQuerySession::open(&archive, CatalogLanguageSelection::Selected(&[]), None)?;
    let partial = session.visit_interactions(
        CatalogRecordSelection::All,
        None,
        |index, snapshot| -> TestResult {
            let [row] = snapshot.rows() else {
                return Err("expected one physical kiosk occurrence".into());
            };
            let flag = row.support_walk_to_align_in_vr().ok_or("Kiosk flag")?;
            assert_eq!(
                flag.state(),
                SourceValueState::Invalid,
                "the malformed stored shape survives"
            );
            assert_eq!(
                snapshot.coverage().incomplete_fields,
                [index],
                "original incomplete owner"
            );
            Ok(())
        },
    )?;
    assert_eq!(
        (partial.owner_count(), partial.row_count()),
        (1, 1),
        "the partial row is counted"
    );
    assert_eq!(
        partial.coverage().incomplete_fields,
        [ORIGINAL],
        "terminal incomplete owner"
    );
    let empty = session.visit_interactions(
        CatalogRecordSelection::RecordIndex(u32::MAX),
        None,
        |_, _| -> TestResult { Err("an absent physical owner must never be visited".into()) },
    )?;
    assert_eq!(
        (empty.owner_count(), empty.row_count()),
        (0, 0),
        "selected Record does not exist"
    );
    assert!(
        matches!(empty.availability(), DatasetAvailability::Available),
        "family is still declared"
    );
    assert_eq!(
        empty.coverage(),
        &CatalogRecordCoverage::default(),
        "selection is complete and empty"
    );
    Ok(())
}
