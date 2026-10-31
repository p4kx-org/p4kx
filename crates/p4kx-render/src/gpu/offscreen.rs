// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::Write;
use std::sync::Arc;

use thiserror::Error;

use super::device::{any_instance, block_on_for, software_instance};
use super::readback::{BYTES_PER_PIXEL, DEVICE_WAIT};
use super::teardown::{Gpu, Released};
use super::{Frame, GpuReport, Renderer, Unavailable};

/// Which graphics adapter to draw on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Adapters {
    /// The most capable adapter, or the software one when there is no other.
    #[default]
    Best,
    /// Only the software adapter (WARP, lavapipe), which gives a picture that does not depend on the graphics card.
    Software,
}

/// Why a picture could not be drawn.
#[derive(Debug, Error)]
pub enum OffscreenError {
    /// No graphics adapter, not even a software one, is available.
    #[error("no graphics adapter is available")]
    NoAdapter(#[source] wgpu::RequestAdapterError),
    /// The adapter refused to create a device.
    #[error("the graphics device could not be created")]
    Device(#[source] wgpu::RequestDeviceError),
    /// The device is too limited for the renderer.
    #[error(transparent)]
    Unavailable(#[from] Unavailable),
    /// The picture has no pixels.
    #[error("the picture must be at least one pixel wide and high")]
    Empty,
    /// The picture is larger than the device draws.
    #[error("the picture is {width} x {height} pixels, the device draws at most {limit} a side")]
    TooLarge {
        /// The wanted width.
        width: u32,
        /// The wanted height.
        height: u32,
        /// The longest side of a texture the device supports.
        limit: u32,
    },
    /// The device cannot draw the shading that was asked for.
    #[error("the graphics device cannot draw the hologram shading")]
    Shading,
    /// The device was lost while drawing.
    #[error("the graphics device was lost")]
    Lost,
    /// Drawing failed, for example for lack of memory.
    #[error("drawing failed, for example for lack of memory")]
    Failed,
    /// The scene is too large for the buffers of the device.
    #[error("the scene is too large for the buffers of the graphics device")]
    Rejected,
    /// The finished picture could not be read back.
    #[error("the picture could not be read back")]
    Readback(#[source] Box<dyn std::error::Error + Send + Sync>),
    /// The graphics driver did not answer within the time limit.
    #[error("the graphics driver did not answer within {} seconds", DEVICE_WAIT.as_secs())]
    TimedOut,
}

/// A finished picture: straight-alpha RGBA, eight bits a channel, the top row first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Picture {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

impl Picture {
    /// Returns the picture of `rgba` pixels, or `None` when the bytes are not four per pixel.
    #[must_use]
    pub fn from_rgba(width: u32, height: u32, rgba: Vec<u8>) -> Option<Self> {
        let wanted = u64::from(width) * u64::from(height) * u64::from(BYTES_PER_PIXEL);
        (u64::try_from(rgba.len()).ok() == Some(wanted)).then_some(Self {
            width,
            height,
            rgba,
        })
    }

    /// Returns the width in pixels.
    #[must_use]
    pub const fn width(&self) -> u32 {
        self.width
    }

    /// Returns the height in pixels.
    #[must_use]
    pub const fn height(&self) -> u32 {
        self.height
    }

    /// Returns the pixels as four bytes each: red, green, blue, alpha.
    #[must_use]
    pub fn rgba(&self) -> &[u8] {
        &self.rgba
    }

    /// Writes the picture as a PNG.
    ///
    /// # Errors
    ///
    /// Returns the encoder's error when `writer` fails.
    pub fn write_png(&self, writer: impl Write) -> Result<(), png::EncodingError> {
        let mut encoder = png::Encoder::new(writer, self.width, self.height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.write_header()?.write_image_data(&self.rgba)
    }
}

/// The renderer on a device made without a window: it draws a [`Frame`] and reads the picture
/// back.
#[derive(Debug)]
pub struct Offscreen {
    gpu: Option<Gpu>,
    report: Arc<GpuReport>,
    adapter: String,
    software: bool,
}

impl Offscreen {
    /// Opens a graphics device on the chosen `adapters` and sets the renderer up on it.
    ///
    /// # Errors
    ///
    /// Returns [`OffscreenError`] when there is no adapter, the device cannot be created or it
    /// is too limited for the renderer.
    pub fn new(adapters: Adapters) -> Result<Self, OffscreenError> {
        let instance = match adapters {
            Adapters::Best => any_instance(),
            Adapters::Software => software_instance(),
        };
        let adapter = request(&instance, adapters)?;
        let info = adapter.get_info();
        let (device, queue) = block_on_for(
            adapter.request_device(&wgpu::DeviceDescriptor {
                label: Some("p4kx-offscreen"),
                required_limits: adapter.limits(),
                ..wgpu::DeviceDescriptor::default()
            }),
            DEVICE_WAIT,
        )
        .ok_or(OffscreenError::TimedOut)?
        .map_err(OffscreenError::Device)?;
        let renderer = Renderer::new(
            &device,
            &queue,
            &adapter,
            wgpu::TextureFormat::Rgba8UnormSrgb,
        )?;
        let report = renderer.report();
        Ok(Self {
            gpu: Some(Gpu {
                renderer,
                queue,
                device,
            }),
            report,
            adapter: format!("{} ({:?})", info.name, info.backend),
            software: info.device_type == wgpu::DeviceType::Cpu,
        })
    }

    /// Returns the name and the backend of the adapter the pictures are drawn on.
    #[must_use]
    pub fn adapter_name(&self) -> &str {
        &self.adapter
    }

    /// Returns whether the adapter is a software one (WARP, lavapipe).
    #[must_use]
    pub const fn is_software(&self) -> bool {
        self.software
    }

    /// Returns what the renderer learned about the device and the scenes.
    #[must_use]
    pub fn report(&self) -> Arc<GpuReport> {
        Arc::clone(&self.report)
    }

    /// Draws `frame` and returns the picture.
    ///
    /// # Errors
    ///
    /// Returns [`OffscreenError`] when the size is empty or beyond the device, the device is
    /// lost, the scene does not fit its buffers or the drawing or the read-back fails.
    pub fn draw(&mut self, frame: &Frame) -> Result<Picture, OffscreenError> {
        let Some(gpu) = self.gpu.as_mut() else {
            return Err(OffscreenError::Lost);
        };
        let [width, height] = frame.size;
        if width == 0 || height == 0 {
            return Err(OffscreenError::Empty);
        }
        let limit = gpu.device.limits().max_texture_dimension_2d;
        if width > limit || height > limit {
            return Err(OffscreenError::TooLarge {
                width,
                height,
                limit,
            });
        }
        if !gpu.renderer.supports(&frame.shading) {
            return Err(OffscreenError::Shading);
        }
        let serial = frame.subject.serial();
        let commands = gpu.renderer.prepare(&gpu.device, &gpu.queue, frame);
        gpu.queue.submit(commands);
        let report = &self.report;
        if report.is_lost() {
            return Err(OffscreenError::Lost);
        }
        if report.is_rejected(serial) {
            return Err(OffscreenError::Rejected);
        }
        if report.has_failed(serial) {
            return Err(OffscreenError::Failed);
        }
        gpu.read_back(serial)
    }
}

impl Offscreen {
    /// Releases the device: waits for the queue to go idle and drops the GPU objects on a worker
    /// thread, so that a driver that never answers cannot block the caller for longer than the
    /// teardown limit. Dropping an `Offscreen` does the same and discards the outcome.
    #[must_use]
    pub fn close(mut self) -> Released {
        self.release()
    }

    fn release(&mut self) -> Released {
        self.gpu.take().map_or(Released::Dropped, Gpu::release)
    }
}

impl Drop for Offscreen {
    fn drop(&mut self) {
        self.release();
    }
}

fn request(instance: &wgpu::Instance, adapters: Adapters) -> Result<wgpu::Adapter, OffscreenError> {
    let ask = |fallback| {
        block_on_for(
            instance.request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: fallback,
                compatible_surface: None,
                ..wgpu::RequestAdapterOptions::default()
            }),
            DEVICE_WAIT,
        )
        .ok_or(OffscreenError::TimedOut)
    };
    match adapters {
        Adapters::Software => ask(true)?,
        Adapters::Best => match ask(false)? {
            Ok(adapter) => Ok(adapter),
            Err(_) => ask(true)?,
        },
    }
    .map_err(OffscreenError::NoAdapter)
}
