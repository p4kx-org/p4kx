// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::Read;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver};
use std::thread;

use p4kx_extraction::Entry;

use super::{
    EntryPreviewProvenance, Loaded, PREVIEW_READ_LIMIT, PreviewContents, ReadResult, Unread,
    XmlPreview, is_cryxml,
};
use crate::EntryIndex;
use crate::model::describe;

pub(super) enum Spawned {
    Running(Receiver<ReadResult>),
    Finished(Box<ReadResult>),
}

pub(super) fn spawn(index: &Arc<EntryIndex>, id: u32, wake: impl Fn() + Send + 'static) -> Spawned {
    let (sender, result) = mpsc::channel();
    let thread_index = Arc::clone(index);
    let spawned = thread::Builder::new()
        .name("preview".to_owned())
        .spawn(move || {
            read(
                &thread_index,
                id,
                |loaded| sender.send(loaded).is_ok(),
                &wake,
            );
        });
    match spawned {
        Ok(_handle) => Spawned::Running(result),
        Err(_ignored) => {
            let mut finished = None;
            read(
                index,
                id,
                |loaded| {
                    finished = Some(loaded);
                    true
                },
                &|| {},
            );
            Spawned::Finished(Box::new(finished.unwrap_or(Err(Unread::Failed(None)))))
        }
    }
}

/// Reads the start of the Entry `id` and hands it to `deliver`, which returns whether the
/// preview still wants it; converts a `CryXml` Entry afterwards. Calls `wake` after each result.
pub(super) fn read(
    index: &EntryIndex,
    id: u32,
    deliver: impl FnOnce(ReadResult) -> bool,
    wake: &dyn Fn(),
) {
    let (Some(archive), Some(entry)) = (index.archive(), index.entry(id)) else {
        if deliver(Err(Unread::Failed(None))) {
            wake();
        }
        return;
    };
    let mut reader = match archive.open_entry(entry) {
        Ok(reader) => reader,
        Err(error) => {
            let unread = if index.watch().check().is_err() {
                Unread::ArchiveChanged
            } else if error.is_key_required() {
                Unread::KeyRequired
            } else if error.is_wrong_key() {
                Unread::KeyRejected
            } else {
                Unread::Failed(Some(describe(&error)))
            };
            if deliver(Err(unread)) {
                wake();
            }
            return;
        }
    };
    let bytes = read_start(&mut reader, entry);
    let read = if index.watch().check().is_err() {
        Err(Unread::ArchiveChanged)
    } else {
        bytes.map_err(|reason| Unread::Failed(Some(reason)))
    };
    let (read, job) = match read {
        Ok(bytes) => {
            let (xml, job) = is_cryxml(&bytes).then(XmlPreview::pending).unzip();
            let start = job.as_ref().map(|_| bytes.clone());
            let provenance =
                EntryPreviewProvenance::capture(archive.observed_entry(entry), bytes.len() as u64);
            let contents = PreviewContents::new(bytes, entry.uncompressed_size(), provenance);
            (Ok(Loaded { contents, xml }), job.zip(start))
        }
        Err(unread) => (Err(unread), None),
    };
    if !deliver(read) {
        return;
    }
    wake();
    if let Some((job, start)) = job {
        job.run(
            index,
            (
                &start,
                entry.uncompressed_size(),
                archive.observed_entry(entry),
            ),
            &mut reader,
            wake,
        );
    }
}

fn read_start(reader: &mut impl Read, entry: &Entry) -> Result<Vec<u8>, String> {
    let limit = u64::try_from(PREVIEW_READ_LIMIT).unwrap_or(u64::MAX);
    let capacity = usize::try_from(entry.uncompressed_size().min(limit)).unwrap_or(0);
    let mut bytes = Vec::with_capacity(capacity);
    reader
        .by_ref()
        .take(limit)
        .read_to_end(&mut bytes)
        .map_err(|error| describe(&error))?;
    Ok(bytes)
}
