// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::sdk_full_proof as full_proof;
use p4kx_export::catalog_queries::{CatalogQuerySession, CatalogRecordSelection, ProvenanceMode};

use super::{TestResult, assert_source_uses};
use crate::commodity_v3_fixture::{self, DcbOrigin, guid};
use crate::spawns_fixture::{self, LOCATION_A};

#[test]
fn mission_location_tag_markers_distinguish_all_dataforge_origins() -> TestResult {
    for (source, expected) in [
        (DcbOrigin::Plain, Some(false)),
        (DcbOrigin::Encrypted, Some(true)),
        (DcbOrigin::Unknown, None),
    ] {
        let (_folder, archive) = commodity_v3_fixture::open(&spawns_fixture::database(), source)?;
        let marker_session = CatalogQuerySession::open(&archive, full_proof::NO_LANGUAGES, None)?;
        let full_session = full_proof::open(&archive, full_proof::NO_LANGUAGES, None)?;
        let selection = CatalogRecordSelection::Guid(guid(LOCATION_A).parse()?);
        for (session, mode) in [
            (marker_session, ProvenanceMode::MarkersOnly),
            (full_session, ProvenanceMode::Full),
        ] {
            session.with_mission_location_templates(selection, None, |snapshot| {
                let [row] = snapshot.rows.as_slice() else {
                    return Err("one original template".into());
                };
                let tag = row
                    .tags()
                    .value()
                    .and_then(|tags| tags.first())
                    .ok_or("one known original tag")?;
                let projected = tag.path()?;
                assert_eq!(
                    projected.origin().contains_encrypted_source(),
                    expected,
                    "the value independently preserves the original DCB origin"
                );
                let references = projected.origin().source_uses_refs();
                assert_eq!(
                    snapshot.source_inventory.provenance_mode(),
                    mode,
                    "location tag inventory follows the session's provenance profile"
                );
                if mode == ProvenanceMode::Full {
                    assert_source_uses(&snapshot, references);
                } else {
                    assert_eq!(
                        references,
                        [],
                        "marker-only tag path carries no source references"
                    );
                    assert!(
                        snapshot.source_inventory.iter_uses().next().is_none(),
                        "marker-only location query retains no physical source uses"
                    );
                }
                Ok::<_, Box<dyn std::error::Error>>(())
            })??;
        }
    }
    Ok(())
}

#[path = "mission_location_tag_owner_selection.rs"]
mod owner_selection;
