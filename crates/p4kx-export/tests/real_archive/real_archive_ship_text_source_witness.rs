// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;
use std::env;
use std::path::PathBuf;

use p4kx_export::catalog_queries::equipment::{ShipSourceSnapshot, with_ship_sources_from};
use p4kx_export::catalog_queries::{
    CanonicalText, CatalogLanguageSelection, CatalogRecordSelection, DatasetAvailability, TextForms,
};
use p4kx_export::{Kind, Language};
use p4kx_extraction::{
    Archive, ArchiveWatch, DataForge, DataForgeEntry, LocalizationSet, RecordTree,
    SourceValueState, SourcedValue, StructValue, Value, is_placeholder_key,
};

use crate::real_archive_catalog_size::replay::bindings::Receipt;
use crate::real_archive_text_oracle::Oracle;
use crate::real_archive_verification::print_line;

#[path = "real_archive_ship_text_sources.rs"]
mod sources;

use self::sources::Closure;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

const LANGUAGES: [Language; 2] = [Language::English, Language::German];

struct Locale {
    language: Language,
    ordinal: u32,
    oracle: Oracle,
}

#[derive(Default)]
struct FieldStatistics {
    distinct: HashSet<String>,
    absent: usize,
    null: usize,
    placeholders: usize,
    keys: usize,
    languages: [LanguageStatistics; LANGUAGES.len()],
}

#[derive(Default)]
struct LanguageStatistics {
    known_text: usize,
    missing_key: usize,
    explicit_empty: usize,
    empty_display: usize,
}

#[test]
#[ignore = "installed LIVE: set P4KX_DATA_P4K and P4KX_CATALOG_MEASUREMENT_DIR"]
fn installed_live_ship_texts_match_original_keys_and_source_lines() -> TestResult {
    let directory = env::var_os("P4KX_CATALOG_MEASUREMENT_DIR")
        .map(PathBuf::from)
        .ok_or("configure the accepted LIVE measurement receipt")?;
    let binding = Receipt::read(&directory)?.binding()?;
    let path = p4kx_testkit::real_archive_path().map_err(|_| "configure the LIVE Archive")?;
    let key = p4kx_testkit::real_archive_key().map_err(|_| "read the configured Archive key")?;
    let archive = Archive::open_with_key(path, Some(key)).map_err(|_| "open the LIVE Archive")?;
    if p4kx_channels::channel_name(archive.path()).as_deref() != Some("LIVE") {
        return Err("the Ship text witness requires LIVE".into());
    }
    let watch = ArchiveWatch::new(&archive);
    binding.check_archive(&archive)?;
    watch.check()?;

    let entry = DataForgeEntry::read(&archive).map_err(|_| "read the original DataForge Entry")?;
    let dataforge = DataForgeEntry::locate(&archive).ok_or("original DataForge Entry")?;
    let dataforge_ordinal = u32::try_from(dataforge.index())?;
    let original = entry
        .open()
        .map_err(|_| "parse the original DataForge Entry")?;
    let locale = LocalizationSet::read(&archive, &LANGUAGES)
        .map_err(|_| "read the original English and German Entries")?;
    let locales = [
        read_locale(&locale, Language::English)?,
        read_locale(&locale, Language::German)?,
    ];
    drop(locale);

    let (rows, stats, source_uses) = with_ship_sources_from(
        &archive,
        &entry,
        CatalogRecordSelection::All,
        CatalogLanguageSelection::Selected(&LANGUAGES),
        None,
        |snapshot| inspect(&archive, &original, dataforge_ordinal, &locales, &snapshot),
    )
    .map_err(|_| "complete the scoped Ship text SDK read")??;
    watch.check()?;
    binding.check_archive(&archive)?;

    let build = archive.build().ok_or("verified LIVE Build")?;
    report_witness(build.change_number(), rows, stats, source_uses, &locales);
    Ok(())
}

