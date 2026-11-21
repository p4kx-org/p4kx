// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ptr;
use std::sync::Arc;

use p4kx_extraction::FieldOrigin;
use serde_json::value::{self, RawValue};

use crate::MediaError;
use crate::origin_contributions::OriginContributions;

/// Reuses the largest repeated source-use array within one output occurrence.
#[derive(Default)]
pub(super) struct Cache {
    largest: Option<ReferenceArray>,
}

struct ReferenceArray {
    contribution: OriginContributions,
    origin: FieldOrigin,
    encoded: Option<Box<RawValue>>,
}

impl Cache {
    pub(super) fn with_origin<T>(
        &mut self,
        actual: &OriginContributions,
        emit: impl FnOnce(&FieldOrigin, Option<&RawValue>) -> Result<T, MediaError>,
    ) -> Result<T, MediaError> {
        let uses = actual.uses.len();
        if let Some(previous) = &mut self.largest
            && previous.matches(actual)
        {
            if previous.encoded.is_none() {
                previous.encoded = Some(value::to_raw_value(previous.origin.source_uses_refs())?);
            }
            return emit(&previous.origin, previous.encoded.as_deref());
        }

        let origin = actual.field_origin()?;
        if uses > 1
            && self.largest.as_ref().is_none_or(|previous| {
                previous.encoded.is_none() || uses >= previous.contribution.uses.len()
            })
        {
            self.largest = Some(ReferenceArray {
                contribution: actual.clone(),
                origin: origin.clone(),
                encoded: None,
            });
        }
        emit(&origin, None)
    }
}

impl ReferenceArray {
    fn matches(&self, actual: &OriginContributions) -> bool {
        let previous = &self.contribution;
        previous.complete == actual.complete
            && previous.owner.is_same_inventory(&actual.owner)
            && (Arc::ptr_eq(&previous.uses, &actual.uses)
                || (previous.uses.len() == actual.uses.len()
                    && previous
                        .uses
                        .iter()
                        .zip(actual.uses.iter())
                        .all(|(left, right)| ptr::eq(left.description(), right.description()))))
    }
}

#[cfg(test)]
#[path = "origin_stream_reference_cache_tests.rs"]
mod tests;
