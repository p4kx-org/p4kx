// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogSummary, Language};
use p4kx_extraction::{
    Archive, ArchiveWatch, DataForgeEntry, DataForgeOrigin, Filter, LocalizationEntry,
    LocalizationSet, RecordTree, is_placeholder_key,
};
use serde_json::Value;

use super::{TestResult, TextTarget, selected_catalog, selected_item, with_item_texts};
use crate::real_archive_shop_inventory_v3::verify_build;
use crate::real_archive_text_oracle::Oracle;
use crate::real_archive_verification::print_line;

#[path = "real_archive_installed_language_sources.rs"]
mod language_sources;
#[path = "real_archive_installed_item_text_origins.rs"]
mod origins;

use origins::{Lookup, TextOrigins};

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and P4KX_CATALOG_ITEM_TEXT_RECORD and pass --ignored"]
fn installed_item_texts_match_every_available_archive_language() -> TestResult<()> {
    let item_path = std::env::var("P4KX_CATALOG_ITEM_TEXT_RECORD")
        .map_err(|_| "configure the exact original Item Record path")?;
    let filter = Filter::parse(&format!("*{item_path}"))?;
    let archive = Archive::open_with_key(
        p4kx_testkit::real_archive_path()?,
        Some(p4kx_testkit::real_archive_key()?),
    )?;
    let watch = ArchiveWatch::new(&archive);
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let original = selected_item(&forge, &filter, &item_path)?;
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
        &original, &catalog, &locales, &languages, &evidence, channel,
    )?;
    watch
        .check()
        .map_err(|_| "Archive changed during all-language Item comparison")?;
    Ok(())
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
    locales: &LocalizationSet,
    languages: &[Language],
    evidence: &TextOrigins<'_>,
    channel: &str,
) -> TestResult<()> {
    with_item_texts(original, catalog, |targets| {
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
        for &language in languages {
            let counts = compare_language(targets, locales, language, evidence)?;
            print_line(&format!(
                "{channel} Item text language={}: present={} missing={} placeholder={}",
                language.code(),
                counts.present,
                counts.missing,
                counts.placeholder,
            ));
        }
        Ok(())
    })
}

#[derive(Default)]
struct Coverage {
    present: usize,
    missing: usize,
    placeholder: usize,
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
        let outcome = if placeholder {
            coverage.placeholder += 1;
            Lookup::Skipped
        } else if raw.is_some() {
            coverage.present += 1;
            Lookup::Resolved
        } else {
            coverage.missing += 1;
            Lookup::Missing
        };
        for form in ["sourceText", "displayText"] {
            let carrier = forms.get(form).ok_or("direct translation carrier")?;
            evidence.compare(carrier, locale, language, outcome)?;
        }
    }
    Ok(coverage)
}