fn report_witness(
    change_number: &str,
    rows: usize,
    stats: [FieldStatistics; 2],
    source_uses: usize,
    locales: &[Locale; LANGUAGES.len()],
) {
    print_line(&format!(
        "Ship text witness: LIVE change={change_number} rows={rows} contributingUses={source_uses} contradictions=0"
    ));
    for (field, stats) in ["name", "description"].into_iter().zip(stats) {
        print_line(&format!(
            "Ship {field}: occurrences={} distinct={} absent={} null={} placeholders={}",
            stats.keys,
            stats.distinct.len(),
            stats.absent,
            stats.null,
            stats.placeholders,
        ));
        for (locale, stats) in locales.iter().zip(stats.languages) {
            print_line(&format!(
                "Ship {field} {}: knownText={} missingKey={} explicitEmpty={} emptyDisplay={}",
                locale.language.code(),
                stats.known_text,
                stats.missing_key,
                stats.explicit_empty,
                stats.empty_display,
            ));
        }
    }
}

fn read_locale(locales: &LocalizationSet, language: Language) -> TestResult<Locale> {
    let entry = locales.entry(language).ok_or("missing original locale")?;
    if entry.text().is_lossy() {
        return Err("the original locale has lossy decoding".into());
    }
    let origin = entry
        .origin()
        .ok_or("missing original locale physical identity")?;
    Ok(Locale {
        language,
        ordinal: origin.entry().ordinal(),
        oracle: Oracle::of(locales, language)?,
    })
}

fn inspect(
    archive: &Archive,
    original: &DataForge<'_>,
    dataforge_ordinal: u32,
    locales: &[Locale; LANGUAGES.len()],
    snapshot: &ShipSourceSnapshot<'_>,
) -> TestResult<(usize, [FieldStatistics; 2], usize)> {
    verify_query_selection(archive, snapshot)?;
    let mut selected = physical_ships(original)?;
    let mut stats = [FieldStatistics::default(), FieldStatistics::default()];
    let mut closure = Closure::default();
    for row in snapshot.rows() {
        let index = *row
            .source()
            .record_index()
            .value()
            .ok_or("Ship physical index")?;
        if !selected.remove(&index) {
            return Err("the Ship query returned a foreign or duplicated physical row".into());
        }
        let record = original.record(index).ok_or("original Ship Record")?;
        let tree = record
            .tree()
            .map_err(|_| "read a selected original Ship body")?;
        let component = vehicle_component(&tree);
        let authored = |field| component.and_then(|fields| fields.field(field));
        closure.owners.extend([
            (index, row.name().origin()),
            (index, row.description().origin()),
        ]);
        check_field(
            authored("vehicleName"),
            row.name(),
            row.name()
                .value()
                .and_then(|reference| snapshot.public_name(reference)),
            locales,
            &mut stats[0],
            &mut closure,
        )?;
        check_field(
            authored("vehicleDescription"),
            row.description(),
            row.description()
                .value()
                .and_then(|reference| snapshot.game_text(reference)),
            locales,
            &mut stats[1],
            &mut closure,
        )?;
    }
    if !selected.is_empty() || stats.iter().any(|field| field.keys == 0) {
        return Err(
            "the original Ship selection or keyed text population does not reconcile".into(),
        );
    }
    let uses = closure.verify(snapshot.source_inventory(), dataforge_ordinal)?;
    Ok((snapshot.rows().len(), stats, uses))
}

fn vehicle_component<'tree, 'source>(
    tree: &'tree RecordTree<'source>,
) -> Option<&'tree StructValue<'source>> {
    let Value::Array(components) = tree.field("Components")? else {
        return None;
    };
    components.iter().find_map(|component| match component {
        Value::Struct(fields) if fields.type_name() == "VehicleComponentParams" => Some(fields),
        _ => None,
    })
}

fn verify_query_selection(archive: &Archive, snapshot: &ShipSourceSnapshot<'_>) -> TestResult {
    let coverage = snapshot.coverage();
    if snapshot.build() != archive.build()
        || !matches!(snapshot.availability(), DatasetAvailability::Available)
        || snapshot.requested_languages() != LANGUAGES.as_slice()
        || snapshot.available_languages() != LANGUAGES.as_slice()
        || !coverage.unreadable_headers.is_empty()
        || !coverage.unreadable_bodies.is_empty()
    {
        return Err(format!(
            "Ship selection does not match LIVE: build={}, availability={:?}, requested={}, available={}, unreadableHeaders={}, unreadableBodies={}",
            snapshot.build() == archive.build(),
            snapshot.availability(),
            snapshot.requested_languages().len(),
            snapshot.available_languages().len(),
            coverage.unreadable_headers.len(),
            coverage.unreadable_bodies.len(),
        )
        .into());
    }
    print_line(&format!(
        "Ship query reported {} rows with incomplete field evidence across all Ship fields",
        coverage.incomplete_fields.len()
    ));
    Ok(())
}

