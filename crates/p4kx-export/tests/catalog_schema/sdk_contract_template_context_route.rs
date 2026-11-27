// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::contract_template_context::{
    ContractTemplateContextSnapshot, LocalizationMeaning, TemplateDefinition, TemplateProjection,
    TemplateTagInterpretation,
};
use p4kx_export::catalog_queries::{CatalogRecordSelection, ParsedCatalogQueries};
use p4kx_extraction::{FieldOrigin, Language, SourceInventory};
use serde_json::{Value, json};

use super::{Route, TEMPLATE_ALIAS, TEMPLATE_RECORD, TestResult};

pub(in super::super) const DATASET: &str = "/configurations/contractTemplates";

pub(super) fn read(parsed: &ParsedCatalogQueries<'_>, catalog: &Value) -> TestResult<Route> {
    parsed.with_contract_template_context(CatalogRecordSelection::All, None, |snapshot| {
        let rows = &snapshot.rows;
        assert_eq!(
            Some(&json!({"availability": snapshot.availability, "rows": rows})),
            catalog.pointer(DATASET),
            "the contextual row of both physical owners matches the same normal producer"
        );
        assert_eq!(
            Some(&serde_json::to_value(&snapshot.owners)?),
            catalog.pointer("/definitions/contractTemplates/rows"),
            "context and normal output share the exact original owners"
        );
        assert!(snapshot.coverage.incomplete_fields.is_empty(), "both owners are complete");
        check_reached(catalog, &snapshot)?;
        check_alias(parsed, catalog)?;
        Route::queried_rows(DATASET, rows, "normal synthetic context, Tag ancestry and Locale origin parity; installed parity open")
    })?
}

fn check_reached(catalog: &Value, snapshot: &ContractTemplateContextSnapshot<'_>) -> TestResult {
    let [reached, alias] = snapshot.rows.as_slice() else {
        return Err("expected both physical Template rows".into());
    };
    assert_eq!(
        reached.template().record_index(),
        TEMPLATE_RECORD,
        "exact reached owner"
    );
    assert_eq!(
        alias.template().record_index(),
        TEMPLATE_ALIAS,
        "exact alias owner"
    );
    let owner = snapshot
        .owner(reached.template())
        .ok_or("physical original owner")?;
    let [tag] = reached.tag_paths() else {
        return Err("one reached Template Tag".into());
    };
    let original = owner
        .source()
        .value(tag.source())
        .ok_or("own authored Tag operand")?;
    assert_eq!(
        original
            .original()
            .value()
            .origin()
            .contains_encrypted_source(),
        Some(true),
        "the source arena retains the encrypted authored Tag operand"
    );
    assert!(
        matches!(tag.interpretation(), TemplateTagInterpretation::Path),
        "leaf ancestry resolved"
    );
    let path = tag.path()?;
    assert_eq!(
        path.origin().contains_encrypted_source(),
        Some(true),
        "Tag ancestry retains encryption"
    );
    origins(catalog, &snapshot.source_inventory, path.origin())?;
    check_locale(catalog, snapshot, reached, owner)
}

fn check_locale(
    catalog: &Value,
    snapshot: &ContractTemplateContextSnapshot<'_>,
    reached: &TemplateProjection<'_>,
    owner: &TemplateDefinition<'_>,
) -> TestResult {
    let [text] = reached.localizations() else {
        return Err("one reached Template locale".into());
    };
    assert!(
        matches!(text.interpretation(), LocalizationMeaning::Lookup),
        "actual authored Locale uses a lookup"
    );
    assert!(
        owner.source().value(text.source()).is_some(),
        "the lookup selects a value from this owner"
    );
    let id = text.text().value().ok_or("reached protected text id")?;
    let row = snapshot
        .game_texts
        .iter()
        .find(|row| row.id().value() == Some(id))
        .ok_or("returned protected text")?;
    let encoded = serde_json::to_value(row)?;
    let catalog_texts = catalog
        .pointer("/texts/gameTexts")
        .and_then(Value::as_array)
        .ok_or("producer protected text rows")?;
    assert!(
        catalog_texts.contains(&encoded),
        "selected locale text and its own origin match the producer"
    );
    assert_eq!(
        snapshot.available_languages,
        [Language::English],
        "only selected available locale"
    );
    Ok(())
}

fn check_alias(parsed: &ParsedCatalogQueries<'_>, catalog: &Value) -> TestResult {
    parsed.with_contract_template_context(
        CatalogRecordSelection::RecordIndex(TEMPLATE_ALIAS),
        None,
        |snapshot| {
            let [alias] = snapshot.rows.as_slice() else {
                return Err("exact alias projection".into());
            };
            let rows = catalog
                .pointer(DATASET)
                .and_then(|table| table.get("rows"))
                .and_then(Value::as_array)
                .ok_or("producer contexts")?;
            let expected = rows
                .iter()
                .find(|row| {
                    row.pointer("/template/recordIndex").and_then(Value::as_u64)
                        == Some(u64::from(TEMPLATE_ALIAS))
                })
                .ok_or("producer alias context")?;
            assert_eq!(
                &serde_json::to_value(alias)?,
                expected,
                "alias retains its own context and source links"
            );
            assert!(
                snapshot.owner(alias.template()).is_some(),
                "alias reuses the canonical original owner"
            );
            assert!(
                !snapshot.tag_ancestry_surveyed,
                "the null alias does not visit the Tag domain"
            );
            assert!(
                snapshot.game_texts.is_empty(),
                "the null alias registers no textual payload"
            );
            Ok(())
        },
    )?
}

fn origins(catalog: &Value, inventory: &SourceInventory, origin: &FieldOrigin) -> TestResult {
    assert!(
        origin.source_set_complete(),
        "selected Tag path has complete ancestry"
    );
    let catalog_uses = catalog
        .pointer("/provenance/uses")
        .and_then(Value::as_array)
        .ok_or("producer source use inventory")?;
    for reference in origin.source_uses_refs() {
        let row = inventory
            .iter_uses()
            .find(|row| row.id() == reference)
            .ok_or("SDK source use")?;
        assert!(
            catalog_uses.contains(&serde_json::to_value(row)?),
            "each actual Tag operand has the same original source-use details"
        );
    }
    Ok(())
}
