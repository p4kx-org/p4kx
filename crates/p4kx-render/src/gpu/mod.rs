// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The wgpu side of the renderer: a scene drawn into an offscreen picture that a host blits into
//! its surface, or reads back.

mod assembly;
mod device;
mod frame;
mod hologram;
#[cfg(test)]
mod hologram_tests;
mod limits;
mod offscreen;
#[cfg(test)]
mod offscreen_tests;
mod pipelines;
#[cfg(test)]
mod probe;
mod readback;
#[cfg(test)]
mod real_gpu_tests;
mod report;
mod resources;
mod shading;
mod skeleton;
#[cfg(test)]
mod skeleton_tests;
mod slot;
mod straight;
mod target;
mod teardown;
#[cfg(test)]
mod tests;

use std::sync::Arc;

use assembly::AssemblyBuffers;
use device::{settle, supported_samples};
pub use limits::{MAX_TEXTURE_SIDE, Rejected, check_buffer, check_texture, target_extent};
pub use offscreen::{Adapters, Offscreen, OffscreenError, Picture};
use pipelines::{BLIT_BYTES, FRAME_BYTES, Pipelines};
pub use report::{GpuReport, Unavailable};
use resources::{MaterialTexture, MeshBuffers};
pub use shading::{Hologram, Shading};
use slot::Slot;
use target::Target;
pub use teardown::{Released, Stage, abandoned_teardowns};

use super::assembly::{AssemblyScene, DrawList};
use super::camera::Uniforms;
use super::scene::Scene;
use super::skeleton::Pose;

const BASE_TEXTURE: u64 = 0;
const MIN_TEXTURE_SIDE: u32 = 2048;
const MIN_BUFFER_BYTES: u64 = 64 * 1024 * 1024;
const WHITE: [u8; 4] = [255; 4];

/// The GPU resources of the renderer: pipelines, the cached meshes and textures of the scene
/// that was drawn last, and the picture it was drawn into.
#[derive(Debug)]
pub struct Renderer {
    pipelines: Pipelines,
    frame_buffer: wgpu::Buffer,
    frame_group: wgpu::BindGroup,
    blit_buffer: wgpu::Buffer,
    blit_sampler: wgpu::Sampler,
    textures: Slot<MaterialTexture>,
    mesh: Slot<MeshBuffers>,
    assembly: Slot<AssemblyBuffers>,
    material_sampler: wgpu::Sampler,
    target: Option<Target>,
    drawn: Option<Drawn>,
    posed: u64,
    samples: u32,
    linear_target: bool,
    report: Arc<GpuReport>,
}

/// How the model is drawn on top of the scene: in which pose, and whether with its skeleton.
#[derive(Debug, Clone, Default)]
pub struct Look {
    /// The bone matrices of the pose to draw, or `None` for the bind pose.
    pub pose: Option<Arc<Pose>>,
    /// The width in device pixels of the skeleton lines, or `None` to leave the skeleton out.
    pub bones: Option<f32>,
}

/// The colours of a picture, as sRGB bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Colours {
    /// What the picture is cleared to, with its alpha; the alpha is only meaningful to a picture
    /// that is read back.
    pub background: [u8; 4],
    /// The colour of the skeleton lines.
    pub bones: [u8; 3],
}

impl Default for Colours {
    fn default() -> Self {
        Self {
            background: [0x14, 0x16, 0x1a, 0xff],
            bones: [0xff, 0xff, 0xff],
        }
    }
}

/// What the picture in the target shows, so a repaint with the same input skips the pass.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Drawn {
    pose: u64,
    list: u64,
    bones: Option<f32>,
    serial: u64,
    uniforms: Uniforms,
    size: [u32; 2],
    colours: Colours,
    shading: Shading,
}

/// What a frame draws: one model, or the instances of an Assembly that a draw list picked.
#[derive(Debug, Clone)]
pub enum Subject {
    /// A model flattened into one mesh.
    Model(Arc<Scene>),
    /// An Assembly with the instances of this frame.
    Assembly(AssemblyDraw),
}

/// An Assembly and the draw list of the frame.
#[derive(Debug, Clone)]
pub struct AssemblyDraw {
    /// The shared meshes and the placed parts.
    pub scene: Arc<AssemblyScene>,
    /// The instances of this frame.
    pub list: Arc<DrawList>,
}

