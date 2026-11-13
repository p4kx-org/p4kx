// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::FieldPath;

use crate::directory_lookup::DirectoryLookup;
use crate::origin_contributions::OriginContributions;
use crate::origin_prepared::PreparedRecordOrigins;
use crate::origin_reference::{ReferenceIdentifier, ReferenceOriginInputs, ReferenceSubresource};
use crate::{
    CandidateInventory, MediaError, MediaKind, MediaWriter, OriginalIdentifier, Reference,
    ReferenceUnavailable, origin_error,
};

pub(crate) struct ReferenceSelection<'a> {
    pub(crate) authored_path: &'a str,
    pub(crate) archive_path: Option<&'a str>,
    pub(crate) kind: Option<MediaKind>,
    pub(crate) original: &'a OriginContributions,
    pub(crate) selector: &'a OriginContributions,
}

impl ReferenceSelection<'_> {
    pub(crate) fn resolve_at(
        &self,
        directory: &DirectoryLookup<'_>,
        address: &str,
        prepared: &mut PreparedRecordOrigins,
        writer: &mut MediaWriter<'_>,
    ) -> Result<Reference, MediaError> {
        let original = OriginalIdentifier::new_with_path(self.authored_path);
        let address = FieldPath::parse(address).map_err(|cause| origin_error::address(&cause))?;
        let Some(path) = self.archive_path else {
            prepared.insert_subtree(&address, self.selector.clone(), writer)?;
            let original_address = crate::origin_reference::child(&address, "original")?;
            prepared.insert_subtree(&original_address, self.original.clone(), writer)?;
            crate::origin_reference::independent(
                prepared,
                &crate::origin_reference::child(&original_address, "type")?,
                writer,
            )?;
            return writer.resolve_reference(
                original,
                [],
                CandidateInventory::Unavailable(ReferenceUnavailable::InvalidIdentifier),
            );
        };
        let lookup = directory.lookup_input_observed(original, path, self.kind, writer)?;
        let origins = ReferenceOriginInputs {
            original: self.original,
            selector: self.selector,
            inventory: &lookup.inventory,
            semantic: self.original,
            candidates: &lookup.evidence,
            read_selected: false,
            identifier: ReferenceIdentifier::Path,
            subresource: ReferenceSubresource::None,
        };
        origins.write(prepared, &address, &lookup.lookup.reference, writer)?;
        Ok(lookup.lookup.reference)
    }
}

pub(crate) fn archive_animation_path(path: &str) -> Option<String> {
    if path.split(['/', '\\']).any(|part| {
        matches!(part, "" | "." | "..")
            || part.contains([':', '\0', '!', '*', '?'])
            || part.ends_with([' ', '.'])
    }) {
        return None;
    }
    let mut parts = path.split(['/', '\\']);
    let first = parts.next()?;
    if first.eq_ignore_ascii_case("data") && parts.next().is_some() {
        Some(path.replace('\\', "/"))
    } else if first.eq_ignore_ascii_case("animations") && parts.next().is_some() {
        Some(format!("Data/{}", path.replace('\\', "/")))
    } else {
        None
    }
}
