// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::mission_definitions::{
    BrokerDefinition, MissionDefinitionFamily, MissionDefinitionRows, MissionDefinitionSelection,
};
use p4kx_export::catalog_queries::mission_properties::{SourcePayload, TemplateSourceRoot};
use p4kx_export::catalog_queries::{CatalogRecordSelection, ParsedCatalogQueries};
use serde_json::{Value, json};

use super::{Route, TestResult};

pub(in super::super) const DATASET: &str = "/definitions/missionBrokerEntries";

const BROKER_RECORD: u32 = 0;

pub(super) fn read(parsed: &ParsedCatalogQueries<'_>, catalog: &Value) -> TestResult<Route> {
    let selection = MissionDefinitionSelection::new(
        MissionDefinitionFamily::MissionBrokerEntries,
        CatalogRecordSelection::RecordIndex(BROKER_RECORD),
    );
    parsed.with_mission_definitions(selection, None, |snapshot| {
        let MissionDefinitionRows::MissionBrokerEntries(rows) = &snapshot.rows else {
            return Err("expected canonical Broker definitions".into());
        };
        let [broker] = rows.as_slice() else {
            return Err("expected one physical Broker definition".into());
        };
        let table = json!({"availability": snapshot.availability, "rows": rows});
        assert_eq!(
            Some(&table),
            catalog.pointer(DATASET),
            "the exact typed owner preserves the normal producer row, original arena and origins",
        );
        assert!(
            snapshot.coverage.unreadable_headers.is_empty()
                && snapshot.coverage.unreadable_bodies.is_empty()
                && snapshot.coverage.incomplete_fields.is_empty(),
            "the selected original Broker projects completely",
        );
        assert!(
            snapshot.game_texts.is_empty() && snapshot.available_languages.is_empty(),
            "the authored Locale is available as a source cell and needs no translation payload",
        );
        check_links(broker)?;
        check_title(broker)?;
        Route::queried_rows(
            DATASET,
            rows,
            "normal synthetic producer identity, arena, link, and origin parity; installed parity open",
        )
    })?
}

fn check_links(broker: &BrokerDefinition<'_>) -> TestResult {
    let header = broker.header();
    let guid = header.guid();
    assert_eq!(
        header.record_index().value(),
        Some(&BROKER_RECORD),
        "the physical Broker position"
    );
    let [flow] = broker.mission_flows() else {
        return Err("expected the original Flow declaration link".into());
    };
    let [properties] = broker.mission_properties() else {
        return Err("expected the original property declaration survey link".into());
    };
    for owner in [flow.owner(), properties.owner()] {
        let owner = owner.physical();
        assert_eq!(
            owner.record_index(),
            BROKER_RECORD,
            "links use the owning physical Record"
        );
        assert_eq!(
            owner.input_ref(),
            broker.input_ref(),
            "links refer to the same original DataForge"
        );
        assert_eq!(
            Some(&owner.record_guid().get()),
            guid.value(),
            "links retain the enclosing Record GUID"
        );
    }
    Ok(())
}

fn check_title(broker: &BrokerDefinition<'_>) -> TestResult {
    let source = broker.source();
    let field = source
        .roots()
        .iter()
        .map(TemplateSourceRoot::field)
        .find(|field| field.name().value().copied() == Some("title"))
        .ok_or("the original title field")?;
    let value = source
        .value(field.value())
        .ok_or("the original title occurrence")?
        .original()
        .value();
    assert!(
        matches!(value.value(), Some(SourcePayload::Locale { .. })),
        "the authored title remains a typed original Locale",
    );
    assert_eq!(
        value.origin().contains_encrypted_source(),
        Some(true),
        "the title key retains its encrypted DataForge source",
    );
    Ok(())
}
