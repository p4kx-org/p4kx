// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_extraction::model::{LodSelection, companion_file_names};
use p4kx_extraction::{CancelToken, ConversionError, Entry, ModelSource, PathIndex};

use super::{Event, MODEL_FILE_LIMIT, finish_model};
use crate::EntryIndex;
use crate::model::describe;

pub(super) fn load(index: &EntryIndex, id: u32, stop: &CancelToken, send: &impl Fn(Event)) {
    let Some(entry) = index.entry(id) else {
        return send(Event::Failed(None));
    };
    if index.watch().check().is_err() {
        return send(Event::ArchiveChanged);
    }
    let Some(paths) = index.path_index() else {
        return send(Event::Failed(None));
    };
    if let Some(size) = oversized_file(&paths, entry) {
        return send(Event::FileTooLarge(size));
    }
    if stop.is_cancelled() {
        return;
    }
    let source = match read_archive_source(&paths, entry.path()) {
        Ok(source) => source,
        Err(error) => {
            return send(if index.watch().check().is_err() {
                Event::ArchiveChanged
            } else {
                Event::Failed(Some(reason(&error, entry.path())))
            });
        }
    };
    if stop.is_cancelled() {
        return;
    }
    finish_model(index, &paths, entry.path(), &source, stop, send);
}

fn read_archive_source(index: &PathIndex<'_>, path: &str) -> Result<ModelSource, ConversionError> {
    let read =
        |lods| ModelSource::from_index_with_drawn_material_summary_origins(index, path, lods);
    read(LodSelection::All).or_else(|_| read(LodSelection::Only(0)))
}

fn oversized_file(paths: &PathIndex<'_>, entry: &Entry) -> Option<u64> {
    let path = entry.path();
    companion_file_names(path)
        .iter()
        .filter_map(|companion| paths.find(companion))
        .chain(Some(entry))
        .map(Entry::uncompressed_size)
        .find(|&size| size > MODEL_FILE_LIMIT)
}

fn reason(error: &dyn Error, path: &str) -> String {
    let described = describe(error);
    described
        .strip_prefix(path)
        .and_then(|rest| rest.strip_prefix(": "))
        .map_or_else(|| described.clone(), str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::reason;

    #[derive(Debug, thiserror::Error)]
    #[error("{0}")]
    struct Named(String);

    #[test]
    fn the_reason_does_not_repeat_the_path_the_message_names() {
        let error = Named("Data\\Objects\\a.cgf: not a chunk file".to_owned());

        assert_eq!(
            reason(&error, "Data\\Objects\\a.cgf"),
            "not a chunk file",
            "path dropped"
        );
        assert_eq!(
            reason(&error, "Data\\Objects\\b.cgf"),
            "Data\\Objects\\a.cgf: not a chunk file",
            "another path stays"
        );
    }
}
