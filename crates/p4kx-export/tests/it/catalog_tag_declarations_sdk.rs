// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::tags::{
    TagDeclarationFamily, TagDeclarationSnapshot, with_tag_declarations, with_tag_declarations_from,
};
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, DatasetAvailability,
};
use p4kx_export::{CatalogError, CatalogPublicationMode, build_catalog_from};
use p4kx_extraction::{CancelToken, DataForgeEntry, InputScope, SourceLocation, SourceValueState};
use p4kx_testkit::DataForgeBuilder;
use serde_json::{Value, json};

use crate::catalog_tags_fixture as fixture;
use crate::commodity_v3_fixture::{self as archive_fixture, DcbOrigin, TestResult};
use crate::commodity_v3_source_inventory::{body, reference_closure, uses};

#[test]
fn tag_declaration_sdk_matches_the_encrypted_catalog_graph() -> TestResult {
    let (_directory, archive) = archive_fixture::open(&fixture::graph(), DcbOrigin::Encrypted)?;
    let entry = DataForgeEntry::read(&archive)?;
    let mut options = archive_fixture::options();
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    let catalog: Value =
        serde_json::from_slice(build_catalog_from(&archive, &entry, &options)?.json())?;
    with_tag_declarations_from(
        &archive,
        &entry,
        TagDeclarationFamily::All,
        CatalogRecordSelection::All,
        None,
        |snapshot| verify_graph(&snapshot, &catalog),
    )?
}

fn verify_graph(snapshot: &TagDeclarationSnapshot<'_>, catalog: &Value) -> TestResult {
    let definitions = snapshot
        .definitions
        .as_ref()
        .ok_or("requested definitions")?;
    let roots = snapshot.roots.as_ref().ok_or("requested root arrays")?;
    let document = json!({
        "provenance": snapshot.source_inventory,
        "definitions": {"tags": {"availability": definitions.availability, "rows": definitions.rows}},
        "declarations": {"tagRoots": {"availability": roots.availability, "rows": roots.rows}},
    });
    for pointer in ["/definitions/tags", "/declarations/tagRoots"] {
        assert_eq!(
            document.pointer(pointer),
            catalog.pointer(pointer),
            "the SDK preserves the canonical table at {pointer}"
        );
    }
    reference_closure(&document)?;
    let vertex = "/definitions/tags/rows/0/children/value";
    let edge = fixture::at(&document, &format!("{vertex}/3"))?;
    assert_eq!(
        edge.get("containsEncryptedSource"),
        Some(&json!(true)),
        "the authored child slot is encrypted"
    );
    assert_eq!(
        uses(&document, edge)?,
        uses(catalog, edge)?,
        "the child slot resolves through the same original ledger uses"
    );
    assert!(
        uses(&document, edge)?.contains(&(0, body("children[3]"), String::from("payload"))),
        "the authored edge keeps its physical array coordinate"
    );
    assert_ne!(
        edge.get("sourceUsesRefs"),
        fixture::at(&document, &format!("{vertex}/4"))?.get("sourceUsesRefs"),
        "two repeated edges preserve their separate source positions"
    );
    Ok(())
}

#[test]
fn tag_declaration_sdk_scopes_aliases_and_root_targets() -> TestResult {
    let (_directory, archive) = archive_fixture::open(&fixture::graph(), DcbOrigin::Plain)?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let leaf = forge.record(1).ok_or("original leaf Record")?.guid();
    let session = CatalogQuerySession::from_entry(
        &archive,
        &entry,
        CatalogLanguageSelection::AllAvailable,
        None,
    )?;
    let parsed = session.parsed(None)?;
    parsed.with_tag_declarations(
        TagDeclarationFamily::Definitions,
        CatalogRecordSelection::Guid(leaf),
        None,
        verify_aliases,
    )??;
    parsed.with_tag_declarations(
        TagDeclarationFamily::Roots,
        CatalogRecordSelection::RecordIndex(4),
        None,
        verify_root,
    )?
}

