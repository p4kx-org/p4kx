// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;

use p4kx_extraction::{SourceLocation, SourceUse, SourceUseRole};
use serde_json::value::RawValue;

use super::health_entry_frame::EntryCapture;
use super::health_entry_read::EntryRead;
use crate::{CatalogError, ExportAccountingError};

/// One validated JSON token borrowed from its actual complete Entry read.
#[derive(Clone, Copy)]
pub(crate) struct EntryJson<'read> {
    read: &'read EntryRead,
    token: &'read str,
}

impl<'read> EntryJson<'read> {
    /// Validates a full JSON document in the same original source frame.
    pub(crate) fn parse(
        read: &'read EntryRead,
        capture: &EntryCapture<'_>,
    ) -> Result<Result<Self, serde_json::Error>, CatalogError> {
        read.receipt().check_frame(&capture.frame())?;
        Ok(
            serde_json::from_slice::<&RawValue>(read.complete_bytes()?).map(|token| Self {
                read,
                token: token.get(),
            }),
        )
    }

    /// Borrows the exact original spelling of this token.
    pub(crate) const fn raw(self) -> &'read str {
        self.token
    }

    /// Borrows a validated JSON number without converting it to a fixed-width number.
    pub(crate) fn number(self) -> Option<&'read str> {
        let raw = self.raw();
        raw.as_bytes()
            .first()
            .is_some_and(|byte| *byte == b'-' || byte.is_ascii_digit())
            .then_some(raw)
    }

    /// Decodes an original JSON string, borrowing it when no escape needs decoding.
    pub(crate) fn text(self) -> Result<Option<Cow<'read, str>>, serde_json::Error> {
        let raw = self.raw();
        if !raw.starts_with('"') {
            return Ok(None);
        }
        if raw.contains('\\') {
            serde_json::from_str(raw).map(Cow::Owned).map(Some)
        } else {
            serde_json::from_str(raw).map(Cow::Borrowed).map(Some)
        }
    }

    /// Visits the exact root token and its descendants in original order.
    pub(crate) const fn walk(self) -> JsonWalk<'read> {
        JsonWalk::new(self)
    }

    /// Registers the original decoded byte range of this token.
    pub(crate) fn observe(self, role: SourceUseRole) -> Result<SourceUse, CatalogError> {
        let bytes = self.read.complete_bytes()?;
        let raw = self.raw().as_bytes();
        let start = raw
            .as_ptr()
            .addr()
            .checked_sub(bytes.as_ptr().addr())
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        let end = start
            .checked_add(raw.len())
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        if !bytes
            .get(start..end)
            .is_some_and(|slot| std::ptr::eq(slot, raw))
        {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        let start =
            u64::try_from(start).map_err(|_| ExportAccountingError::ConflictingSourceDatum)?;
        let len =
            u64::try_from(raw.len()).map_err(|_| ExportAccountingError::ConflictingSourceDatum)?;
        let location = SourceLocation::Subresource {
            namespace: "archiveJson".to_owned(),
            indices: vec![start, len],
            field: "$token".to_owned(),
        };
        self.read
            .receipt()
            .payload_entry()?
            .payload_at(location, role)
    }

    const fn with_token(self, token: &'read str) -> Self {
        Self { token, ..self }
    }
}

#[path = "health_entry_json_walk.rs"]
mod walk;
pub(crate) use walk::{JsonEvent, JsonWalk};

#[cfg(test)]
#[path = "health_entry_json_tests.rs"]
mod tests;
