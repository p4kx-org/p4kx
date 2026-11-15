// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::UNIX_EPOCH;

use p4kx_export::{CatalogOptions, CatalogPublicationMode, Compression, Kind, Language};
use p4kx_extraction::{
    Archive, ArchiveWatch, DataForge, DataForgeEntry, Filter, LocalizationSet, RecordTree,
    StructValue, Value as Original, is_placeholder_key,
};
use serde_json::Value;

use super::{
    ItemPresentationIndex, TestResult, TextDomain, TextIndex, TextReferenceShape, localization,
    record_index,
};
use crate::catalog_v3_consumer;
use crate::real_archive_shop_inventory_v3::verify_build;
use crate::real_archive_text_oracle::Oracle;
use crate::real_archive_verification::print_line;

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn installed_live_item_texts_match_original_record_and_languages() -> TestResult<()> {
    let path = std::env::var("P4KX_CATALOG_ITEM_TEXT_RECORD")
        .map_err(|_| "configure the exact original Item Record path")?;
    let filter = Filter::parse(&format!("*{path}"))?;
    let archive = Archive::open_with_key(
        p4kx_testkit::real_archive_path()?,
        Some(p4kx_testkit::real_archive_key()?),
    )?;
    let watch = ArchiveWatch::new(&archive);
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let original = selected_item(&forge, &filter, &path)?;
    let options = CatalogOptions {
        filter: Some(&filter),
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..CatalogOptions::new(UNIX_EPOCH)
    };
    let (_, payload) =
        p4kx_export::write_catalog_from(&archive, &entry, &options, Compression::None, || {
            Ok(Vec::new())
        })?;
    let catalog: Value = serde_json::from_slice(&payload)?;
    drop(payload);
    if catalog.pointer("/source/channel").and_then(Value::as_str) != Some("LIVE") {
        return Err("select the installed LIVE Channel".into());
    }
    verify_build(&archive, &catalog)?;
    let locales = LocalizationSet::read(&archive, &[Language::English, Language::German])?;
    let oracles = [
        ("en", Oracle::of(&locales, Language::English)?),
        ("de", Oracle::of(&locales, Language::German)?),
    ];
    let keys = compare_item(&original, &catalog, &oracles)?;
    watch
        .check()
        .map_err(|_| "LIVE Archive changed during Item text comparison")?;
    print_line(&format!(
        "LIVE Item text witness: {keys} original keys, en/de source and display forms"
    ));
    Ok(())
}

fn selected_item<'f>(
    forge: &'f DataForge<'f>,
    filter: &Filter,
    path: &str,
) -> TestResult<RecordTree<'f>> {
    let mut accepted = forge
        .records()
        .filter(|record| record.record_path().is_ok_and(|path| filter.matches(path)));
    let record = accepted.next().ok_or("no selected Item Record")?;
    if record.record_path()? != path
        || record.type_name()? != "EntityClassDefinition"
        || accepted.next().is_some()
    {
        return Err("the exact Item path must admit one physical Entity Record".into());
    }
    Ok(record.tree()?)
}

fn compare_item(
    original: &RecordTree<'_>,
    catalog: &Value,
    oracles: &[(&str, Oracle)],
) -> TestResult<usize> {
    let [item] = catalog_v3_consumer::rows(catalog, Kind::Items)?.as_slice() else {
        return Err("the selected original Record must produce one Item".into());
    };
    let Some(Original::Array(source_components)) = original.field("Components") else {
        return Err("the selected Item must have an original Components array".into());
    };
    if record_index(item)? != u64::from(original.record_index()) {
        return Err("the Item projection selects a different physical Record".into());
    }
    let presentations = ItemPresentationIndex::new(catalog)?;
    let components = presentations.components(item)?;
    let texts = TextIndex::new(catalog)?;
    let (source, wire) = first_slot(
        source_components,
        components,
        "SAttachableComponentParams",
        "attachment",
    )?
    .ok_or("the selected Item needs an original attachment")?;
    let Some(Original::Struct(definition)) = source.field("AttachDef") else {
        return Err("the selected Item needs an authored AttachDef".into());
    };
    let Some(Original::Struct(original_locale)) = definition.field("Localization") else {
        return Err("the selected Item needs authored attachment Localization".into());
    };
    let catalog_locale = localization(wire)?.ok_or("the authored Item localization was omitted")?;
    let mut keys = 0;
    let mut german_resolved = false;
    for (field, carrier, domain) in [
        ("Name", "name", TextDomain::PublicName),
        ("Description", "description", TextDomain::GameText),
    ] {
        german_resolved |= compare_locale(
            original_locale.field(field),
            catalog_locale.get(carrier),
            &texts,
            oracles,
            domain,
        )?;
        keys += 1;
    }
    if let Some((source, wire)) = first_slot(
        source_components,
        components,
        "SCItemPurchasableParams",
        "purchasable",
    )? {
        german_resolved |= compare_locale(
            source.field("displayType"),
            wire.pointer("/value/displayType"),
            &texts,
            oracles,
            TextDomain::GameText,
        )?;
        keys += 1;
    }
    if !german_resolved {
        return Err("select an Item with at least one nonempty original German translation".into());
    }
    Ok(keys)
}

fn first_slot<'a, 'f>(
    original: &'a [Original<'f>],
    exported: &'a [Value],
    source_type: &str,
    kind: &str,
) -> TestResult<Option<(&'a StructValue<'f>, &'a Value)>> {
    let original = original.iter().enumerate().find_map(|(ordinal, value)| {
        if let Original::Struct(value) = value
            && value.type_name() == source_type
        {
            Some((ordinal, value))
        } else {
            None
        }
    });
    let exported = exported
        .iter()
        .find(|row| row.get("kind").and_then(Value::as_str) == Some(kind));
    match (original, exported) {
        (None, None) => Ok(None),
        (Some((ordinal, original)), Some(exported))
            if exported
                .pointer("/componentOrdinal/value")
                .and_then(Value::as_u64)
                == Some(u64::try_from(ordinal)?)
                && exported
                    .pointer("/sourceType/value")
                    .and_then(Value::as_str)
                    == Some(source_type) =>
        {
            Ok(Some((original, exported)))
        }
        _ => Err(format!("the first {kind} disagrees with the original Item slot").into()),
    }
}

fn compare_locale(
    original: Option<&Original<'_>>,
    carrier: Option<&Value>,
    texts: &TextIndex<'_>,
    oracles: &[(&str, Oracle)],
    domain: TextDomain,
) -> TestResult<bool> {
    let Some(Original::Locale(key)) = original else {
        return Err("select an Item with original Locale fields".into());
    };
    let target = texts
        .reference(
            carrier.ok_or("the original Locale lacks a carrier")?,
            domain,
            TextReferenceShape::Required,
        )?
        .ok_or("the original Locale lacks a text reference")?;
    if target.key != *key {
        return Err("the text reference changed the original Locale key".into());
    }
    let lookup = (!is_placeholder_key(key)).then_some(*key);
    let mut german_resolved = false;
    for (language, oracle) in oracles {
        let expected_source = lookup.and_then(|key| oracle.source_text(key));
        let expected_display = lookup.and_then(|key| oracle.canonical_text(key));
        if target.raw(language)?.as_str() != expected_source
            || target.shown(language)?.as_str() != expected_display
        {
            return Err(format!(
                "the Item Locale differs from independently read {language} texts"
            )
            .into());
        }
        german_resolved |=
            *language == "de" && expected_display.is_some_and(|text| !text.is_empty());
    }
    Ok(german_resolved)
}
