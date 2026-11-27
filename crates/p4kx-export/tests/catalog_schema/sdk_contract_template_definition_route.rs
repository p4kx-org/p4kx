// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::mission_definitions::{
    MissionDefinitionFamily, MissionDefinitionRows, MissionDefinitionSelection,
    MissionDefinitionSnapshot, TemplateDefinition,
};
use p4kx_export::catalog_queries::{CatalogRecordSelection, ParsedCatalogQueries};
use serde_json::{Value, json};

use super::{Route, TEMPLATE_ALIAS, TEMPLATE_RECORD, TestResult};

pub(in super::super) const DATASET: &str = "/definitions/contractTemplates";
const FAMILY: MissionDefinitionFamily = MissionDefinitionFamily::ContractTemplates;

pub(super) fn read(parsed: &ParsedCatalogQueries<'_>, catalog: &Value) -> TestResult<Route> {
    let selection = MissionDefinitionSelection::new(FAMILY, CatalogRecordSelection::All);
    parsed.with_mission_definitions(selection, None, |snapshot| {
        let rows = rows(&snapshot)?;
        let table = json!({"availability": snapshot.availability, "rows": rows});
        assert_eq!(
            Some(&table),
            catalog.pointer(DATASET),
            "the typed definitions preserve normal producer rows, source arenas, links and origins",
        );
        let [authored, empty] = rows else {
            return Err("expected two distinct Template definitions".into());
        };
        check_header(authored, TEMPLATE_RECORD)?;
        check_header(empty, TEMPLATE_ALIAS)?;
        assert!(!authored.source().values().is_empty(), "owned authored values are present");
        assert!(snapshot.available_languages.is_empty(), "the snapshot consults no Locale payloads");
        assert!(snapshot.coverage.incomplete_fields.is_empty(), "both Template owners project completely");
        Route::queried_rows(
            DATASET,
            rows,
            "normal synthetic producer identity, arena, link, and origin parity; installed parity open",
        )
    })?
}

pub(super) fn check_alias(parsed: &ParsedCatalogQueries<'_>) -> TestResult {
    let selection = MissionDefinitionSelection::new(FAMILY, CatalogRecordSelection::All);
    let expected =
        parsed.with_mission_definitions(selection, None, |snapshot| -> TestResult<Value> {
            let [first, alias] = rows(&snapshot)? else {
                return Err("both physical Templates sharing a GUID should be selected".into());
            };
            let first_guid = first.header().guid();
            assert_eq!(
                first_guid.value(),
                alias.header().guid().value(),
                "the two Templates share a GUID"
            );
            check_header(first, TEMPLATE_RECORD)?;
            check_header(alias, TEMPLATE_ALIAS)?;
            Ok(serde_json::to_value(alias)?)
        })??;
    let selection = MissionDefinitionSelection::new(
        FAMILY,
        CatalogRecordSelection::RecordIndex(TEMPLATE_ALIAS),
    );
    parsed.with_mission_definitions(selection, None, |exact| -> TestResult {
        let [selected] = rows(&exact)? else {
            return Err("selection by physical index must return only its Template".into());
        };
        assert_eq!(
            serde_json::to_value(selected)?,
            expected,
            "the physical alias retains its own header, links and arena",
        );
        Ok(())
    })?
}

fn rows<'snapshot, 'source>(
    snapshot: &'snapshot MissionDefinitionSnapshot<'source>,
) -> TestResult<&'snapshot [TemplateDefinition<'source>]> {
    let MissionDefinitionRows::ContractTemplates(rows) = &snapshot.rows else {
        return Err("expected canonical Template definitions".into());
    };
    Ok(rows)
}

fn check_header(definition: &TemplateDefinition<'_>, index: u32) -> TestResult {
    let header = definition.header();
    assert_eq!(
        header.record_index().value(),
        Some(&index),
        "the original Record position is retained"
    );
    assert_eq!(
        header.record_index().origin().contains_encrypted_source(),
        Some(true),
        "the header retains its encrypted source origin",
    );
    let [flow] = definition.mission_flows() else {
        return Err("expected one original flow declaration binding".into());
    };
    let [properties] = definition.mission_properties() else {
        return Err("expected one property declaration binding".into());
    };
    for owner in [flow.owner(), properties.owner()] {
        let owner = owner.physical();
        assert_eq!(
            owner.record_index(),
            index,
            "both bindings preserve the owning Record index"
        );
        assert_eq!(
            owner.input_ref(),
            definition.input_ref(),
            "the binding and owner share one original DataForge"
        );
    }
    Ok(())
}
