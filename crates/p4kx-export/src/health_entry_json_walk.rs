// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::Deserializer;
use serde_json::value::RawValue;

use super::EntryJson;
use crate::{CatalogError, ExportAccountingError};

/// One event from the validated JSON document.
#[derive(Clone, Copy)]
pub(crate) enum JsonEvent<'read> {
    Array,
    Object,
    Name(EntryJson<'read>),
    Value(EntryJson<'read>),
    End(EntryJson<'read>),
}

struct Open<'read> {
    tail: &'read str,
    end: char,
}

/// Visits JSON containers and scalar tokens without rescanning nested values.
pub(crate) struct JsonWalk<'read> {
    root: EntryJson<'read>,
    remaining: &'read str,
    open: Vec<Open<'read>>,
}

impl<'read> JsonWalk<'read> {
    pub(super) const fn new(root: EntryJson<'read>) -> Self {
        Self {
            root,
            remaining: root.raw(),
            open: Vec::new(),
        }
    }

    fn next_event(&mut self) -> Result<Option<JsonEvent<'read>>, CatalogError> {
        let Some(byte) = self.skip_separators() else {
            return if self.open.is_empty() {
                Ok(None)
            } else {
                Err(ExportAccountingError::ConflictingSourceDatum.into())
            };
        };
        match byte {
            b'[' => self.begin(']', JsonEvent::Array).map(Some),
            b'{' => self.begin('}', JsonEvent::Object).map(Some),
            b']' | b'}' => self.end().map(Some),
            _ => self.atom().map(Some),
        }
    }

    fn skip_separators(&mut self) -> Option<u8> {
        self.remaining = self
            .remaining
            .trim_start_matches(|ch: char| ch.is_ascii_whitespace() || matches!(ch, ',' | ':'));
        self.remaining.as_bytes().first().copied()
    }

    fn begin(
        &mut self,
        end: char,
        event: JsonEvent<'read>,
    ) -> Result<JsonEvent<'read>, CatalogError> {
        let tail = self.remaining;
        self.remaining = tail
            .get(1..)
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        self.open
            .try_reserve(1)
            .map_err(CatalogError::JsonSourceAllocation)?;
        self.open.push(Open { tail, end });
        Ok(event)
    }

    fn end(&mut self) -> Result<JsonEvent<'read>, CatalogError> {
        let open = self
            .open
            .pop()
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        self.remaining = self
            .remaining
            .strip_prefix(open.end)
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        let length = open
            .tail
            .len()
            .checked_sub(self.remaining.len())
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        let raw = open
            .tail
            .get(..length)
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        Ok(JsonEvent::End(self.root.with_token(raw)))
    }

    fn atom(&mut self) -> Result<JsonEvent<'read>, CatalogError> {
        let remaining = self.remaining;
        let mut tokens = Deserializer::from_str(remaining).into_iter::<&RawValue>();
        let token = tokens
            .next()
            .transpose()
            .map_err(|_| ExportAccountingError::ConflictingSourceDatum)?
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        self.remaining = remaining
            .get(tokens.byte_offset()..)
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        let token = self.root.with_token(token.get());
        if self.remaining.trim_ascii_start().starts_with(':') {
            Ok(JsonEvent::Name(token))
        } else {
            Ok(JsonEvent::Value(token))
        }
    }
}

impl<'read> Iterator for JsonWalk<'read> {
    type Item = Result<JsonEvent<'read>, CatalogError>;

    fn next(&mut self) -> Option<Self::Item> {
        let next = self.next_event();
        if next.is_err() {
            self.open.clear();
            self.remaining = "";
        }
        next.transpose()
    }
}
