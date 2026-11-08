// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use p4kx_extraction::Entry;

use crate::directory_lookup::DirectoryLookup;
use crate::error;
use crate::required_reader::RequiredReader;
use crate::retained_source;
use crate::source;
use crate::source_selection::EntrySelection;
use crate::{
    EntryInput, Identity, InputOmission, InputReceipt, MediaError, MediaWriter, Source,
    SourceContext, Subresource,
};

/// Checked access to one required input for the duration of its callback.
pub struct RequiredInput<'input> {
    receipt: InputReceipt,
    reader: RequiredReader<'input>,
}

impl RequiredInput<'_> {
    /// Returns measured input evidence without granting publication authority.
    #[must_use]
    pub const fn receipt(&self) -> &InputReceipt {
        &self.receipt
    }
}

impl Read for RequiredInput<'_> {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        self.reader.read(output)
    }
}

impl Seek for RequiredInput<'_> {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.reader.seek(position)
    }
}

impl SourceContext<'_> {
    /// Uses one proved dependency and cleans private bytes before returning.
    ///
    /// # Errors
    /// Returns original admission, source, I/O, callback and combined cleanup failures.
    pub fn with_required_input<T>(
        &self,
        input: &EntryInput<'_>,
        use_at: &Subresource,
        writer: &mut MediaWriter<'_>,
        consume: impl FnOnce(&mut RequiredInput<'_>, &mut MediaWriter<'_>) -> Result<T, MediaError>,
    ) -> Result<T, MediaError> {
        let entry = validate(self, input, use_at, writer)?;
        let lookup = self.directory;
        if lookup.selection.classify(lookup.chain, entry.path()) != EntrySelection::Excluded {
            let (source, payload) = retain(lookup, entry, writer)?;
            let digest = source
                .raw
                .as_ref()
                .ok_or_else(|| MediaError::Contract("retained input has no raw receipt".into()))?
                .digest
                .clone();
            return use_payload(
                lookup,
                entry,
                &payload,
                InputReceipt {
                    source,
                    digest,
                    omission: None,
                    use_at: use_at.clone(),
                },
                writer,
                consume,
            );
        }
        if !lookup
            .selection
            .exclude
            .is_part_of_kept_texture(self.entry.path(), entry.path())
        {
            return Err(MediaError::Contract(
                "excluded input is not a required part of the selected texture".into(),
            ));
        }
        with_private_input(lookup, entry, use_at, writer, consume)
    }
}

fn with_private_input<T>(
    lookup: &DirectoryLookup<'_>,
    entry: &Entry,
    use_at: &Subresource,
    writer: &mut MediaWriter<'_>,
    consume: impl FnOnce(&mut RequiredInput<'_>, &mut MediaWriter<'_>) -> Result<T, MediaError>,
) -> Result<T, MediaError> {
    let state = lookup.input_state()?;
    let declared = source::entry(lookup.root, lookup.index.archive, entry, lookup.chain)?;
    state.begin(entry.index(), writer)?;
    let result = (|| {
        state
            .budget
            .bytes(entry.uncompressed_size(), writer.limits())?;
        retained_source::with_private(
            lookup.index.archive,
            entry,
            writer,
            |digest, payload, writer| {
                let mut source = declared.clone();
                source::verified(&mut source, entry);
                use_payload(
                    lookup,
                    entry,
                    payload,
                    InputReceipt {
                        source,
                        digest,
                        omission: Some(InputOmission::ExplicitExclusion),
                        use_at: use_at.clone(),
                    },
                    writer,
                    consume,
                )
            },
        )
    })();
    error::finish(result, state.end(entry.index(), None))
        .map_err(|cause| error::at(&declared, cause))
}

pub(crate) fn retain(
    lookup: &DirectoryLookup<'_>,
    entry: &Entry,
    writer: &mut MediaWriter<'_>,
) -> Result<(Source, PathBuf), MediaError> {
    let state = lookup.input_state()?;
    let declared = source::entry(lookup.root, lookup.index.archive, entry, lookup.chain)?;
    let cached = state.begin(entry.index(), writer)?;
    let first = cached.is_none();
    let retained = match cached {
        Some(raw) => {
            let payload = writer.root.join(&raw.path);
            Ok((raw, payload))
        }
        None => state
            .budget
            .bytes(entry.uncompressed_size(), writer.limits())
            .and_then(|()| retained_source::retain(lookup.index.archive, entry, writer)),
    };
    let result = match retained {
        Err(cause) => Err(error::at(&declared, cause)),
        Ok((raw, payload)) => {
            let mut source = declared;
            source::retained(&mut source, entry, raw);
            let publication = if first {
                writer.retained(&source)
            } else {
                Ok(())
            };
            match publication {
                Ok(()) => Ok((source, payload)),
                Err(cause) => Err(error::at(&source, cause)),
            }
        }
    };
    let raw = if first {
        result
            .as_ref()
            .ok()
            .and_then(|(source, _)| source.raw.clone())
    } else {
        None
    };
    error::finish(result, state.end(entry.index(), raw))
}

fn use_payload<T>(
    lookup: &DirectoryLookup<'_>,
    entry: &Entry,
    payload: &Path,
    receipt: InputReceipt,
    writer: &mut MediaWriter<'_>,
    consume: impl FnOnce(&mut RequiredInput<'_>, &mut MediaWriter<'_>) -> Result<T, MediaError>,
) -> Result<T, MediaError> {
    let reader = RequiredReader::open(
        payload,
        entry.path(),
        receipt.source.size,
        lookup.input_state()?.budget,
        writer.checkpoint,
        writer.limits(),
    )
    .map_err(|cause| error::at(&receipt.source, cause))?;
    let mut input = RequiredInput { receipt, reader };
    let result = consume(&mut input, writer)
        .map_err(|cause| error::at(&input.receipt.source, error::input(cause)));
    drop(input);
    result
}

fn validate<'context>(
    context: &'context SourceContext<'_>,
    input: &EntryInput<'_>,
    use_at: &Subresource,
    writer: &MediaWriter<'_>,
) -> Result<&'context Entry, MediaError> {
    writer.require_source(context.source)?;
    let lookup = context.directory;
    let owner_matches = matches!(
        &context.source.identity,
        Identity::Entry { containers, entry }
            if containers == lookup.chain && entry.entry_index == context.entry.index()
    );
    if use_at.namespace.is_empty()
        || !owner_matches
        || &context.source.root != lookup.root
        || input.root != lookup.root
        || input.chain != lookup.chain
        || lookup
            .selection
            .classify(lookup.chain, context.entry.path())
            != EntrySelection::Included
    {
        return Err(MediaError::Contract(
            "required input is outside its active source scope".into(),
        ));
    }
    let entry = lookup
        .index
        .archive
        .entries()
        .get(input.position)
        .filter(|entry| entry.index() == input.position)
        .ok_or_else(|| MediaError::Contract("required input has no physical Entry".into()))?;
    if lookup.index.archive.raw_path(entry.index()).is_some() {
        return Err(MediaError::Contract(
            "required input has an unproved raw path".into(),
        ));
    }
    writer.checkpoint()?;
    Ok(entry)
}

#[cfg(test)]
#[path = "required_input_fault_tests.rs"]
mod fault_tests;

#[cfg(test)]
#[path = "required_token_fault_tests.rs"]
mod token_fault_tests;
