// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! A single admitted owned-value traversal and its privately minted source sites.

#[path = "original_value_frame.rs"]
mod frame;
#[path = "owned_value_walk.rs"]
mod owned_walk;
#[path = "original_value_path.rs"]
mod path;
#[path = "original_value_tokens.rs"]
mod tokens;
#[path = "owned_value_visit.rs"]
mod visit;
#[path = "original_value_walk.rs"]
mod walk;

pub(crate) use owned_walk::{OwnedValueEvent, OwnedValueWalk};
pub(crate) use tokens::{
    OriginalContainerKind, OriginalErrorParameter, OriginalValueEdge, OriginalValuePart,
};
pub(crate) use visit::OwnedValueVisit;
pub(crate) use walk::{OriginalValueEvent, OriginalValueFact, OriginalValueSite};

#[cfg(test)]
#[path = "original_value_walk_fixture.rs"]
pub(super) mod fixture;
#[cfg(test)]
#[path = "owned_value_walk_tests.rs"]
mod owned_tests;
#[cfg(test)]
#[path = "original_value_walk_tests.rs"]
mod tests;
