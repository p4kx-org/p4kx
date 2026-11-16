// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::PathBuf;

use p4kx_extraction::Entry;

use crate::directory_lookup::DirectoryLookup;
use crate::retained_source::{self, PayloadVerification};
use crate::source_selection::EntrySelection;
use crate::{MediaError, MediaKind, MediaWriter, Source, error, source};

pub(crate) fn retain(
    lookup: &DirectoryLookup<'_>,
    entry: &Entry,
    writer: &mut MediaWriter<'_>,
) -> Result<(Source, PathBuf), MediaError> {
    let state = lookup.input_state()?;
    let declared = source::entry(lookup.root, lookup.index.archive, entry, lookup.chain)?;
    let cached = state.begin(entry.index(), writer)?;
    let first = cached.is_none();
    let retained = if let Some(retained) = cached {
        let payload = writer.root.join(&retained.raw.path);
        Ok((retained, payload))
    } else {
        let verification = retention_verification(lookup, entry, writer);
        state
            .budget
            .bytes(entry.uncompressed_size(), writer.limits())
            .and_then(|()| {
                retained_source::retain(lookup.index.archive, entry, verification, writer)
            })
    };
    let mut completed = None;
    let result = match retained {
        Err(cause) => Err(error::at(&declared, cause)),
        Ok((retained, payload)) => {
            let mut source = declared;
            source::retained(&mut source, entry, retained.raw.clone());
            let publication = if first {
                writer
                    .origins
                    .input(lookup.index.archive, entry)
                    .and_then(|origin| writer.retained_with_input(&source, &origin))
            } else {
                Ok(())
            };
            match publication {
                Ok(()) => {
                    completed = first.then_some(retained);
                    Ok((source, payload))
                }
                Err(cause) => Err(error::at(&source, cause)),
            }
        }
    };
    error::finish(result, state.end(entry.index(), completed))
}

fn retention_verification(
    lookup: &DirectoryLookup<'_>,
    entry: &Entry,
    writer: &MediaWriter<'_>,
) -> PayloadVerification {
    if lookup.chain.is_empty()
        && writer.animation_entity.is_some()
        && writer
            .kinds
            .iter()
            .any(|kind| kind.capability.kind == MediaKind::Animation)
        && lookup.selection.classify(lookup.chain, entry.path()) == EntrySelection::Included
        && lookup.index.archive.raw_path(entry.index()).is_none()
        && entry
            .path()
            .rsplit_once('.')
            .is_some_and(|(_, ext)| ext.eq_ignore_ascii_case("xml"))
    {
        PayloadVerification::MatchOriginal
    } else {
        PayloadVerification::Ordinary
    }
}
