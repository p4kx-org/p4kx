// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The renderer of P4KX: a model flattened into a [`Scene`], an Assembly flattened into an
//! [`AssemblyScene`] of shared meshes and placed instances, the orbit [`Camera`] that looks at
//! them, the [`Skeleton`] and its [`Pose`], and the wgpu drawing of all of it by the
//! [`Renderer`], which a window blits into its surface.

mod assembly;
mod bounds;
mod camera;
#[cfg(test)]
mod fixtures;
mod gpu;
mod rigid;
mod scene;
mod skeleton;

pub use assembly::{
    AssemblyScene, BuildError, BuildOptions, DrawList, GroupDraw, Hooks, Instance, Left,
    MAX_VIEW_ITEMS, MAX_VIEW_TEXTURE_BYTES, MAX_VIEW_TRIANGLES, MAX_VIEW_VERTICES, MIN_PART_PIXELS,
    PartInfo, Progress, VIEW_TEXTURE_MIP, hidden_mask,
};
pub use camera::{Camera, HologramView, Matrix, Step, Uniforms};
pub use gpu::{
    Adapters, AssemblyDraw, Colours, Frame, GpuReport, Hologram, Look, MAX_TEXTURE_SIDE, Offscreen,
    OffscreenError, Picture, Rejected, Released, Renderer, Shading, Stage, Subject, Unavailable,
    abandoned_teardowns, check_buffer, check_texture, target_extent,
};
pub use scene::{
    Bounds, MAX_SCENE_TRIANGLES, MAX_SCENE_VERTICES, MorphSpan, SKIN_STRIDE, Scene, SceneError,
    SkinVertex, Vertex,
};
pub use skeleton::{MAX_SKELETON_BONES, Pose, Sample, Skeleton};
