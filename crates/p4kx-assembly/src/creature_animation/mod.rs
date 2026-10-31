// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The animations of a creature Entity. Its `SAnimatedCharacterComponentParams` name a Mannequin
//! animation database (`.adb`), whose fragments list the clips by name; the parameter file
//! (`.chrparams`) of its skeleton names the folder, and possibly a track database, the clips are
//! stored in. [`creature_animations`] reads the clips, [`Assembly::animate`] fits them to the
//! skeleton and [`Assembly::write_glb_animated_with`] writes them.
//!
//! [`Assembly::animate`]: crate::Assembly::animate
//! [`Assembly::write_glb_animated_with`]: crate::Assembly::write_glb_animated_with

mod clip_files;
mod component;
mod database;
mod fit;
mod source;

pub(crate) use component::database_of;
pub use fit::CreatureAnimations;
pub use source::{CreatureClips, creature_animations};
