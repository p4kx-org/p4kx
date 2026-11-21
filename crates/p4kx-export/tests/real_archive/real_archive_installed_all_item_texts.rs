// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;
use std::env::{self, VarError};

use p4kx_export::{CatalogSummary, Kind, KindTextSummary, Language, TextOwner, is_kind_leftover};
use p4kx_extraction::{
    Archive, ArchiveWatch, DataForge, DataForgeEntry, DataForgeOrigin, Filter, LocalizationEntry,
    LocalizationSet, RecordTree, StructValue, Value as Original, is_placeholder_key,
};
use serde_json::Value;

use super::{
    TestResult, TextTarget, selected_catalog, selected_item, selected_record, with_item_texts,
};
use crate::real_archive_shop_inventory_v3::verify_build;
use crate::real_archive_text_oracle::Oracle;
use crate::real_archive_verification::print_line;

#[path = "real_archive_installed_language_sources.rs"]
mod language_sources;
#[path = "real_archive_installed_item_text_origins.rs"]
mod origins;

use origins::{Lookup, TextOrigins};

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn installed_item_texts_match_every_available_archive_language() -> TestResult<()> {
    let archive = Archive::open_with_key(
        p4kx_testkit::real_archive_path()?,
        Some(p4kx_testkit::real_archive_key()?),
    )?;
    let watch = ArchiveWatch::new(&archive);
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let Selection { filter, original } = select(&forge)?;
    let (summary, catalog) = selected_catalog(&archive, &entry, &filter)?;
    let channel = catalog
        .pointer("/source/channel")
        .and_then(Value::as_str)
        .filter(|channel| matches!(*channel, "LIVE" | "PTU"))
        .ok_or("select the installed LIVE or PTU Channel")?;
    verify_build(&archive, &catalog)?;
    let DataForgeOrigin::Archive(dataforge) = entry.origin() else {
        return Err("expected the physical DataForge Entry".into());
    };
    let evidence = TextOrigins::new(&catalog, dataforge, original.record_index())?;
    let locales = LocalizationSet::read_all(&archive)?;
    let languages = original_languages(&archive, &locales, &summary)?;
    language_sources::verify(&catalog, &locales, &evidence, archive.entries().len())?;
    compare_item(
        &original, &catalog, &summary, &locales, &languages, &evidence, channel,
    )?;
    watch
        .check()
        .map_err(|_| "Archive changed during all-language Item comparison")?;
    Ok(())
}

struct Selection<'source> {
    filter: Filter,
    original: RecordTree<'source>,
}

fn select<'source>(forge: &'source DataForge<'source>) -> TestResult<Selection<'source>> {
    match env::var("P4KX_CATALOG_ITEM_TEXT_RECORD") {
        Ok(path) => {
            let filter = Filter::parse(&format!("*{path}"))?;
            let original = selected_item(forge, &filter, &path)?;
            Ok(Selection { filter, original })
        }
        Err(VarError::NotPresent) => original_selection(forge),
        Err(error) => Err(error.into()),
    }
}

fn original_selection<'source>(
    forge: &'source DataForge<'source>,
) -> TestResult<Selection<'source>> {
    let rule = Kind::Items
        .rule()
        .ok_or("the Item admission rule is required")?;
    for record in forge.records() {
        let (Ok(path), Ok(name), Ok(source_type)) =
            (record.record_path(), record.name(), record.type_name())
        else {
            continue;
        };
        if source_type != rule.type_name
            || !rule.matches_path(path)
            || is_kind_leftover(Kind::Items, path, name)
        {
            continue;
        }
        let Ok(original) = record.tree() else {
            continue;
        };
        if !has_original_texts(&original) {
            continue;
        }
        let filter = Filter::parse(&format!("*{path}"))?;
        if selected_record(forge, &filter, path).is_ok() {
            return Ok(Selection { filter, original });
        }
    }
    Err("no unique Item Record with the original text fields required by the witness".into())
}

