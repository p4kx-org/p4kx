// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_model::{PrefabObjectSource, PrefabSource};

use super::PrefabFile;

/// A Prefab row in one original archive Entry.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub struct PrefabLocation<'a> {
    /// The exact loaded library.
    pub file: &'a PrefabFile,
    /// The row's index in that library's parsed Prefabs.
    pub prefab_index: usize,
}

impl<'a> PrefabLocation<'a> {
    /// Returns the original row positions when the parser retained them.
    #[must_use]
    pub fn source(self) -> Option<&'a PrefabSource> {
        self.file.source()?.prefabs.get(self.prefab_index)
    }
}

/// A flattened object backed by one original XML element.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub struct PrefabObjectLocation<'a> {
    /// The owning physical library and Prefab row.
    pub prefab: PrefabLocation<'a>,
    /// The index of the surviving flattened object within that row.
    pub object_index: usize,
}

impl<'a> PrefabObjectLocation<'a> {
    /// Returns the object's exact selector and transform sites when retained.
    #[must_use]
    pub fn source(self) -> Option<&'a PrefabObjectSource> {
        self.prefab
            .source()?
            .object_sources
            .as_ref()?
            .get(self.object_index)
    }
}

#[derive(Debug, Clone)]
pub(super) struct OwnedPrefabLocation {
    pub(super) file: Arc<PrefabFile>,
    pub(super) prefab_index: usize,
}

impl OwnedPrefabLocation {
    pub(super) fn location(&self) -> PrefabLocation<'_> {
        PrefabLocation {
            file: &self.file,
            prefab_index: self.prefab_index,
        }
    }
}

/// The actual lookup branch for an authored nested Prefab reference.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum PrefabLookupRoute {
    /// A name selected the target without a nonempty GUID.
    Name,
    /// The named target was accepted after comparing its Id with the authored GUID.
    NameAndGuid,
    /// The GUID index selected the target.
    GuidIndex,
    /// The named target was retained after the GUID index supplied no usable match.
    NameFallback,
    /// Name resolution found no target and there was no nonempty GUID.
    UnmatchedName,
    /// Neither name nor GUID resolution supplied a usable target.
    UnmatchedGuid,
}

/// Source-side facts evaluated while resolving a nested Prefab reference.
#[derive(Debug, Clone)]
pub struct PrefabLinkSource {
    pub(super) route: PrefabLookupRoute,
    pub(super) target: Option<OwnedPrefabLocation>,
    pub(super) compared_name: Option<OwnedPrefabLocation>,
    pub(super) surveyed_names: Vec<Arc<PrefabFile>>,
    pub(super) failed_entry_indices: Vec<usize>,
}

impl PrefabLinkSource {
    /// Returns the lookup branch responsible for this decision.
    #[must_use]
    pub const fn route(&self) -> PrefabLookupRoute {
        self.route
    }

    /// Returns the selected target, including a found target later refused by cycle or depth checks.
    #[must_use]
    pub fn target(&self) -> Option<PrefabLocation<'_>> {
        self.target.as_ref().map(OwnedPrefabLocation::location)
    }

    /// Returns the named candidate that an eventual GUID-index match superseded.
    #[must_use]
    pub fn compared_name(&self) -> Option<PrefabLocation<'_>> {
        self.compared_name
            .as_ref()
            .map(OwnedPrefabLocation::location)
    }

    /// Returns readable libraries searched for the authored name without a matching row.
    pub fn surveyed_names(&self) -> impl ExactSizeIterator<Item = &PrefabFile> {
        self.surveyed_names.iter().map(Arc::as_ref)
    }

    /// Returns failed read positions in the owning set's Archive entry directory.
    #[must_use]
    pub fn failed_entry_indices(&self) -> &[usize] {
        &self.failed_entry_indices
    }
}

/// One authored instance selection and its evaluated lookup.
#[derive(Debug, Clone)]
pub struct PrefabSelectionSource {
    pub(super) owner: OwnedPrefabLocation,
    pub(super) object_index: usize,
    pub(super) lookup: Arc<PrefabLinkSource>,
}

impl PrefabSelectionSource {
    /// Returns the original object supplying the reference and the local transform.
    #[must_use]
    pub fn object(&self) -> PrefabObjectLocation<'_> {
        PrefabObjectLocation {
            prefab: self.owner.location(),
            object_index: self.object_index,
        }
    }

    /// Returns the actual target selection evidence.
    #[must_use]
    pub fn lookup(&self) -> &PrefabLinkSource {
        self.lookup.as_ref()
    }
}

/// An entered nested Prefab occurrence and its containing route.
#[derive(Debug, Clone)]
pub struct PrefabInstanceSource {
    pub(super) selection: Arc<PrefabSelectionSource>,
    pub(super) parent: Option<Arc<Self>>,
}

impl PrefabInstanceSource {
    /// Returns the object selecting this Prefab and the interpreted lookup.
    #[must_use]
    pub fn selection(&self) -> &PrefabSelectionSource {
        self.selection.as_ref()
    }

    /// Returns the next outward containing instance, when there is one.
    #[must_use]
    pub fn parent(&self) -> Option<&Self> {
        self.parent.as_deref()
    }
}

/// The source-side context of one emitted Prefab issue.
#[derive(Debug, Clone)]
pub struct PrefabIssueSource {
    pub(super) owner: OwnedPrefabLocation,
    pub(super) object_index: Option<usize>,
    pub(super) route: Option<Arc<PrefabInstanceSource>>,
    pub(super) link: Option<Arc<PrefabLinkSource>>,
}

impl PrefabIssueSource {
    /// Returns the originating Prefab row.
    #[must_use]
    pub fn prefab(&self) -> PrefabLocation<'_> {
        self.owner.location()
    }

    /// Returns the authored Object that led to the issue, if it concerns one Object.
    #[must_use]
    pub fn object(&self) -> Option<PrefabObjectLocation<'_>> {
        self.object_index.map(|object_index| PrefabObjectLocation {
            prefab: self.prefab(),
            object_index,
        })
    }

    /// Returns the containing instance route.
    #[must_use]
    pub fn instance_route(&self) -> Option<&PrefabInstanceSource> {
        self.route.as_deref()
    }

    /// Returns the evaluated link when this issue arose during target resolution.
    #[must_use]
    pub fn link(&self) -> Option<&PrefabLinkSource> {
        self.link.as_deref()
    }
}

/// The original root and distinct entered selectors behind one resolved Prefab.
#[derive(Debug, Clone)]
pub struct PrefabResolutionSource {
    pub(super) root: OwnedPrefabLocation,
    pub(super) entered: Vec<Arc<PrefabSelectionSource>>,
    pub(super) issues: Vec<PrefabIssueSource>,
}

impl PrefabResolutionSource {
    /// Returns the root Prefab position.
    #[must_use]
    pub fn root(&self) -> PrefabLocation<'_> {
        self.root.location()
    }

    /// Returns distinct authored selections that entered a Prefab, including empty targets.
    pub fn entered(&self) -> impl ExactSizeIterator<Item = &PrefabSelectionSource> {
        self.entered.iter().map(Arc::as_ref)
    }

    /// Returns issue evidence in the order of the emitted issues.
    #[must_use]
    pub fn issues(&self) -> &[PrefabIssueSource] {
        &self.issues
    }
}
