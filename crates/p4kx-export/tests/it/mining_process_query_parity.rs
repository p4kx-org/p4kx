// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::mining::{MiningGlobalSnapshot, RefiningProcessSnapshot};
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection,
};
use p4kx_export::{CatalogPublicationMode, build_catalog};
use serde_json::Value;

use super::{fixture, walk};
use crate::canonical_header::json::canonical_value;
use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};

#[test]
fn complete_mining_and_refining_getters_match_canonical_wire_for_all_source_states() -> TestResult {
    for (origin, marker) in [
        (DcbOrigin::Plain, Some(false)),
        (DcbOrigin::Encrypted, Some(true)),
        (DcbOrigin::Unknown, None),
    ] {
        let (_folder, archive) = common::open(&fixture::database(false), origin)?;
        let mut options = common::options();
        options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
        let built = build_catalog(&archive, &options)?;
        let catalog: Value = serde_json::from_slice(built.json())?;
        assert!(
            catalog.get("refiningProcesses").is_none(),
            "no duplicate legacy refining wire"
        );
        assert!(
            catalog.get("miningGlobalParams").is_none(),
            "no duplicate legacy global wire"
        );
        let session =
            CatalogQuerySession::open(&archive, CatalogLanguageSelection::AllAvailable, None)?;
        let parsed = session.parsed(None)?;
        parsed.with_mining_global_params(CatalogRecordSelection::All, None, |snapshot| {
            globals(&snapshot, &catalog, marker)
        })??;
        parsed.with_refining_processes(CatalogRecordSelection::All, None, |snapshot| {
            refining(&snapshot, &catalog, marker)
        })??;
    }
    Ok(())
}

fn globals(
    snapshot: &MiningGlobalSnapshot<'_>,
    catalog: &Value,
    marker: Option<bool>,
) -> TestResult {
    let expected = common::at(catalog, "/configurations/miningGlobal/rows")?
        .as_array()
        .ok_or("canonical globals")?;
    assert_eq!(
        canonical_value(snapshot.rows())?,
        *common::at(catalog, "/configurations/miningGlobal/rows")?,
        "shared canonical global rows"
    );
    assert!(
        snapshot.available_languages().is_empty(),
        "globals need no locale payload"
    );
    assert!(
        snapshot.dependency_coverage().unreadable_bodies.is_empty(),
        "no Resource-group demand"
    );
    for (row, expected) in snapshot.rows().iter().zip(expected) {
        walk::visit_global(row, snapshot.source_inventory(), expected)?;
        assert_eq!(
            row.composition_mass_multiplier()
                .origin()
                .contains_encrypted_source(),
            marker,
            "each numeric datum keeps its original marker"
        );
    }
    Ok(())
}

fn refining(
    snapshot: &RefiningProcessSnapshot<'_>,
    catalog: &Value,
    marker: Option<bool>,
) -> TestResult {
    let expected = common::at(catalog, "/definitions/refiningProcesses/rows")?
        .as_array()
        .ok_or("canonical refining rows")?;
    assert_eq!(
        canonical_value(snapshot.rows())?,
        *common::at(catalog, "/definitions/refiningProcesses/rows")?,
        "shared canonical refining rows"
    );
    assert_eq!(
        snapshot.rows().len(),
        2,
        "physical aliases remain independent"
    );
    for (row, expected) in snapshot.rows().iter().zip(expected) {
        walk::visit_refining(row, snapshot.source_inventory(), expected)?;
        assert_eq!(
            row.refining_speed().origin().contains_encrypted_source(),
            marker,
            "enum payload has its own immediate evidence"
        );
        let reference = row
            .process_name()
            .value()
            .ok_or("protected text identity")?;
        let text = snapshot
            .game_text(reference)
            .ok_or("query-local protected text row")?;
        let key = text
            .key()
            .value()
            .copied()
            .ok_or("original protected Locale key")?;
        assert!(
            matches!(key, "@process_method" | "@process_missing"),
            "the authored @ token is never normalized away"
        );
        assert_eq!(
            text.key().origin().contains_encrypted_source(),
            marker,
            "the key derives from its actual DCB input"
        );
    }
    Ok(())
}
