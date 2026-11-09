// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::EntryMetadata;

use p4kx_extraction::{
    Archive, Entry, EntryOrigin, InputScope, SourceDetails, SourceInput, SourceScope, SourceUses,
};

use crate::origin_error;
use crate::{MediaError, MediaWriter, SourceContext};

/// Original observed input without authority restored from serialized identities.
#[derive(Debug, Clone)]
pub(crate) struct MediaOriginInput {
    pub(crate) owner: SourceUses,
    pub(crate) scope: SourceScope,
    pub(crate) details_limited: bool,
}

impl MediaOriginInput {
    pub(crate) fn observe(
        owner: &SourceUses,
        archive: &Archive,
        entry: &Entry,
        detail_bytes: u64,
    ) -> Result<Self, MediaError> {
        let observed = archive
            .observed_entry(entry)
            .ok_or_else(origin_error::foreign)?;
        Self::observe_entry(
            owner,
            SourceInput::ArchiveView(observed),
            entry.path(),
            entry.metadata(),
            observed
                .ancestors()
                .map(|ancestor| ancestor.map(|entry| (entry.path(), entry.metadata()))),
            detail_bytes,
        )
    }

    pub(crate) fn observe_origin(
        owner: &SourceUses,
        origin: &EntryOrigin,
        detail_bytes: u64,
    ) -> Result<Self, MediaError> {
        Self::observe_entry(
            owner,
            SourceInput::Archive(origin),
            origin.entry().path(),
            origin.entry().metadata(),
            origin
                .ancestors()
                .map(|ancestor| ancestor.map(|entry| (entry.path(), entry.metadata()))),
            detail_bytes,
        )
    }

    fn observe_entry<'a>(
        owner: &SourceUses,
        input: SourceInput<'_>,
        path: &str,
        metadata: &EntryMetadata,
        mut ancestors: impl Iterator<Item = Option<(&'a str, &'a EntryMetadata)>>,
        detail_bytes: u64,
    ) -> Result<Self, MediaError> {
        let mut scope = owner
            .observe_with_details(input, InputScope::Payload, SourceDetails::CaptureLimited)
            .map_err(origin_error::evidence)?;
        let mut remaining = detail_bytes;
        let full = charge_details(path, metadata, &mut remaining)
            && ancestors.all(|ancestor| {
                ancestor
                    .is_none_or(|(path, metadata)| charge_details(path, metadata, &mut remaining))
            });
        if full {
            scope = owner
                .observe(input, InputScope::Payload)
                .map_err(origin_error::evidence)?;
        }
        Ok(Self {
            owner: owner.clone(),
            scope,
            details_limited: !full,
        })
    }

    pub(crate) fn ancestor(&self, position: usize) -> Result<Option<Self>, MediaError> {
        self.scope
            .ancestor(position)
            .map_err(origin_error::evidence)
            .map(|scope| {
                scope.map(|scope| Self {
                    owner: self.owner.clone(),
                    scope,
                    details_limited: self.details_limited,
                })
            })
    }
}

fn charge_details(path: &str, metadata: &EntryMetadata, remaining: &mut u64) -> bool {
    let Ok(path_bytes) = u64::try_from(path.len()) else {
        return false;
    };
    let bytes =
        metadata
            .encryption_declarations()
            .iter()
            .try_fold(path_bytes, |bytes, declaration| {
                u64::try_from(size_of_val(declaration))
                    .ok()
                    .and_then(|declaration_bytes| bytes.checked_add(declaration_bytes))
            });
    let Some(bytes) = bytes else { return false };
    let Some(next) = remaining.checked_sub(bytes) else {
        return false;
    };
    *remaining = next;
    true
}

impl SourceContext<'_> {
    pub(crate) fn origin_input(
        &self,
        writer: &MediaWriter<'_>,
    ) -> Result<MediaOriginInput, MediaError> {
        writer.require_source(self.source)?;
        let observed = self
            .directory
            .index
            .archive
            .observed_entry(self.entry)
            .ok_or_else(origin_error::foreign)?;
        let input = writer
            .source_origin
            .as_ref()
            .ok_or_else(origin_error::foreign)?;
        if !input
            .scope
            .matches_entry(&observed)
            .map_err(origin_error::evidence)?
        {
            return Err(origin_error::foreign());
        }
        let active = writer.source.as_ref().ok_or_else(origin_error::foreign)?;
        let raw = active.raw.as_ref().ok_or_else(origin_error::foreign)?;
        if self.payload != writer.root.join(&raw.path) {
            return Err(origin_error::foreign());
        }
        Ok(input.clone())
    }
}