fn has_original_texts(original: &RecordTree<'_>) -> bool {
    let Some(Original::Array(components)) = original.field("Components") else {
        return false;
    };
    let Some(attachment) = unique_component(components, "SAttachableComponentParams") else {
        return false;
    };
    let Some(purchasable) = unique_component(components, "SCItemPurchasableParams") else {
        return false;
    };
    let Some(Original::Struct(definition)) = attachment.field("AttachDef") else {
        return false;
    };
    let Some(Original::Struct(locale)) = definition.field("Localization") else {
        return false;
    };
    let has_key = |value: Option<&Original<'_>>| matches!(value, Some(Original::Locale(key)) if !key.is_empty() && !is_placeholder_key(key));
    has_key(locale.field("Name"))
        && has_key(locale.field("Description"))
        && has_key(purchasable.field("displayType"))
        && purchasable
            .field("displayName")
            .is_none_or(|value| matches!(value, Original::Locale(_)))
}

fn unique_component<'record, 'source>(
    components: &'record [Original<'source>],
    source_type: &str,
) -> Option<&'record StructValue<'source>> {
    let mut matches = components.iter().filter_map(|value| match value {
        Original::Struct(component) if component.type_name() == source_type => Some(component),
        _ => None,
    });
    matches.next().filter(|_| matches.next().is_none())
}

fn original_languages(
    archive: &Archive,
    locales: &LocalizationSet,
    summary: &CatalogSummary,
) -> TestResult<Vec<Language>> {
    let mut languages = Vec::new();
    for language in Language::ALL {
        let expected = LocalizationEntry::locate(archive, language);
        if expected.is_some() != summary.text(language).is_some_and(|row| row.available) {
            return Err(
                format!("{language}: the catalog summary omits or invents an Entry").into(),
            );
        }
        let observed = locales.entry(language);
        if let Some(original) = expected {
            let read = observed.ok_or("the physical language Entry was not read")?;
            let origin = read
                .origin()
                .ok_or("the locale read lacks a physical Entry binding")?;
            let admitted = archive
                .entry_origin(original)
                .ok_or("missing directory Entry binding")?;
            if origin.binding() != admitted.binding() {
                return Err(
                    format!("{language}: the resolver read a different physical Entry").into(),
                );
            }
            languages.push(language);
        } else if observed.is_some() {
            return Err(format!("{language}: the resolver invented an absent language").into());
        }
    }
    if languages.is_empty() {
        return Err("the installed Archive has no supported language Entry".into());
    }
    Ok(languages)
}

fn compare_item(
    original: &RecordTree<'_>,
    catalog: &Value,
    summary: &CatalogSummary,
    locales: &LocalizationSet,
    languages: &[Language],
    evidence: &TextOrigins<'_>,
    channel: &str,
) -> TestResult<()> {
    let registrations = item_registrations(original)?;
    let mut reported = BTreeMap::new();
    for row in summary
        .text_kind_coverage()
        .iter()
        .filter(|row| row.owner == TextOwner::Catalog(Kind::Items))
    {
        if reported.insert(row.language, row).is_some() {
            return Err("duplicate Item coverage for a supported language".into());
        }
    }
    if reported.len() != Language::ALL.len() {
        return Err("Item coverage must contain every supported language".into());
    }
    with_item_texts(original, catalog, |targets| {
        if targets.len() != registrations {
            return Err("the physical Item Locale registration count differs".into());
        }
        for target in targets {
            if target.translations.len() != languages.len()
                || languages
                    .iter()
                    .any(|language| !target.translations.contains_key(language.code()))
            {
                return Err(
                    "the canonical language-code map differs from the physical language Entries"
                        .into(),
                );
            }
        }
        for language in Language::ALL {
            let counts = if languages.contains(&language) {
                compare_language(targets, locales, language, evidence)?
            } else {
                Coverage::without_language(targets)
            };
            let reported = reported.get(&language).ok_or("reported Item coverage")?;
            counts.verify(reported)?;
            print_line(&format!(
                "{channel} Item text language={}: resolved={} missingLanguage={} missingKey={} emptySource={} emptyDisplay={} placeholder={}",
                language.code(),
                counts.resolved,
                counts.missing_language,
                counts.missing_key,
                counts.empty_source,
                counts.empty_display,
                counts.placeholder,
            ));
        }
        Ok(())
    })
}

