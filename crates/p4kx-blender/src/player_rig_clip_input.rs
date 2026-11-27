// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_assembly::{SocArchiveMember, SocCaptureError};
use p4kx_extraction::{
    CancelToken, Entry, InputScope, PathIndex, PhysicalSourceRef, SourceInput, SourceLocation,
    SourceScope, SourceUse, SourceUseRole, SourceUses, SourceValue, SourcedValue,
};
use schemars::JsonSchema;
use serde::Serialize;

use crate::player_source::{PlayerSourceError, check, index_error};

pub(super) struct Inputs<'a, 'archive> {
    pub(super) paths: &'a PathIndex<'archive>,
    pub(super) sources: &'a SourceUses,
    pub(super) directory: &'a SourceUse,
    pub(super) cancel: &'a CancelToken,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "camelCase")]
pub(super) enum Lookup {
    Missing,
    Ambiguous { candidates: Vec<PhysicalSourceRef> },
    Found { source: PhysicalSourceRef },
}

impl<'archive> Inputs<'_, 'archive> {
    pub(super) fn entries(&self, path: &str) -> Result<Vec<&'archive Entry>, PlayerSourceError> {
        self.paths
            .matching_entries_with(path, |_| check(self.cancel))
            .map_err(index_error)?
            .map(|result| result.map_err(index_error))
            .collect()
    }

    pub(super) fn lookup(
        &self,
        entries: &[&Entry],
    ) -> Result<SourcedValue<Lookup>, PlayerSourceError> {
        let mut uses = vec![self.directory.clone()];
        let mut refs = Vec::with_capacity(entries.len());
        for entry in entries {
            let scope = self.scope(entry)?;
            uses.push(scope.at(
                SourceLocation::Metadata {
                    field: "archiveEntryPath".into(),
                },
                SourceUseRole::Reference,
            )?);
            refs.push(scope.source_ref().clone());
        }
        let value = match refs.as_slice() {
            [] => Lookup::Missing,
            [source] => Lookup::Found {
                source: source.clone(),
            },
            _ => Lookup::Ambiguous { candidates: refs },
        };
        Ok(self.sources.value(SourceValue::Present(value), &uses, true))
    }

    pub(super) fn scope(&self, entry: &Entry) -> Result<SourceScope, PlayerSourceError> {
        let origin = self
            .paths
            .archive()
            .entry_origin(entry)
            .ok_or(PlayerSourceError::Binding)?;
        Ok(self
            .sources
            .observe(SourceInput::Archive(&origin), InputScope::Payload)?)
    }

    pub(super) fn member(&self, entry: &Entry) -> Result<SocArchiveMember, PlayerSourceError> {
        let archive = self.paths.archive();
        Ok(SocArchiveMember::read(archive, entry, || {
            if self.cancel.is_cancelled() {
                Err(SocCaptureError::Cancelled)
            } else {
                Ok(())
            }
        })?)
    }

    pub(super) fn unreadable<T>(
        &self,
        scope: &SourceScope,
        field: &str,
    ) -> Result<SourcedValue<T>, PlayerSourceError> {
        let attempted = scope.at(
            SourceLocation::Metadata {
                field: field.to_owned(),
            },
            SourceUseRole::Attempted,
        )?;
        Ok(self
            .sources
            .value(SourceValue::Unreadable, &[attempted], false))
    }

    pub(super) fn bytes<T>(
        &self,
        scope: &SourceScope,
        range: Range<u64>,
        field: &str,
        value: SourceValue<T>,
        complete: bool,
    ) -> Result<SourcedValue<T>, PlayerSourceError> {
        let used = scope.at(
            SourceLocation::Subresource {
                namespace: "animation-byte-range".into(),
                indices: vec![range.start, range.end],
                field: field.to_owned(),
            },
            SourceUseRole::Payload,
        )?;
        Ok(self.sources.value(value, &[used], complete))
    }
}
