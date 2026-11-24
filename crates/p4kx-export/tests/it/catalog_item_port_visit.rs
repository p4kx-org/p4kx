// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::{
    self, CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection,
    DatasetAvailability, ItemPortSnapshot,
};
use p4kx_export::{CatalogError, build_catalog};
use p4kx_extraction::{CancelToken, DataForgeEntry, Language};
use p4kx_testkit::{DataForgeBuilder, TestEntry, TestRecord, TestStruct, TestType, TestValue};
use serde_json::Value;

use super::{Layout, OWNER, PORT_KEY, PORTS, TestResult, common, database, proof};

#[test]
fn item_port_visitor_retains_canonical_rows_and_owner_local_origins() -> TestResult {
    let (_folder, archive) = common::open_bytes_with_entries(
        database(Layout::Localized).build(),
        common::DcbOrigin::Encrypted,
        [TestEntry::stored(
            Language::French.global_ini_path(),
            b"fixture_port_display=Point de fixation\n",
        )],
    )?;
    let entry = DataForgeEntry::read(&archive)?;
    let catalog: Value =
        serde_json::from_slice(build_catalog(&archive, &common::options())?.json())?;
    let languages = CatalogLanguageSelection::Selected(&[Language::French]);
    let mut indices = Vec::new();
    let mut observed = Vec::new();
    let summary = catalog_queries::visit_item_ports_from(
        &archive,
        &entry,
        CatalogRecordSelection::All,
        languages,
        None,
        |index, snapshot| -> TestResult {
            indices.push(index);
            observed.push(observed_port(index, snapshot)?);
            Ok(())
        },
    )?;
    assert_eq!(indices, [0, 1], "physical Record order");
    assert_eq!(summary.owner_count(), 2, "each selected owner visited");
    assert_eq!(
        summary.row_count(),
        2,
        "both physical port components visited"
    );
    assert!(
        matches!(summary.availability(), DatasetAvailability::Available),
        "authored declarations"
    );
    assert!(
        summary.coverage().incomplete_fields.is_empty(),
        "complete port projection"
    );
    assert_eq!(
        observed.as_slice(),
        proof::rows(&catalog, PORTS)?,
        "Normal rows and origin references"
    );
    Ok(())
}

fn observed_port(index: u32, snapshot: &ItemPortSnapshot<'_>) -> TestResult<Value> {
    let [row] = snapshot.rows.as_slice() else {
        return Err("one physical port component per selected fixture owner".into());
    };
    assert_eq!(
        row.owner_record_index().value(),
        Some(&index),
        "physical owner"
    );
    assert_eq!(
        row.component_ordinal().value(),
        Some(&1),
        "physical component"
    );
    assert_eq!(
        snapshot.available_languages,
        [Language::French],
        "requested locale"
    );
    check_localized_label(index, snapshot)?;
    Ok(serde_json::to_value(row)?)
}

fn check_localized_label(index: u32, snapshot: &ItemPortSnapshot<'_>) -> TestResult {
    let name = snapshot
        .public_names
        .iter()
        .find(|name| name.key.value().map(String::as_str) == Some(PORT_KEY));
    if index != 0 {
        assert!(
            name.is_none(),
            "no localized label inherited from another owner"
        );
        return Ok(());
    }
    let name = name.ok_or("the original owner's localized port label")?;
    let translation = name
        .translations
        .value()
        .and_then(|texts| texts.get("fr"))
        .ok_or("selected French translation")?;
    assert_eq!(
        translation
            .source_text
            .value()
            .map(std::borrow::Cow::as_ref),
        Some("Point de fixation"),
        "authored French translation is retained"
    );
    let references = name.id.origin().source_uses_refs();
    assert!(!references.is_empty(), "authored public label origin");
    for reference in references {
        assert!(
            snapshot
                .source_inventory
                .iter_uses()
                .any(|row| row.id() == reference),
            "owner-local public label origin is closed"
        );
    }
    Ok(())
}

