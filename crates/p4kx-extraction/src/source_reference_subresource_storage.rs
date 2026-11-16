// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::OnceLock;

use super::{SourceLocation, SourceUseRef, Subresource};

#[derive(Debug)]
pub(crate) struct StoredReferenceSubresource {
    reference: SourceUseRef,
    site: Site,
    original: OnceLock<Box<SourceLocation>>,
}

#[derive(Debug, Clone, Copy)]
struct Site {
    indices: [u64; 2],
    index_count: u8,
    namespace_start: u32,
    namespace_length: u32,
    field_start: u32,
}

impl StoredReferenceSubresource {
    pub(super) fn from_original(
        location: &SourceLocation,
        reference: &SourceUseRef,
        location_start: usize,
    ) -> Option<Self> {
        let SourceLocation::Subresource {
            namespace,
            indices,
            field,
        } = location
        else {
            return None;
        };
        let site = Subresource {
            namespace,
            indices,
            field,
        };
        Self::new(&site, reference, location_start)
    }

    pub(super) fn new(
        site: &Subresource<'_>,
        reference: &SourceUseRef,
        location_start: usize,
    ) -> Option<Self> {
        let (indices, index_count) = match site.indices {
            [] => ([0, 0], 0),
            [first] => ([*first, 0], 1),
            [first, second] => ([*first, *second], 2),
            _ => return None,
        };
        let text = reference.as_str();
        let (length, tail) = text
            .get(location_start..)?
            .strip_prefix('s')?
            .split_once(':')?;
        if length.parse::<usize>().ok()? != site.namespace.len() {
            return None;
        }
        tail.strip_prefix(site.namespace)?;
        let field_start = text.strip_suffix(site.field)?.len();
        let site = Site {
            indices,
            index_count,
            namespace_start: u32::try_from(text.len() - tail.len()).ok()?,
            namespace_length: u32::try_from(site.namespace.len()).ok()?,
            field_start: u32::try_from(field_start).ok()?,
        };
        Some(Self {
            reference: reference.clone(),
            site,
            original: OnceLock::new(),
        })
    }

    pub(super) fn subresource(&self) -> Subresource<'_> {
        // Text offsets were checked against this immutable UTF-8 reference.
        let text = self.reference.as_str();
        let (_, tail) = text.split_at(self.site.namespace_start as usize);
        let (namespace, _) = tail.split_at(self.site.namespace_length as usize);
        let (_, field) = text.split_at(self.site.field_start as usize);
        let (indices, _) = self
            .site
            .indices
            .split_at(usize::from(self.site.index_count));
        Subresource {
            namespace,
            indices,
            field,
        }
    }

    pub(super) fn original(&self) -> &SourceLocation {
        self.original
            .get_or_init(|| Box::new(self.subresource().to_owned_location()))
    }

    #[cfg(test)]
    pub(super) fn is_materialized(&self) -> bool {
        self.original.get().is_some()
    }
}

impl Clone for StoredReferenceSubresource {
    fn clone(&self) -> Self {
        Self {
            reference: self.reference.clone(),
            site: self.site,
            original: OnceLock::new(),
        }
    }
}

impl PartialEq for StoredReferenceSubresource {
    fn eq(&self, other: &Self) -> bool {
        let left = self.subresource();
        let right = other.subresource();
        left.namespace == right.namespace
            && left.indices == right.indices
            && left.field == right.field
    }
}

impl Eq for StoredReferenceSubresource {}