fn item_registrations(original: &RecordTree<'_>) -> TestResult<usize> {
    let Some(Original::Array(components)) = original.field("Components") else {
        return Err("the selected original Item must have a Components array".into());
    };
    let mut registrations = 0;
    for (required, fields) in [
        ("SAttachableComponentParams", 2),
        ("SCItemPurchasableParams", 1),
    ] {
        let component = unique_component(components, required)
            .ok_or_else(|| format!("select an Item with exactly one physical {required}"))?;
        registrations += fields;
        if required == "SCItemPurchasableParams"
            && matches!(component.field("displayName"), Some(Original::Locale(_)))
        {
            registrations += 1;
        }
    }
    Ok(registrations)
}

#[derive(Debug, Default, PartialEq, Eq)]
struct Coverage {
    resolved: usize,
    missing_language: usize,
    missing_key: usize,
    empty_source: usize,
    empty_display: usize,
    placeholder: usize,
}

impl Coverage {
    fn without_language(targets: &[&TextTarget<'_>]) -> Self {
        let placeholder = targets
            .iter()
            .filter(|target| is_placeholder_key(target.key))
            .count();
        Self {
            missing_language: targets.len() - placeholder,
            placeholder,
            ..Self::default()
        }
    }

    fn verify(&self, actual: &KindTextSummary) -> TestResult<()> {
        let reported = Self {
            resolved: actual.resolved,
            missing_language: actual.missing_language,
            missing_key: actual.missing_key,
            empty_source: actual.empty_source,
            empty_display: actual.empty_display,
            placeholder: actual.placeholder,
        };
        if *self != reported {
            return Err(format!(
                "{}: Item coverage differs from the original Locale registrations, expected {self:?}, reported {reported:?}",
                actual.language,
            )
            .into());
        }
        Ok(())
    }
}

fn compare_language(
    targets: &[&TextTarget<'_>],
    locales: &LocalizationSet,
    language: Language,
    evidence: &TextOrigins<'_>,
) -> TestResult<Coverage> {
    let locale = locales
        .entry(language)
        .and_then(LocalizationEntry::origin)
        .ok_or("original locale Entry")?;
    let oracle = Oracle::of(locales, language)?;
    let code = language.code();
    let mut coverage = Coverage::default();
    for target in targets {
        let placeholder = is_placeholder_key(target.key);
        let lookup = (!placeholder).then_some(target.key);
        let raw = lookup.and_then(|key| oracle.source_text(key));
        let display = lookup.and_then(|key| oracle.canonical_text(key));
        if target.raw(code)?.as_str() != raw || target.shown(code)?.as_str() != display {
            return Err(format!("{language}: the Item Locale differs from the independently read source or display value").into());
        }
        let forms = target
            .translations
            .get(code)
            .ok_or("actual language forms")?;
        let outcome = match (placeholder, raw, display) {
            (true, _, _) => {
                coverage.placeholder += 1;
                Lookup::Skipped
            }
            (false, None, None) => {
                coverage.missing_key += 1;
                Lookup::Missing
            }
            (false, Some(""), Some("")) => {
                coverage.empty_source += 1;
                Lookup::Resolved
            }
            (false, Some(_), Some("")) => {
                coverage.empty_display += 1;
                Lookup::Resolved
            }
            (false, Some(_), Some(_)) => {
                coverage.resolved += 1;
                Lookup::Resolved
            }
            _ => return Err("original source and display text disagree on lookup presence".into()),
        };
        for form in ["sourceText", "displayText"] {
            let carrier = forms.get(form).ok_or("direct translation carrier")?;
            evidence.compare(carrier, locale, language, outcome)?;
        }
    }
    Ok(coverage)
}
