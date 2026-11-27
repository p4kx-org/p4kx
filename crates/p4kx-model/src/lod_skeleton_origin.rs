// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

/// What a level-specific skeleton lookup observed before effective-bone sharing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum LodSkeletonOrigin {
    /// The level uses the model file's skeleton without a level-specific lookup.
    ModelFile,
    /// The level has no skinned meshes, so its model file was not requested.
    NotRead,
    /// The resolver returned no level-specific model file.
    MissingFile,
    /// The level-specific file had no recognised skeleton chunk.
    NoSkeleton,
    /// The level-specific file's recognised skeleton chunk declared no bones.
    EmptySkeleton,
    /// The reader's nonempty level skeleton compared equal to the model file's bones.
    EqualModel,
    /// The level-specific file supplied a distinct nonempty skeleton.
    Distinct,
}