impl Subject {
    /// Returns the serial number of the scene the subject draws.
    #[must_use]
    pub fn serial(&self) -> u64 {
        match self {
            Self::Model(scene) => scene.serial(),
            Self::Assembly(view) => view.scene.serial(),
        }
    }
}

/// One picture to draw: the `subject` in the `look` seen through `uniforms`, in a view of `size`
/// pixels.
#[derive(Debug, Clone)]
pub struct Frame {
    /// What to draw.
    pub subject: Subject,
    /// The camera.
    pub uniforms: Uniforms,
    /// The size of the view in pixels.
    pub size: [u32; 2],
    /// The pose and the skeleton overlay.
    pub look: Look,
    /// The background and overlay colours.
    pub colours: Colours,
    /// How an Assembly is shaded; a model is always drawn solid.
    pub shading: Shading,
}

impl Renderer {
    /// Creates the renderer on `device`, drawing the blit of its picture in `target_format`.
    ///
    /// # Errors
    ///
    /// Returns [`Unavailable`] when the device is too limited or the shaders and pipelines cannot
    /// be created.
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        adapter: &wgpu::Adapter,
        target_format: wgpu::TextureFormat,
    ) -> Result<Self, Unavailable> {
        let limits = device.limits();
        if limits.max_texture_dimension_2d < MIN_TEXTURE_SIDE {
            return Err(Unavailable::TextureSide(limits.max_texture_dimension_2d));
        }
        if limits.max_buffer_size < MIN_BUFFER_BYTES {
            return Err(Unavailable::BufferSize(limits.max_buffer_size));
        }
        let samples = supported_samples(adapter);
        let report = Arc::new(GpuReport::default());
        let lost = Arc::clone(&report);
        device.set_device_lost_callback(move |_reason, _message| lost.device_lost());
        let validation = device.push_error_scope(wgpu::ErrorFilter::Validation);
        let gpu = Self::create(device, queue, samples, target_format, report);
        settle(validation.pop())
            .flatten()
            .map_or(gpu, |error| Err(Unavailable::Setup(error.to_string())))
    }

    /// Returns what the renderer learned about the device and the scenes.
    #[must_use]
    pub fn report(&self) -> Arc<GpuReport> {
        Arc::clone(&self.report)
    }

    /// Returns whether the device can draw with shading; solid shading always works.
    #[must_use]
    pub const fn supports(&self, shading: &Shading) -> bool {
        match shading {
            Shading::Solid => true,
            Shading::Hologram(_) => self.pipelines.hologram.is_some(),
        }
    }

    /// Frees the Assembly buffers, for a host that no longer shows one.
    pub fn release(&mut self) {
        self.assembly.clear();
        self.drawn = None;
    }

    fn create(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        samples: u32,
        target_format: wgpu::TextureFormat,
        report: Arc<GpuReport>,
    ) -> Result<Self, Unavailable> {
        let pipelines = Pipelines::new(device, samples, target_format);
        let buffer = |label: &str, size: u64| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        };
        let frame_buffer = buffer("p4kx-viewer-frame", FRAME_BYTES);
        let blit_buffer = buffer("p4kx-viewer-blit", BLIT_BYTES);
        let frame_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("p4kx-viewer-frame"),
            layout: &pipelines.frame_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: frame_buffer.as_entire_binding(),
            }],
        });
        let material_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("p4kx-viewer-material"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            ..wgpu::SamplerDescriptor::default()
        });
        let blit_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("p4kx-viewer-blit"),
            ..wgpu::SamplerDescriptor::default()
        });
        let white = MaterialTexture::upload(
            device,
            queue,
            &pipelines.material_layout,
            &material_sampler,
            (1, 1, &WHITE),
            &[],
        )
        .map_err(|error| Unavailable::Setup(error.to_string()))?;
        let mut textures = Slot::default();
        textures.store(BASE_TEXTURE, white);
        Ok(Self {
            pipelines,
            frame_buffer,
            frame_group,
            blit_buffer,
            blit_sampler,
            textures,
            mesh: Slot::default(),
            assembly: Slot::default(),
            material_sampler,
            target: None,
            drawn: None,
            posed: 0,
            samples,
            linear_target: target_format.is_srgb(),
            report,
        })
    }
}
