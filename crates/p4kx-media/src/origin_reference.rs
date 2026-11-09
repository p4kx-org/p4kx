// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{FieldPath, InputScope, SourceUseRole};

use crate::origin_capture::OriginSite;
use crate::origin_contributions::OriginContributions;
use crate::origin_input::MediaOriginInput;
use crate::origin_prepared::PreparedRecordOrigins;
use crate::reference_resolve::ReferenceObservations;
use crate::{
    MediaError, MediaWriter, Reference, ReferenceResolution, ReferenceTarget, origin_error,
};

pub(crate) trait CandidateInput {
    fn original(&self) -> &MediaOriginInput;
    fn property_site(&self) -> Option<PropertySite> {
        None
    }
}

impl CandidateInput for MediaOriginInput {
    fn original(&self) -> &MediaOriginInput {
        self
    }
}

#[derive(Clone, Copy)]
pub(crate) struct PropertySite {
    pub(crate) ordinal: u64,
    pub(crate) name: u64,
}

#[derive(Clone, Copy)]
pub(crate) enum ReferenceIdentifier {
    Path,
    Text,
}

#[derive(Clone, Copy)]
pub(crate) enum ReferenceSubresource {
    None,
    Definition,
    Property,
}

/// Inputs retained at the owning lookup and canonical admission boundaries.
pub(crate) struct ReferenceOriginInputs<'a, E> {
    pub(crate) original: &'a OriginContributions,
    pub(crate) selector: &'a OriginContributions,
    pub(crate) inventory: &'a OriginContributions,
    pub(crate) semantic: &'a OriginContributions,
    pub(crate) candidates: &'a ReferenceObservations<E>,
    pub(crate) read_selected: bool,
    pub(crate) identifier: ReferenceIdentifier,
    pub(crate) subresource: ReferenceSubresource,
}

