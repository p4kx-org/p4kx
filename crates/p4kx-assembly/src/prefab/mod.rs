// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use thiserror::Error;

mod resolution_source;
mod resolve;
mod resolved;
mod set;

pub use resolution_source::{
    PrefabInstanceSource, PrefabIssueSource, PrefabLinkSource, PrefabLocation, PrefabLookupRoute,
    PrefabObjectLocation, PrefabResolutionSource, PrefabSelectionSource, PrefabUnreadableEvidence,
    PrefabUnreadableSource,
};
pub use resolved::{ResolvedObject, ResolvedPrefab};
pub use set::{PrefabFile, PrefabSet};

/// The deepest chain of nested prefabs that is entered; a prefab below it is listed as an
/// object that was not expanded.
pub const MAX_PREFAB_DEPTH: usize = 16;

/// The most objects one resolved prefab holds; further objects are left out.
pub const MAX_PREFAB_OBJECTS: usize = 1 << 19;

/// The most objects one resolution looks at, expanded prefabs counted every time they are
/// entered; the resolution stops there.
pub const MAX_PREFAB_VISITS: usize = 1 << 22;

/// The most issues one resolution lists; the last one says the rest is left out.
pub const MAX_PREFAB_ISSUES: usize = 1 << 12;

/// The largest prefab library that is read, 64 MiB.
pub const MAX_PREFAB_LIBRARY_LEN: u64 = 64 << 20;

/// A prefab or library that cannot be resolved.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum PrefabError {
    /// The Archive holds no library of this name below `Data\Prefabs`.
    #[error("there is no prefab library {name}")]
    NoSuchLibrary {
        /// The name asked for.
        name: String,
    },
    /// The library holds no prefab of this name.
    #[error("the library {library} holds no prefab {name}")]
    NoSuchPrefab {
        /// The library.
        library: String,
        /// The prefab name asked for.
        name: String,
    },
    /// The library file cannot be read.
    #[error("the library {library} cannot be read: {reason}")]
    Unreadable {
        /// The library.
        library: String,
        /// Why it cannot be read.
        reason: String,
    },
    /// The Archive needs a key that is missing or wrong.
    #[error("the Archive cannot be read")]
    Archive(#[source] Box<dyn std::error::Error + Send + Sync>),
}

/// What went wrong with an object of a prefab while it was resolved.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum PrefabIssueKind {
    /// The object instances a prefab that no library holds; it is kept unexpanded.
    #[error("the prefab {prefab} is not in any library")]
    Missing {
        /// The `PrefabName` the object names.
        prefab: String,
    },
    /// The object instances a prefab that is being expanded further up; it is kept unexpanded.
    #[error("the prefab {prefab} contains itself")]
    Cycle {
        /// The `PrefabName` the object names.
        prefab: String,
    },
    /// The chain of nested prefabs is longer than [`MAX_PREFAB_DEPTH`]; the object is kept
    /// unexpanded.
    #[error("the prefabs are nested deeper than {limit} levels")]
    TooDeep {
        /// The limit.
        limit: usize,
    },
    /// The library that should hold the prefab cannot be read.
    #[error("the library {library} cannot be read: {reason}")]
    LibraryUnreadable {
        /// The library.
        library: String,
        /// Why it cannot be read.
        reason: String,
    },
    /// The `PrefabGUID` of the object is not the `Id` of the prefab its name finds; the prefab
    /// of the name is used.
    #[error("the prefab {prefab} has the Id {found}, not {named}")]
    GuidMismatch {
        /// The `PrefabName` the object names.
        prefab: String,
        /// The `PrefabGUID` the object names.
        named: String,
        /// The `Id` of the prefab that was found.
        found: String,
    },
    /// Objects of the prefab were left out because their transform is not usable.
    #[error("{count} objects of the prefab {prefab} have no usable transform and are left out")]
    ObjectsLeftOut {
        /// The prefab.
        prefab: String,
        /// How many.
        count: usize,
    },
    /// Objects of the prefab are attached to others; their transform is not relative to the
    /// prefab and is used as it stands.
    #[error(
        "{count} objects of the prefab {prefab} are attached to others; their transforms are used as they stand"
    )]
    Attached {
        /// The prefab.
        prefab: String,
        /// How many.
        count: usize,
    },
    /// The prefab holds more objects than [`MAX_PREFAB_OBJECTS`]; the rest is left out.
    #[error("the prefab holds more than {limit} objects; the rest is left out")]
    TooManyObjects {
        /// The limit.
        limit: usize,
    },
    /// The resolution found more than [`MAX_PREFAB_ISSUES`] issues; the rest is not listed.
    #[error("more than {limit} issues; the rest is not listed")]
    TooManyIssues {
        /// The limit.
        limit: usize,
    },
    /// The resolution looked at more than [`MAX_PREFAB_VISITS`] objects and stopped.
    #[error("the prefab expands to more than {limit} objects; the rest is left out")]
    TooManyVisits {
        /// The limit.
        limit: usize,
    },
}

/// Something that could not be resolved for an object of a prefab.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrefabIssue {
    /// The names of the nested prefab objects from the resolved prefab down to the one the
    /// object sits in, joined by `/`; empty for the resolved prefab itself.
    pub container: String,
    /// The object's name; empty when the issue concerns a whole prefab.
    pub name: String,
    /// What went wrong.
    pub kind: PrefabIssueKind,
}

impl PrefabIssueKind {
    /// Returns a stable code for the kind, e.g. `missing`, for scripts; the message is text for
    /// people.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Missing { .. } => "missing",
            Self::Cycle { .. } => "cycle",
            Self::TooDeep { .. } => "too_deep",
            Self::LibraryUnreadable { .. } => "library_unreadable",
            Self::GuidMismatch { .. } => "guid_mismatch",
            Self::ObjectsLeftOut { .. } => "objects_left_out",
            Self::Attached { .. } => "attached",
            Self::TooManyObjects { .. } => "too_many_objects",
            Self::TooManyVisits { .. } => "too_many_visits",
            Self::TooManyIssues { .. } => "too_many_issues",
        }
    }
}
