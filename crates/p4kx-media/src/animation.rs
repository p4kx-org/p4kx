// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

mod binding_payload;
mod binding_projection;
mod controller_projection;
pub(crate) mod dba_metadata_lookup;
mod dba_projection;
mod directional_paths;
mod image_payload;
mod image_projection;
mod origin;
mod payload;
mod projection;
mod provider;
mod schema;
pub(crate) mod selection_path;
mod skeleton_selection;

pub(crate) use provider::AnimationProvider;
pub(crate) use schema::bind_record_schema;
pub(crate) use skeleton_selection::{SkeletonSelection, validate_skeleton_path};