fn verify_aliases(snapshot: TagDeclarationSnapshot<'_>) -> TestResult {
    assert!(
        snapshot.roots.is_none(),
        "only the selected family is projected"
    );
    let tags = snapshot.definitions.ok_or("declared Tag definitions")?;
    let [leaf, alias] = tags.rows.as_slice() else {
        return Err("both physical GUID occurrences must be present".into());
    };
    for (row, index, name) in [(leaf, 1, "Same"), (alias, 5, "Shadow")] {
        assert_eq!(
            row.source().record_index().value(),
            Some(&index),
            "GUID occurrences are ordered by physical index"
        );
        assert_eq!(row.tag_name().value(), Some(&name), "authored alias name");
        assert_eq!(
            row.tag_name().origin().contains_encrypted_source(),
            Some(false),
            "plaintext is proven for a complete original name"
        );
    }
    Ok(())
}

fn verify_root(snapshot: TagDeclarationSnapshot<'_>) -> TestResult {
    assert!(
        snapshot.definitions.is_none(),
        "root lookups return only declarations"
    );
    let roots = snapshot.roots.ok_or("original TagDatabase array")?;
    let [row] = roots.rows.as_slice() else {
        return Err("one exact physical TagDatabase declaration".into());
    };
    assert_eq!(row.source().record_index().value(), Some(&4), "exact owner");
    let [null, _, root, _, duplicate] = row.tags().value().ok_or("authored root array")?.as_slice()
    else {
        return Err("five original root occurrences".into());
    };
    assert_eq!(null.state(), SourceValueState::Null, "explicit null slot");
    for cell in [root, duplicate] {
        let target = cell.value().ok_or("original Tag reference")?;
        assert_eq!(
            target.target_record_index().value(),
            Some(&0),
            "reference carries the external physical Tag header"
        );
        assert_eq!(
            cell.origin().contains_encrypted_source(),
            Some(false),
            "the original slot is plaintext"
        );
    }
    let inventory = serde_json::to_value(&snapshot.source_inventory)?;
    let uses = inventory
        .get("uses")
        .and_then(Value::as_array)
        .ok_or("SDK uses")?;
    assert!(
        uses.iter()
            .all(|used| used.pointer("/location/field") != Some(&json!("tagName"))),
        "resolving target headers does not project their authored names"
    );
    Ok(())
}

#[test]
fn tag_declaration_sdk_distinguishes_absent_families_from_empty_selection() -> TestResult {
    for (database, expected) in [
        (DataForgeBuilder::new().version(8), "absent"),
        (fixture::schema(), "available"),
    ] {
        let (_directory, archive) = archive_fixture::open(&database, DcbOrigin::Plain)?;
        with_tag_declarations(
            &archive,
            TagDeclarationFamily::All,
            CatalogRecordSelection::RecordIndex(u32::MAX),
            None,
            |snapshot| check_empty(snapshot, expected),
        )??;
    }
    Ok(())
}

fn check_empty(snapshot: TagDeclarationSnapshot<'_>, expected: &str) -> TestResult {
    let tags = snapshot.definitions.ok_or("requested tag definitions")?;
    let roots = snapshot.roots.ok_or("requested root arrays")?;
    for (empty, availability) in [
        (tags.rows.is_empty(), tags.availability),
        (roots.rows.is_empty(), roots.availability),
    ] {
        assert!(empty, "the missing physical index cannot yield rows");
        assert!(
            !matches!(availability, DatasetAvailability::Unavailable),
            "family declarations were readable"
        );
        assert_eq!(
            serde_json::to_value(availability)?,
            json!(expected),
            "selection is independent of the declared source family"
        );
    }
    let mut uses = snapshot.source_inventory.iter_uses().peekable();
    assert!(
        uses.peek().is_some(),
        "physical DataForge Entry metadata is retained"
    );
    assert!(
        uses.all(|usage| {
            usage.scope() == InputScope::ArchiveDirectoryMetadata
                && matches!(usage.location(), SourceLocation::Metadata { .. })
        }),
        "a missing Record index uses only Archive directory metadata",
    );
    Ok(())
}

#[test]
fn tag_declaration_sdk_honors_cancellation() -> TestResult {
    let (_directory, archive) = archive_fixture::open(&fixture::graph(), DcbOrigin::Plain)?;
    let cancel = CancelToken::new();
    cancel.cancel();
    let result: Result<(), _> = with_tag_declarations(
        &archive,
        TagDeclarationFamily::Definitions,
        CatalogRecordSelection::All,
        Some(&cancel),
        |_| panic!("a cancelled query must not publish a snapshot"),
    );
    assert!(
        matches!(result, Err(CatalogError::Cancelled)),
        "explicit cancellation"
    );
    Ok(())
}
