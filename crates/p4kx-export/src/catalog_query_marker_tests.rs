// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_extraction::{DataForgeEntry, ProvenanceMode, SourceUses, SourceValue};
use serde_json::Value;

use super::tests::open_fixture;
use super::{CatalogLanguageSelection, CatalogQuerySession};
use crate::catalog_queries::CatalogRecordSelection;
use crate::contract_loadout::LoadoutPort;

#[test]
fn one_decoded_input_supports_markers_by_default_and_full_proof_on_request()
-> Result<(), Box<dyn Error>> {
    let folder = tempfile::tempdir()?;
    let archive = open_fixture(folder.path())?;
    let entry = DataForgeEntry::read(&archive)?;
    let languages = CatalogLanguageSelection::Selected(&[]);
    let markers = CatalogQuerySession::from_entry(&archive, &entry, languages, None)?;
    let full = CatalogQuerySession::from_entry(&archive, &entry, languages, None)?
        .with_provenance_mode(ProvenanceMode::Full);
    let selection = CatalogRecordSelection::RecordIndex(0);
    let marker_row = markers
        .parsed(None)?
        .read_blueprint_categories(selection, None)?;
    let full_row = full.read_blueprint_categories(selection, None)?;

    assert_eq!(
        markers.provenance_mode(),
        ProvenanceMode::MarkersOnly,
        "SDK default mode"
    );
    assert_eq!(marker_row.build, full_row.build, "exact Build identity");
    assert_eq!(
        marker_row.coverage, full_row.coverage,
        "identical record coverage"
    );
    let marker = marker_row.rows.first().ok_or("marker row")?;
    let original = full_row.rows.first().ok_or("full row")?;
    assert_eq!(
        marker.source.guid.value(),
        original.source.guid.value(),
        "same GUID"
    );
    let marker_source = serde_json::to_value(&marker.source)?;
    let original_source = serde_json::to_value(&original.source)?;
    let marker = marker_source.get("guid").ok_or("marker GUID cell")?;
    let original = original_source.get("guid").ok_or("full GUID cell")?;
    assert_eq!(
        marker["containsEncryptedSource"],
        Value::Bool(false),
        "plain original data"
    );
    assert_eq!(
        marker["containsEncryptedSource"], original["containsEncryptedSource"],
        "identical encryption marker"
    );
    assert!(
        marker.get("sourceUsesRefs").is_none(),
        "marker row omits exact references"
    );
    assert!(
        original["sourceUsesRefs"]
            .as_array()
            .is_some_and(|uses| !uses.is_empty()),
        "full row carries exact references"
    );
    Ok(())
}

#[test]
fn unobserved_loadout_fields_keep_the_mode_of_the_port() -> Result<(), Box<dyn Error>> {
    let fields = [
        "minSize",
        "maxSize",
        "flags",
        "portTags",
        "requiredTags",
        "displayNameKey",
    ];
    for mode in [ProvenanceMode::MarkersOnly, ProvenanceMode::Full] {
        let uses = SourceUses::with_provenance_mode(mode);
        let name = uses.value(SourceValue::Present("test_port".to_owned()), &[], false);
        let port = serde_json::to_value(LoadoutPort::named(name))?;
        for field in fields {
            let cell = port.get(field).ok_or("an unobserved port field")?;
            assert_eq!(cell["state"], "absent", "{field}");
            assert!(cell["containsEncryptedSource"].is_null(), "{field}");
            assert_eq!(
                cell.get("sourceUsesRefs").is_some(),
                mode == ProvenanceMode::Full,
                "{field} must retain the port's provenance mode"
            );
        }
    }
    Ok(())
}