fn physical_ships(original: &DataForge<'_>) -> TestResult<HashSet<u32>> {
    let rule = Kind::Ships.rule().ok_or("original Ship record selection")?;
    let mut indices = HashSet::new();
    for record in original.records() {
        let path = record
            .record_path()
            .map_err(|_| "read original Record path")?;
        if rule.matches_path(path)
            && record.type_name().map_err(|_| "read original Ship type")? == rule.type_name
            && !p4kx_export::is_kind_leftover(
                Kind::Ships,
                path,
                record.name().map_err(|_| "read original Ship name")?,
            )
        {
            indices.insert(record.index());
        }
    }
    Ok(indices)
}

fn check_field<'query, R>(
    authored: Option<&Value<'_>>,
    cell: &'query SourcedValue<R>,
    target: Option<&'query CanonicalText<'_, R>>,
    locales: &[Locale; LANGUAGES.len()],
    statistics: &mut FieldStatistics,
    closure: &mut Closure<'query>,
) -> TestResult {
    closure.retain(cell.origin());
    let key = match authored {
        None | Some(Value::Locale("")) if cell.state() == SourceValueState::Absent => {
            statistics.absent += 1;
            return Ok(());
        }
        Some(Value::Null) if cell.state() == SourceValueState::Null => {
            statistics.null += 1;
            return Ok(());
        }
        Some(Value::Locale(key))
            if !key.is_empty() && cell.state() == SourceValueState::Present =>
        {
            *key
        }
        _ => return Err("Ship text reference disagrees with its original Locale state".into()),
    };
    let target = target.ok_or("missing canonical text target for the original Ship key")?;
    if target.key().value().copied() != Some(key) {
        return Err("the resolved Ship key differs from the original Locale declaration".into());
    }
    for origin in [
        target.id().origin(),
        target.key().origin(),
        target.translations().origin(),
    ] {
        closure.retain(origin);
    }
    let forms = target
        .translations()
        .value()
        .ok_or("missing canonical Ship language forms")?;
    if forms.len() != locales.len() {
        return Err("the Ship target differs from the selected original languages".into());
    }
    let lookup_key = if is_placeholder_key(key) {
        statistics.placeholders += 1;
        None
    } else {
        statistics.keys += 1;
        statistics
            .distinct
            .insert(key.strip_prefix('@').unwrap_or(key).to_ascii_lowercase());
        Some(key)
    };
    for (locale, counts) in locales.iter().zip(&mut statistics.languages) {
        let forms = forms
            .get(locale.language.code())
            .ok_or("missing Ship language form")?;
        check_translation(lookup_key, locale, forms, counts, closure)?;
    }
    Ok(())
}

fn check_translation<'query>(
    lookup_key: Option<&str>,
    locale: &Locale,
    forms: &'query TextForms<'_>,
    counts: &mut LanguageStatistics,
    closure: &mut Closure<'query>,
) -> TestResult {
    let expected = lookup_key.map_or((None, None), |key| {
        (
            locale.oracle.source_text(key),
            locale.oracle.canonical_text(key),
        )
    });
    if !matches_form(&forms.source_text, expected.0)
        || !matches_form(&forms.display_text, expected.1)
    {
        return Err(format!(
            "Ship text contradicts the original {} INI",
            locale.language.code()
        )
        .into());
    }
    for origin in [forms.source_text.origin(), forms.display_text.origin()] {
        closure.retain(origin);
        if lookup_key.is_some() {
            closure
                .lookups
                .push((locale.language, locale.ordinal, origin));
        }
    }
    if lookup_key.is_some() {
        match expected {
            (None, None) => counts.missing_key += 1,
            (Some(""), Some("")) => counts.explicit_empty += 1,
            (Some(_), Some("")) => counts.empty_display += 1,
            (Some(_), Some(_)) => counts.known_text += 1,
            _ => return Err("inconsistent original INI forms".into()),
        }
    }
    Ok(())
}

fn matches_form(cell: &SourcedValue<impl AsRef<str>>, expected: Option<&str>) -> bool {
    let state = if expected.is_some() {
        SourceValueState::Present
    } else {
        SourceValueState::Absent
    };
    cell.state() == state && cell.value().map(AsRef::as_ref) == expected
}
