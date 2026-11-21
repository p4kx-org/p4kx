// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_dataforge::{DataForgeError, Record, RecordTree};

use crate::{LoadoutError, LoadoutFileObservation, LoadoutPath, LoadoutRequest};

/// An actual source operation performed by a loadout resolver.
#[derive(Debug)]
pub enum LoadoutReadEvent<'r, 'f> {
    /// A traversal limit prevents enumerating further required source inputs.
    InventoryIncomplete,
    /// An actual required XML file read completed.
    FileRead {
        /// The original document operation whose read completed.
        observation: &'r LoadoutFileObservation,
        /// The actual retained document candidate that supplied this operation.
        request: &'r LoadoutPath,
        /// The current caller supplied before document-cache lookup.
        caller: &'r LoadoutRequest,
    },
    /// The actual intrinsic source document projection completed.
    FileProjection {
        /// The same original operation whose successful read was observed.
        observation: &'r LoadoutFileObservation,
        /// The actual retained document candidate that supplied this operation.
        request: &'r LoadoutPath,
        /// The current caller supplied before document-cache lookup.
        caller: &'r LoadoutRequest,
    },
    /// Original file and projection outcomes supplied a cached document result.
    FileReused {
        /// Immutable first-population evidence, without another source operation.
        observation: &'r LoadoutFileObservation,
        /// The actual retained document candidate that supplied this operation.
        request: &'r LoadoutPath,

        /// The current caller supplied before document-cache lookup.
        caller: &'r LoadoutRequest,
    },
    /// One original direct Include links two genuine file operations.
    FileDependency {
        /// The original including document's operation.
        parent: &'r LoadoutFileObservation,
        /// The original child operation, including an actual read or parse failure.
        observation: &'r LoadoutFileObservation,
        /// The child's original first-population candidate.
        request: &'r LoadoutPath,
        /// The genuine direct Include request selected by the parent parse.
        dependency: &'r LoadoutRequest,
    },
    /// A genuine Include is refused before a child operation is read.
    FileDependencyRefused {
        /// The original including document's operation.
        parent: &'r LoadoutFileObservation,
        /// The genuine Include request that reached this refusal.
        dependency: &'r LoadoutRequest,
        /// The actual cycle or input-limit failure.
        error: &'r LoadoutError,
    },
    /// A canonical dependency read completed before optional projection helpers run.
    CanonicalRead {
        /// The physical dependency Record.
        record: &'r Record<'f>,
        /// The actual canonical result.
        outcome: Result<&'r RecordTree<'f>, &'r DataForgeError>,
    },
    /// A caller supplied the canonical root tree instead of decoding it again.
    ProvidedTree {
        /// The physical source identity supplied by the caller.
        record: &'r Record<'f>,
        /// The supplied immutable canonical tree.
        tree: &'r RecordTree<'f>,
    },
    /// A previously read physical source supplies a cached projection.
    Reused {
        /// The original physical source behind the cache entry.
        record: &'r Record<'f>,
    },
    /// A known dependency cannot supply the required source type before canonical reading.
    Rejected {
        /// The physical dependency Record.
        record: &'r Record<'f>,
        /// The typed source-selection error.
        error: &'r LoadoutError,
    },
}
