// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, CancelToken, Entry, PathIndex, PathIndexBuildError, PathKeys};
use p4kx_extraction::{SourceValue, SourceValueState, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use crate::CatalogError;
use crate::catalog_read::original::{EntryCapture, PhysicalEntryRef};
use crate::catalog_read::parsed_entry_json;
use crate::entry_source::EntrySource;
use crate::schema_fields::every_field_required;
use crate::selection::starts_with_folder;

/// Resolution of a declared Archive-relative path against directory evidence.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(extend("x-p4kx-vocabularyClass" = "control"))]
pub(crate) enum LooseArchivePathState {
    Absent,
    Null,
    Empty,
    Invalid,
    Unreadable,
    Unavailable,
    Missing,
    WrongFamily,
    Ambiguous,
    Filtered,
    Resolved,
}

/// A directory-only link and its source-bound outcome.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct LooseArchivePath {
    pub(crate) state: SourcedValue<LooseArchivePathState>,
    targets: Vec<SourcedValue<PhysicalEntryRef>>,
}

pub(crate) fn from_field(
    path: &SourcedValue<String>,
    capture: &EntryCapture<'_>,
) -> Result<LooseArchivePath, CatalogError> {
    let state = match (path.state(), path.value()) {
        (SourceValueState::Absent, _) => LooseArchivePathState::Absent,
        (SourceValueState::Null, _) => LooseArchivePathState::Null,
        (SourceValueState::Unreadable, _) => LooseArchivePathState::Unreadable,
        (_, Some(value)) if value.is_empty() => LooseArchivePathState::Empty,
        _ => LooseArchivePathState::Invalid,
    };
    Ok(LooseArchivePath {
        state: capture
            .source_uses()
            .value_with_origins(SourceValue::Present(state), &[path.origin()])?,
        targets: Vec::new(),
    })
}

pub(crate) fn valid(path: &str) -> bool {
    !path.is_empty()
        && !path
            .chars()
            .any(|ch| ch.is_control() || matches!(ch, ':' | '*' | '?' | '<' | '>' | '|'))
        && !path
            .split(['/', '\\'])
            .any(|part| matches!(part, "" | "." | ".."))
}

pub(crate) fn keys(
    archive: &Archive,
    cancel: Option<&CancelToken>,
) -> Result<PathKeys, CatalogError> {
    PathKeys::try_new_with(archive, |_| parsed_entry_json::check_cancel(cancel))
        .map_err(index_error)
}

pub(crate) fn resolve(
    index: &PathIndex<'_>,
    capture: &EntryCapture<'_>,
    cancel: Option<&CancelToken>,
    path: &SourcedValue<String>,
    accepted: impl Fn(&Entry) -> bool,
) -> Result<LooseArchivePath, CatalogError> {
    let Some(authored) = path.value().filter(|path| valid(path)) else {
        return from_field(path, capture);
    };
    let mut targets = Vec::new();
    let mut wrong_family = false;
    let lookup = |wanted: &str,
                  targets: &mut Vec<SourcedValue<PhysicalEntryRef>>,
                  wrong_family: &mut bool| {
        let matches = index
            .matching_entries_with(wanted, |_| parsed_entry_json::check_cancel(cancel))
            .map_err(index_error)?;
        for candidate in matches {
            let entry = candidate.map_err(index_error)?;
            if !accepted(entry) {
                *wrong_family = true;
                continue;
            }
            let source = EntrySource::outer(capture, index.archive(), entry)?;
            let target = source
                .receipt()
                .physical_reference(capture)?
                .into_cell(capture.source_uses());
            parsed_entry_json::push(targets, target)?;
        }
        Ok::<_, CatalogError>(())
    };
    lookup(authored, &mut targets, &mut wrong_family)?;
    if !starts_with_folder(authored, "data/") {
        let game_relative = format!("Data/{authored}");
        lookup(&game_relative, &mut targets, &mut wrong_family)?;
    }
    let state = match targets.len() {
        0 if wrong_family => LooseArchivePathState::WrongFamily,
        0 if crate::export_archive_health::inventory(index.archive()).is_some() => {
            LooseArchivePathState::Unavailable
        }
        0 => LooseArchivePathState::Missing,
        1 => LooseArchivePathState::Resolved,
        _ => LooseArchivePathState::Ambiguous,
    };
    let sources = capture.source_uses();
    let directory = sources.observe_root_directory_paths(index.archive())?;
    let directory_origin = sources.origin(
        directory.as_slice(),
        directory.is_some() && !matches!(state, LooseArchivePathState::Unavailable),
    );
    let mut origins = vec![path.origin(), &directory_origin];
    origins.extend(targets.iter().map(SourcedValue::origin));
    let state = sources.value_with_origins(SourceValue::Present(state), &origins)?;
    Ok(LooseArchivePath { state, targets })
}

fn index_error(error: PathIndexBuildError<CatalogError>) -> CatalogError {
    match error.into_cause() {
        Ok(error) => error,
        Err(error) => CatalogError::ArchivePathIndex(error),
    }
}