impl<E: CandidateInput> ReferenceOriginInputs<'_, E> {
    pub(crate) fn write(
        &self,
        prepared: &mut PreparedRecordOrigins,
        prefix: &FieldPath,
        reference: &Reference,
        writer: &MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let full = self.resolution(writer)?;
        prepared.insert_subtree(prefix, full.clone(), writer)?;
        let original = child(prefix, "original")?;
        prepared.insert_subtree(&original, self.original.clone(), writer)?;
        independent(prepared, &child(&original, "type")?, writer)?;
        if matches!(self.identifier, ReferenceIdentifier::Text) {
            independent(prepared, &child(&original, "namespace")?, writer)?;
        }
        for field in ["resolution", "observed_candidates", "candidates_complete"] {
            prepared.insert_subtree(&child(prefix, field)?, full.clone(), writer)?;
        }
        let resolution = child(prefix, "resolution")?;
        self.targets(prepared, &resolution, reference.resolution(), writer)
    }

    pub(crate) fn resolution(
        &self,
        writer: &MediaWriter<'_>,
    ) -> Result<OriginContributions, MediaError> {
        let mut capture = writer.capture_origins();
        capture.include(self.selector, writer)?;
        capture.include(self.semantic, writer)?;
        let observed = !self.candidates.retained.is_empty() || self.candidates.discarded.is_some();
        if self.candidates.inventory_used || (self.read_selected && observed) {
            capture.include(self.inventory, writer)?;
        }
        for candidate in self
            .candidates
            .retained
            .iter()
            .chain(self.candidates.discarded.iter())
        {
            if let Some(indices) = property_indices(candidate, writer)? {
                capture.include(&indices, writer)?;
            }
            capture.add(
                candidate.original(),
                InputScope::ArchiveDirectoryMetadata,
                OriginSite::new("archive-entry", &[], "reference-candidate"),
                SourceUseRole::Selector,
                writer,
            )?;
        }
        Ok(capture.finish())
    }

    fn targets(
        &self,
        prepared: &mut PreparedRecordOrigins,
        prefix: &FieldPath,
        resolution: &ReferenceResolution,
        writer: &MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        match resolution {
            ReferenceResolution::Resolved { target }
            | ReferenceResolution::NotSelected { target, .. } => {
                let input = self.candidates.retained.first().ok_or_else(|| {
                    MediaError::Contract("resolved reference lost its original candidate".into())
                })?;
                self.target(prepared, &child(prefix, "target")?, target, input, writer)?;
            }
            ReferenceResolution::Ambiguous { candidates }
            | ReferenceResolution::Unavailable {
                known_candidates: candidates,
                ..
            } => {
                let field = if matches!(resolution, ReferenceResolution::Ambiguous { .. }) {
                    "candidates"
                } else {
                    "known_candidates"
                };
                if candidates.len() != self.candidates.retained.len() {
                    return Err(MediaError::Contract(
                        "reference candidate evidence lost retained order".into(),
                    ));
                }
                for (index, (candidate, input)) in
                    candidates.iter().zip(&self.candidates.retained).enumerate()
                {
                    let mut path = child(prefix, field)?;
                    path.push_index(index)
                        .map_err(|cause| origin_error::address(&cause))?;
                    self.target(
                        prepared,
                        &child(&path, "target")?,
                        &candidate.target,
                        input,
                        writer,
                    )?;
                }
            }
            ReferenceResolution::Missing => {}
        }
        if let ReferenceResolution::Unavailable {
            reason:
                crate::ReferenceUnavailable::CandidateLimit { .. }
                | crate::ReferenceUnavailable::CandidateBytes { .. },
            ..
        } = resolution
        {
            independent(
                prepared,
                &child(&child(prefix, "reason")?, "limit")?,
                writer,
            )?;
        }
        Ok(())
    }

    fn target(
        &self,
        prepared: &mut PreparedRecordOrigins,
        prefix: &FieldPath,
        target: &ReferenceTarget,
        candidate: &E,
        writer: &MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let input = candidate.original();
        let mut selector = writer.capture_origins();
        selector.include(self.selector, writer)?;
        selector.add(
            input,
            InputScope::ArchiveDirectoryMetadata,
            OriginSite::new("archive-entry", &[], "reference-candidate"),
            SourceUseRole::Selector,
            writer,
        )?;
        crate::origin_prepared_source::identity(
            prepared,
            &child(prefix, "source")?,
            &target.source,
            input,
            &selector.finish(),
            writer,
        )?;
        let mut root = writer.capture_origins();
        if let Some(input) = writer.origins.root.as_ref() {
            root.add(
                input,
                InputScope::Payload,
                OriginSite::new("media-physical-input", &[], "complete-archive-file"),
                SourceUseRole::Conversion,
                writer,
            )?;
        } else {
            root.include(&OriginContributions::unknown(&writer.origins.owner), writer)?;
        }
        prepared.insert_subtree(&child(prefix, "root")?, root.finish(), writer)?;
        independent(prepared, &child(prefix, "kind")?, writer)?;
        self.write_subresource(prepared, prefix, target, candidate, writer)
    }

    fn write_subresource(
        &self,
        prepared: &mut PreparedRecordOrigins,
        prefix: &FieldPath,
        target: &ReferenceTarget,
        candidate: &E,
        writer: &MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        if target.subresource.is_none() && matches!(self.subresource, ReferenceSubresource::None) {
            independent(prepared, &child(prefix, "subresource")?, writer)?;
        }
        if let Some(subresource) = &target.subresource
            && !matches!(self.subresource, ReferenceSubresource::None)
        {
            let path = child(prefix, "subresource")?;
            independent(prepared, &child(&path, "namespace")?, writer)?;
            if matches!(self.subresource, ReferenceSubresource::Property) {
                let mut capture = writer.capture_origins();
                capture.include(self.selector, writer)?;
                if self.read_selected {
                    capture.include(self.inventory, writer)?;
                }
                capture.include(
                    &property_indices(candidate, writer)?.ok_or_else(|| {
                        MediaError::Contract(
                            "Property target lost its original ordinal input".into(),
                        )
                    })?,
                    writer,
                )?;
                prepared.insert_subtree(&child(&path, "indices")?, capture.finish(), writer)?;
            }
            if matches!(self.subresource, ReferenceSubresource::Definition)
                && subresource.indices.is_empty()
            {
                independent(prepared, &child(&path, "indices")?, writer)?;
            }
        }
        Ok(())
    }
}

pub(crate) fn child(prefix: &FieldPath, field: &str) -> Result<FieldPath, MediaError> {
    let mut path = prefix.clone();
    path.push_field(field)
        .map_err(|cause| origin_error::address(&cause))?;
    Ok(path)
}

pub(crate) fn independent(
    prepared: &mut PreparedRecordOrigins,
    path: &FieldPath,
    writer: &MediaWriter<'_>,
) -> Result<(), MediaError> {
    prepared.insert_subtree(
        path,
        OriginContributions::independent(&writer.origins.owner),
        writer,
    )
}

fn property_indices(
    candidate: &impl CandidateInput,
    writer: &MediaWriter<'_>,
) -> Result<Option<OriginContributions>, MediaError> {
    let Some(site) = candidate.property_site() else {
        return Ok(None);
    };
    let mut capture = writer.capture_origins();
    capture.add(
        candidate.original(),
        InputScope::Payload,
        OriginSite::new("shader-property", &[site.ordinal], "declaration-index"),
        SourceUseRole::Conversion,
        writer,
    )?;
    capture.add(
        candidate.original(),
        InputScope::Payload,
        OriginSite::new("shader-property", &[site.ordinal, site.name], "Name"),
        SourceUseRole::Selector,
        writer,
    )?;
    Ok(Some(capture.finish()))
}
