// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The 3D viewer of the Model tab: the [`p4kx_render`] scenes, the orbit [`Camera`] that looks at
//! them and the wgpu drawing of them, which this module adapts to the egui window, plus the
//! [`Playback`] of a skeleton's animations.

#[cfg(test)]
mod fixtures;
mod model_info_provenance;
mod paint;
mod playback;
mod preview;
mod preview_animations;
mod support;

pub use model_info_provenance::ModelInfoProvenance;
pub(crate) use p4kx_render::{AssemblyDraw, Look, Subject, hidden_mask};
pub use p4kx_render::{
    AssemblyScene, Bounds, BuildError, BuildOptions, Camera, DrawList, GpuReport, GroupDraw, Hooks,
    Instance, Left, MAX_SCENE_TRIANGLES, MAX_SCENE_VERTICES, MAX_SKELETON_BONES, MAX_TEXTURE_SIDE,
    MAX_VIEW_ITEMS, MAX_VIEW_TEXTURE_BYTES, MAX_VIEW_TRIANGLES, MAX_VIEW_VERTICES, MIN_PART_PIXELS,
    Matrix, MorphSpan, PartInfo, Pose, Progress, Rejected, SKIN_STRIDE, Scene, SceneError,
    Skeleton, SkinVertex, Step, Unavailable, Uniforms, VIEW_TEXTURE_MIP, Vertex, check_buffer,
    check_texture, target_extent,
};
pub use paint::install;
pub(crate) use paint::{paint_callback, release_callback};
pub use playback::{Control, Playback};
pub use preview::{MODEL_FILE_LIMIT, ModelPreview, ModelStatus, is_model_path};
pub use preview_animations::{AnimationCountProvenance, PREVIEW_ANIMATION_BOUNDS};
pub use support::{Problem, ViewerSupport};