#[test]
fn item_port_visitor_reports_unknown_component_selection() -> TestResult {
    let forge = malformed_port_owner();
    let (_folder, archive) = common::open(&forge, common::DcbOrigin::Plain)?;
    let selection = CatalogRecordSelection::RecordIndex(0);
    let languages = CatalogLanguageSelection::Selected(&[]);
    let aggregate =
        catalog_queries::with_item_ports(&archive, selection, languages, None, |snapshot| {
            snapshot.coverage
        })?;
    assert!(
        aggregate.incomplete_fields.is_empty(),
        "existing aggregate coverage contract"
    );
    let session = CatalogQuerySession::open(&archive, languages, None)?;
    let summary = session
        .parsed(None)?
        .visit_item_ports(selection, None, |index, snapshot| {
            assert_eq!(index, 0, "the selected malformed Record is readable");
            assert!(
                snapshot.rows.is_empty(),
                "no port container could be selected"
            );
            assert_eq!(
                snapshot.coverage.incomplete_fields,
                [0],
                "owner-local selection error"
            );
            Ok::<_, CatalogError>(())
        })?;
    assert_eq!(
        summary.owner_count(),
        1,
        "owner admitted despite malformed components"
    );
    assert_eq!(summary.row_count(), 0, "no rows recovered");
    assert!(
        matches!(summary.availability(), DatasetAvailability::Available),
        "port type is declared"
    );
    assert_eq!(
        summary.coverage().incomplete_fields,
        [0],
        "terminal coverage preserves the failure"
    );
    Ok(())
}

fn malformed_port_owner() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("EntityClassDefinition").field("Components", TestType::String))
        .structure(TestStruct::new("SItemPortContainerComponentParams"))
        .record(
            TestRecord::new(
                OWNER,
                "EntityClassDefinition",
                "EntityClassDefinition.FixtureMalformedPortOwner",
                "libs/foundry/records/entities/scitem/fixture/malformed_port_owner.xml",
            )
            .value("Components", TestValue::text("invalid collection")),
        )
}

#[test]
fn item_port_visitor_keeps_scoped_field_faults() -> TestResult {
    for (layout, expected) in [(Layout::UnrelatedInvalid, 0), (Layout::Unreadable, 1)] {
        let (_folder, archive) = common::open(&database(layout), common::DcbOrigin::Plain)?;
        let summary = catalog_queries::visit_item_ports(
            &archive,
            CatalogRecordSelection::RecordIndex(0),
            CatalogLanguageSelection::Selected(&[]),
            None,
            |_, snapshot| {
                assert_eq!(
                    snapshot.rows.len(),
                    1,
                    "retain the available port container"
                );
                assert_eq!(
                    snapshot.coverage.incomplete_fields.len(),
                    expected,
                    "port-only read faults"
                );
                Ok::<_, CatalogError>(())
            },
        )?;
        assert_eq!(
            summary.coverage().incomplete_fields.len(),
            expected,
            "terminal coverage"
        );
    }
    Ok(())
}

#[test]
fn item_port_visitor_loads_locales_with_the_first_readable_owner() -> TestResult {
    let (_folder, archive) = common::open_bytes_with_entries(
        database(Layout::Localized).build(),
        common::DcbOrigin::Plain,
        [TestEntry::zstd(
            Language::French.global_ini_path(),
            b"fixture_port_display=Port",
        )
        .with_payload(b"invalid zstd frame")],
    )?;
    let languages = CatalogLanguageSelection::Selected(&[Language::French]);
    let empty_selection = CatalogRecordSelection::RecordIndex(u32::MAX);
    let empty = catalog_queries::visit_item_ports(
        &archive,
        empty_selection,
        languages,
        None,
        |_, _| -> Result<(), CatalogError> { panic!("an empty selection has no callback") },
    )?;
    assert_eq!(empty.owner_count(), 0, "no owner uses the requested locale");
    assert_eq!(empty.row_count(), 0, "the empty selection yields no rows");

    let selected = catalog_queries::visit_item_ports(
        &archive,
        CatalogRecordSelection::RecordIndex(0),
        languages,
        None,
        |_, _| -> Result<(), CatalogError> { panic!("locale failure precedes the callback") },
    );
    assert!(matches!(selected, Err(CatalogError::LoadTexts(_))));
    let aggregate =
        catalog_queries::with_item_ports(&archive, empty_selection, languages, None, |_| ());
    assert!(matches!(aggregate, Err(CatalogError::LoadTexts(_))));
    Ok(())
}

#[test]
fn item_port_visitor_stops_after_cancellation() -> TestResult {
    let (_folder, archive) = common::open(&database(Layout::Complete), common::DcbOrigin::Plain)?;
    let cancel = CancelToken::new();
    let mut visited = Vec::new();
    let result = catalog_queries::visit_item_ports(
        &archive,
        CatalogRecordSelection::All,
        CatalogLanguageSelection::Selected(&[]),
        Some(&cancel),
        |index, _| {
            visited.push(index);
            cancel.cancel();
            Ok::<_, CatalogError>(())
        },
    );
    assert!(
        matches!(result, Err(CatalogError::Cancelled)),
        "no terminal summary after cancellation"
    );
    assert_eq!(
        visited,
        [0],
        "cancellation stops traversal before the next Record"
    );
    Ok(())
}
