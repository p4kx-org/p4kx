// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;
use std::env;

use p4kx_assembly::ENTITY_TYPE;
use p4kx_export::catalog_queries::{CatalogLanguageSelection, CatalogQuerySession, ProvenanceMode};
use p4kx_extraction::{Archive, ArchiveWatch, DataForge, DataForgeEntry, Filter};
use p4kx_testkit::{real_archive_key, real_archive_path};

use crate::real_archive_catalog_table_parity::TestResult;

/// Opens a session for Archive tests that verify original source evidence.
pub(crate) fn full_source_session<'archive>(
    archive: &'archive Archive,
    languages: CatalogLanguageSelection<'_>,
) -> TestResult<CatalogQuerySession<'archive>> {
    Ok(CatalogQuerySession::open(archive, languages, None)?
        .with_provenance_mode(ProvenanceMode::Full))
}

/// Opens a Full proof session from this Archive's decoded Entry.
pub(crate) fn full_source_session_from<'archive>(
    archive: &'archive Archive,
    entry: &'archive DataForgeEntry,
    languages: CatalogLanguageSelection<'_>,
) -> TestResult<CatalogQuerySession<'archive>> {
    Ok(
        CatalogQuerySession::from_entry(archive, entry, languages, None)?
            .with_provenance_mode(ProvenanceMode::Full),
    )
}

pub(crate) fn with_archive(check: impl FnOnce(&Archive) -> TestResult) -> TestResult {
    let path = real_archive_path().map_err(|_| "configure the installed Archive path")?;
    let key = real_archive_key().map_err(|_| "configure the trusted installed Archive key")?;
    let archive =
        Archive::open_with_key(path, Some(key)).map_err(|_| "open the installed Archive")?;
    let expected =
        env::var("P4KX_NEW_SDK_BUILD").map_err(|_| "configure the expected Build change number")?;
    let build = archive
        .build()
        .ok_or("the installed Archive lacks a Build")?;
    if build.change_number() != expected
        || expected.is_empty()
        || !expected.bytes().all(|byte| byte.is_ascii_digit())
        || [build.branch(), build.version(), build.build_id()]
            .iter()
            .any(|value| value.trim().is_empty())
    {
        return Err("the installed Archive differs from the configured Build".into());
    }
    let watch = ArchiveWatch::new(&archive);
    watch
        .check()
        .map_err(|_| "verify the original Archive stamp")?;
    let result = check(&archive);
    watch
        .check()
        .map_err(|_| "the original Archive changed during proof")?;
    result
}

pub(super) fn dataforge(archive: &Archive) -> TestResult<DataForgeEntry> {
    DataForgeEntry::read(archive).map_err(|_| "read the bound DataForge Entry".into())
}

pub(super) fn record_index(variable: &str) -> TestResult<u32> {
    env::var(variable)
        .ok()
        .and_then(|value| value.parse().ok())
        .ok_or_else(|| "configure a private physical Ship Record index".into())
}

pub(super) fn literal_filter(input: &DataForgeEntry, index: u32) -> TestResult<Filter> {
    let forge = input.open().map_err(|_| "parse the original DataForge")?;
    let record = forge
        .record(index)
        .ok_or("the nominated physical Ship Record is missing")?;
    let path = record
        .record_path()
        .map_err(|_| "the original Ship path is unreadable")?;
    if record.type_name().ok() != Some(ENTITY_TYPE) || path.contains(['*', '?']) {
        return Err("the nominated Ship lacks a literal Entity Record path".into());
    }
    let anchor = path
        .find(|character: char| character.is_ascii_alphabetic())
        .ok_or("the original Ship path has no ASCII anchor")?;
    let mut glob = path.to_owned();
    glob.replace_range(anchor..=anchor, "?");
    let filter = Filter::parse(&glob)?;
    let exact = forge
        .records_at_path(path)
        .map(|record| record.index())
        .collect();
    if matching(&forge, &filter) != exact {
        return Err("the anchored literal Store Filter admits an unrelated Record".into());
    }
    Ok(filter)
}

pub(super) fn physical_filter(
    input: &DataForgeEntry,
    filter_variable: &str,
    records_variable: &str,
    required: &BTreeSet<u32>,
) -> TestResult<Filter> {
    let literal = env::var(filter_variable)
        .map_err(|_| "configure the private structurally anchored producer glob")?;
    let filter = Filter::parse(&literal)?;
    let declared = env::var(records_variable)
        .map_err(|_| "configure the exact approved producer Record inventory")?;
    let admitted = declared
        .split(',')
        .map(str::parse)
        .collect::<Result<BTreeSet<u32>, _>>()
        .map_err(|_| "the approved Record inventory is invalid")?;
    let forge = input.open().map_err(|_| "parse the original DataForge")?;
    if !filter.is_glob() || !required.is_subset(&admitted) || matching(&forge, &filter) != admitted
    {
        return Err("the glob admits a different physical scope or omits a reached owner".into());
    }
    Ok(filter)
}

fn matching(forge: &DataForge<'_>, filter: &Filter) -> BTreeSet<u32> {
    forge
        .records()
        .filter_map(|record| {
            record
                .record_path()
                .ok()
                .filter(|path| filter.matches(path))
                .map(|_| record.index())
        })
        .collect()
}
