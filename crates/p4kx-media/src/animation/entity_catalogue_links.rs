// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{
    ClipArchiveSource, ClipDatabaseUnavailable, ClipSourceSelection, EntityClipRig, NamedCafTarget,
    RigFile,
};

use super::super::{LINK, Link, Payload, Scope, SelectionFailure};
use super::{Rows, positions};
use crate::origin_capture::OriginSite;
use crate::origin_contributions::OriginContributions;
use crate::{MediaError, MediaWriter};
use p4kx_extraction::{InputScope, SourceUseRole};

impl Rows<'_, '_> {
    pub(super) fn links(
        &self,
        rig: usize,
        node: &EntityClipRig,
        catalogue_sources: &[ClipArchiveSource],
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let Some(parameters) = node.rig.parameters.as_ref().and_then(RigFile::value) else {
            return Ok(());
        };
        let input = node
            .rig
            .parameters
            .as_ref()
            .and_then(RigFile::source)
            .map(|input| self.observe(input, writer))
            .transpose()?;
        let mut links = node.links.iter().peekable();
        for (slot, parameter) in parameters.iter().enumerate() {
            Self::parameter(rig, slot, parameter, input.as_ref(), writer)?;
            let selection = links
                .next_if(|link| link.parameter == slot)
                .map(|link| &link.selection);
            if let Some(ClipSourceSelection::Folder { sources }) = selection
                && !sources.is_empty()
            {
                for (candidate, &source) in sources.iter().enumerate() {
                    self.link(
                        [rig, slot, candidate],
                        Link::FolderCandidate { source },
                        &parameter.selectors,
                        input.as_ref(),
                        catalogue_sources,
                        writer,
                    )?;
                }
                continue;
            }
            let link = link_of(selection);
            let mut selectors = parameter.selectors.clone();
            if let Some(folder) = link.folder_parameter() {
                let owner = parameters.get(folder).ok_or_else(|| {
                    MediaError::Contract("CAF link points to an unavailable selector".into())
                })?;
                selectors.extend_from_slice(&owner.selectors);
            }
            self.link(
                [rig, slot, 0],
                link,
                &selectors,
                input.as_ref(),
                catalogue_sources,
                writer,
            )?;
        }
        Ok(())
    }

    fn link(
        &self,
        address: [usize; 3],
        link: Link,
        selectors: &[p4kx_extraction::SourceLocation],
        input: Option<&crate::origin_input::MediaOriginInput>,
        catalogue_sources: &[ClipArchiveSource],
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let proof = input.map_or_else(
            || Ok(OriginContributions::unknown(&writer.origins.owner)),
            |input| positions(input, selectors, writer),
        )?;
        let mut scope = Scope::new(writer);
        let selection =
            self.link_selection(link.source_position(), catalogue_sources, &proof, writer)?;
        scope.at("disposition", selection.clone(), writer)?;
        if link.source_position().is_some() {
            scope.at("source", selection, writer)?;
        }
        match &link {
            Link::NamedCaf { .. } | Link::Unable { .. } => {
                scope.at("filepath_parameter", proof, writer)?;
            }
            _ => {}
        }
        scope.write(
            LINK,
            address.into_iter().map(|index| index as u64).collect(),
            &Payload::Link(link),
            &[],
            writer,
        )
    }

    fn link_selection(
        &self,
        target: Option<usize>,
        catalogue_sources: &[ClipArchiveSource],
        selector: &OriginContributions,
        writer: &MediaWriter<'_>,
    ) -> Result<OriginContributions, MediaError> {
        let mut combined = writer.capture_origins();
        combined.include(selector, writer)?;
        let archive = self.lookup.index.archive;
        if let Some(target) = target {
            let original = catalogue_sources
                .get(target)
                .ok_or_else(crate::origin_error::foreign)?;
            let entry = archive
                .entries()
                .get(original.archive_ordinal)
                .ok_or_else(crate::origin_error::foreign)?;
            let input = writer.origins.input(archive, entry)?;
            combined.add(
                &input,
                InputScope::ArchiveDirectoryMetadata,
                OriginSite::new("archive-entry", &[original.archive_ordinal as u64], "path"),
                SourceUseRole::Selector,
                writer,
            )?;
        }
        Ok(combined.finish())
    }
}

impl Link {
    const fn source_position(&self) -> Option<usize> {
        match self {
            Self::NamedCaf { source, .. }
            | Self::Database { source }
            | Self::FolderCandidate { source } => Some(*source),
            _ => None,
        }
    }
    const fn folder_parameter(&self) -> Option<usize> {
        match self {
            Self::NamedCaf {
                filepath_parameter, ..
            }
            | Self::Unable {
                filepath_parameter, ..
            } => *filepath_parameter,
            _ => None,
        }
    }
}

const fn link_of(selection: Option<&ClipSourceSelection>) -> Link {
    match selection {
        Some(ClipSourceSelection::Folder { .. }) => Link::FolderEmpty,
        Some(ClipSourceSelection::FolderUnavailable) => Link::FolderUnavailable,
        Some(ClipSourceSelection::Database { source }) => Link::Database { source: *source },
        Some(ClipSourceSelection::DatabaseUnavailable { reason }) => Link::Unable {
            reason: database_failure(reason),
            filepath_parameter: None,
        },
        Some(ClipSourceSelection::NamedCaf {
            filepath_parameter,
            target,
        }) => match target {
            NamedCafTarget::File(source) => Link::NamedCaf {
                source: *source,
                filepath_parameter: *filepath_parameter,
            },
            _ => Link::Unable {
                reason: named_failure(target),
                filepath_parameter: *filepath_parameter,
            },
        },
        _ => Link::Unexpanded,
    }
}

const fn database_failure(reason: &ClipDatabaseUnavailable) -> SelectionFailure {
    match reason {
        ClipDatabaseUnavailable::Missing => SelectionFailure::Missing,
        ClipDatabaseUnavailable::Ambiguous => SelectionFailure::Ambiguous,
        ClipDatabaseUnavailable::UnsupportedPath => SelectionFailure::InvalidPath,
        _ => SelectionFailure::Unreadable,
    }
}

const fn named_failure(target: &NamedCafTarget) -> SelectionFailure {
    match target {
        NamedCafTarget::Missing => SelectionFailure::Missing,
        NamedCafTarget::Ambiguous => SelectionFailure::Ambiguous,
        NamedCafTarget::MissingScope => SelectionFailure::MissingScope,
        NamedCafTarget::InvalidPath => SelectionFailure::InvalidPath,
        _ => SelectionFailure::Unreadable,
    }
}
