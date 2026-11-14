// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::TryReserveError;

use p4kx_extraction::{
    Archive, ArchiveChanged, ArchiveWatch, CancelToken, EntryOrigin, PathIndex, PathIndexError,
    PathKeys,
};
use p4kx_model::{SocContainerDescriptor, SocContainerDescriptorError};
use thiserror::Error;

#[path = "soc_archive_member.rs"]
mod soc_archive_member;
#[path = "soc_descriptor_member.rs"]
mod soc_descriptor_member;
#[path = "soc_geometry_capture.rs"]
mod soc_geometry_capture;
#[path = "soc_nested_capture.rs"]
mod soc_nested_capture;
#[path = "soc_stream_capture.rs"]
mod soc_stream_capture;

pub use soc_archive_member::{SocArchiveMember, SocMemberError};
pub use soc_descriptor_member::SocMemberSelection;
pub use soc_geometry_capture::{
    SocGeometryCandidateKind, SocGeometryReference, SocGeometryTarget, SocGeometryTier,
};
pub use soc_nested_capture::{SocChildInput, SocChildState};
pub use soc_stream_capture::{SocMemberSelector, SocStreamInput};

/// The selected source occurrences of one logical SOC container tree.
#[derive(Debug)]
pub struct SocContainerCapture {
    occurrences: Vec<SocContainerOccurrence>,
}

impl SocContainerCapture {
    /// Reads authored sources from a container and its original nested placements.
    ///
    /// Each layer before the final container must be an actual containing Archive.
    ///
    /// # Errors
    /// Returns an error for invalid root context, cancellation, Archive mutation or failed storage.
    pub fn read(layers: &[&Archive], cancel: &CancelToken) -> Result<Self, SocCaptureError> {
        let (root, containing) = layers.split_last().ok_or(SocCaptureError::Root)?;
        if containing.is_empty()
            || containing
                .iter()
                .zip(layers.iter().skip(1))
                .any(|(owner, inner)| {
                    inner
                        .container_input_origin()
                        .is_none_or(|origin| !owner.owns_origin(&origin))
                })
        {
            return Err(SocCaptureError::Root);
        }
        let root_origin = root.container_input_origin().ok_or(SocCaptureError::Root)?;
        let watch = ArchiveWatch::new(root);
        let check = CaptureCheck {
            watch: &watch,
            cancel,
        };
        check.poll()?;
        let mut walk = soc_nested_capture::Walk::new(layers, check)?;
        walk.visit(root_origin)?;
        watch.check()?;
        Ok(Self {
            occurrences: walk.occurrences,
        })
    }

    /// Returns the logical occurrences, each child after its parent.
    #[must_use]
    pub fn occurrences(&self) -> &[SocContainerOccurrence] {
        &self.occurrences
    }
}

/// A positioned occurrence and its selected physical sources.
#[derive(Debug)]
pub struct SocContainerOccurrence {
    /// Logical parent occurrence and ordinal of its original child declaration.
    pub placement: Option<(usize, usize)>,
    /// The exact physical container selected for this occurrence.
    pub origin: EntryOrigin,
    /// Selection of the source descriptor within the opened container.
    pub descriptor: SocMemberSelection,
    /// Selection of the room registry within the opened container.
    pub registry: SocMemberSelection,
    /// Original stream declarations and their physical selections.
    pub streams: Vec<SocStreamInput>,
    /// Original nested placements, including failed and repeated declarations.
    pub children: Vec<SocChildInput>,
}

impl SocContainerOccurrence {
    /// Parses the retained descriptor, when a selected payload was read.
    ///
    /// # Errors
    /// Returns an error when the original descriptor has invalid XML or root.
    pub fn parsed_descriptor(
        &self,
    ) -> Result<Option<SocContainerDescriptor<'_>>, SocContainerDescriptorError> {
        self.descriptor
            .selected()
            .and_then(|member| member.payload().ok())
            .map(SocContainerDescriptor::parse)
            .transpose()
    }
}

/// A failure which interrupts capture of the whole SOC source tree.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum SocCaptureError {
    /// The requested root is not an observed physical container.
    #[error("SOC capture requires an admitted physical root container")]
    Root,
    /// Cancellation interrupted source capture.
    #[error("SOC source capture cancelled")]
    Cancelled,
    /// The Archive changed during capture.
    #[error(transparent)]
    Changed(#[from] ArchiveChanged),
    /// A selected Entry could not be bound to its physical owner.
    #[error("SOC source selection has no physical archive binding")]
    Origin,
    /// An Archive lookup could not be completed.
    #[error(transparent)]
    Index(#[from] PathIndexError),
    /// Storage for the selected source could not be allocated.
    #[error(transparent)]
    Allocation(#[from] TryReserveError),
    /// A decoder returned more bytes than the read buffer holds.
    #[error("SOC source decoder returned an invalid byte count")]
    ReadWidth,
}

#[derive(Clone, Copy)]
struct CaptureCheck<'a> {
    watch: &'a ArchiveWatch,
    cancel: &'a CancelToken,
}

impl CaptureCheck<'_> {
    fn poll(self) -> Result<(), SocCaptureError> {
        if self.cancel.is_cancelled() {
            return Err(SocCaptureError::Cancelled);
        }
        self.watch.poll()?;
        Ok(())
    }

    fn keys(self, archive: &Archive) -> Result<PathKeys, SocCaptureError> {
        PathKeys::try_new_with(archive, |_| self.poll()).map_err(soc_archive_member::path_error)
    }
}

fn scan(
    index: &PathIndex<'_>,
    origin: EntryOrigin,
    placement: Option<(usize, usize)>,
    check: CaptureCheck<'_>,
) -> Result<SocContainerOccurrence, SocCaptureError> {
    let stem = crate::interior::container_stem(origin.entry().path());
    let descriptor = soc_descriptor_member::descriptor(index, stem, || check.poll())?;
    let registry = soc_descriptor_member::registry(index.archive(), || check.poll())?;
    let parsed = soc_descriptor_member::parsed(&descriptor);
    let streams = parsed
        .as_ref()
        .map(|descriptor| soc_stream_capture::streams(descriptor, index, || check.poll()))
        .transpose()?
        .unwrap_or_default();
    let children = parsed
        .as_ref()
        .map_or_default(soc_nested_capture::declarations);
    Ok(SocContainerOccurrence {
        placement,
        origin,
        descriptor,
        registry,
        streams,
        children,
    })
}
