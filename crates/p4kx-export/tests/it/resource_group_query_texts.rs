// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::{CanonicalText, ResourceGroupDefinition, ResourceGroupSnapshot};
use p4kx_extraction::{
    Language, SourceEncryptionState, SourceInventory, SourceLocation, SourceUseRole, SourcedValue,
};
use serde::Serialize;
use serde_json::Value;

use crate::canonical_header::wire::Wire;
use crate::commodity_v3_fixture::{self as common, TestResult};

pub(super) fn verify(
    snapshot: &ResourceGroupSnapshot<'_>,
    catalog: &Value,
    marker: Option<bool>,
) -> TestResult {
    assert_eq!(
        snapshot.public_names().len(),
        1,
        "one shared public-name key is registered"
    );
    assert_eq!(
        snapshot.game_texts().len(),
        1,
        "the same protected key has its own domain row"
    );
    assert!(
        snapshot.requested_languages().contains(&Language::English),
        "English was requested"
    );
    assert!(
        snapshot.requested_languages().contains(&Language::German),
        "German was requested"
    );
    assert_eq!(
        snapshot.available_languages().len(),
        2,
        "only the two physical locale payloads are available"
    );
    assert!(
        snapshot.available_languages().contains(&Language::English),
        "the encrypted English locale was read"
    );
    assert!(
        snapshot.available_languages().contains(&Language::German),
        "the plain German locale was read"
    );
    assert!(
        snapshot.localization_notes().is_empty(),
        "synthetic locales decode cleanly"
    );
    for row in snapshot.definitions() {
        definition_texts(row, snapshot, catalog, marker)?;
    }
    Ok(())
}

fn definition_texts(
    row: &ResourceGroupDefinition<'_>,
    snapshot: &ResourceGroupSnapshot<'_>,
    catalog: &Value,
    marker: Option<bool>,
) -> TestResult {
    let name = row.name().value().ok_or("public group name identity")?;
    let description = row
        .description()
        .value()
        .ok_or("protected group description identity")?;
    assert_ne!(
        name.as_str(),
        description.as_str(),
        "equal keys in different text domains retain distinct IDs"
    );
    text(
        snapshot
            .public_name(name)
            .ok_or("query-local public name")?,
        snapshot.source_inventory(),
        catalog,
        "publicNames",
        marker,
    )?;
    text(
        snapshot
            .game_text(description)
            .ok_or("query-local protected text")?,
        snapshot.source_inventory(),
        catalog,
        "gameTexts",
        marker,
    )?;
    assert_eq!(
        row.name().origin().contains_encrypted_source(),
        marker,
        "the name-reference cell retains its actual DCB origin"
    );
    assert_eq!(
        row.description().origin().contains_encrypted_source(),
        marker,
        "protection domain does not replace input encryption"
    );
    Ok(())
}

fn text<R: Serialize>(
    row: &CanonicalText<'_, R>,
    inventory: &SourceInventory,
    catalog: &Value,
    domain: &str,
    marker: Option<bool>,
) -> TestResult {
    let id = serde_json::to_value(row.id().value().ok_or("canonical text ID")?)?;
    let expected = common::text_row(catalog, domain, &id)?;
    let wire = Wire::new(expected, inventory);
    wire.field("id")?.leaf(row.id())?;
    wire.field("key")?.leaf(row.key())?;
    assert_eq!(
        row.key().value().copied(),
        Some("@shared"),
        "the authored key token is retained"
    );
    assert_eq!(
        row.key().origin().contains_encrypted_source(),
        marker,
        "the key derives from its original DCB source"
    );
    wire.field("translations")?
        .cell(row.translations(), |translations, wire| {
            assert_eq!(
                translations.len(),
                2,
                "both available locale forms are retained"
            );
            for (language, text, encrypted) in [
                (Language::English, "Shared text", true),
                (Language::German, "Gemeinsamer Text", false),
            ] {
                let forms = translations
                    .get(language.code())
                    .ok_or("original language forms")?;
                let wire = wire.field(language.code())?;
                wire.field("sourceText")?.leaf(&forms.source_text)?;
                wire.field("displayText")?.leaf(&forms.display_text)?;
                assert_eq!(
                    forms.source_text.value().map(AsRef::as_ref),
                    Some(text),
                    "exact decoded language source is retained"
                );
                assert_eq!(
                    forms.display_text.value().map(AsRef::as_ref),
                    Some(text),
                    "the corresponding display form is retained"
                );
                for cell in [&forms.source_text, &forms.display_text] {
                    assert_eq!(
                        cell.origin().contains_encrypted_source(),
                        if encrypted { Some(true) } else { marker },
                        "each form retains its authored key selector and locale contributions"
                    );
                    contributors(inventory, cell, language, encrypted)?;
                }
            }
            Ok(())
        })
}

fn contributors<T>(
    inventory: &SourceInventory,
    cell: &SourcedValue<T>,
    language: Language,
    encrypted: bool,
) -> TestResult {
    let mut key_selector = false;
    let mut locale_payload = false;
    for reference in cell.origin().source_uses_refs() {
        let description = inventory
            .iter_uses()
            .find(|description| description.id() == reference)
            .ok_or("original text contribution in the query-local inventory")?;
        key_selector |= matches!(description.location(), SourceLocation::DataForge { .. })
            && description.role() == SourceUseRole::Selector;
        if matches!(
            description.location(),
            SourceLocation::LocalizationLookup { language: actual, .. } if *actual == language
        ) && description.role() == SourceUseRole::Payload
        {
            assert_eq!(
                description.effective_encryption(),
                if encrypted {
                    SourceEncryptionState::Encrypted
                } else {
                    SourceEncryptionState::Unencrypted
                },
                "the actual locale payload keeps its independent encryption state"
            );
            locale_payload = true;
        }
    }
    assert!(key_selector, "the authored DCB key selector contributes");
    assert!(locale_payload, "the selected language payload contributes");
    Ok(())
}
