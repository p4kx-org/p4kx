// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The egui paint callbacks that draw a [`Frame`] of the renderer inside the window's pass.

use std::sync::Arc;

use eframe::egui_wgpu::{self, wgpu};
use egui::{Color32, Rect};
use p4kx_render::{
    Colours, Frame, GpuReport, Look, Renderer, Shading, Subject, Unavailable, Uniforms,
};

use crate::theme::palette;

/// Creates the renderer on `state` and registers it with the egui renderer so paint callbacks
/// find it.
///
/// # Errors
///
/// Returns [`Unavailable`] when the device is too limited or the shaders and pipelines cannot be
/// created; the rest of the window is not affected.
pub fn install(state: &egui_wgpu::RenderState) -> Result<Arc<GpuReport>, Unavailable> {
    let renderer = Renderer::new(
        &state.device,
        &state.queue,
        &state.adapter,
        state.target_format,
    )?;
    let report = renderer.report();
    state.renderer.write().callback_resources.insert(renderer);
    Ok(report)
}

const fn bytes(colour: Color32) -> [u8; 4] {
    [colour.r(), colour.g(), colour.b(), u8::MAX]
}

const fn rgb(colour: Color32) -> [u8; 3] {
    [colour.r(), colour.g(), colour.b()]
}

/// Builds the paint callback that draws `subject` in the `look` seen through `uniforms` into
/// `rect`, a view of `size` pixels.
#[must_use]
pub(crate) fn paint_callback(
    rect: Rect,
    subject: Subject,
    uniforms: Uniforms,
    (size, look): ([u32; 2], Look),
) -> egui::PaintCallback {
    egui_wgpu::Callback::new_paint_callback(
        rect,
        SceneCallback {
            frame: Frame {
                subject,
                uniforms,
                size,
                look,
                shading: Shading::Solid,
                colours: Colours {
                    background: bytes(palette::SURFACE_INSET),
                    bones: rgb(palette::PRIMARY),
                },
            },
        },
    )
}

/// Builds the paint callback that frees the Assembly buffers of the viewer, for a window that no
/// longer shows one.
#[must_use]
pub(crate) fn release_callback() -> egui::PaintCallback {
    egui_wgpu::Callback::new_paint_callback(
        Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1.0, 1.0)),
        Release,
    )
}

struct Release;

impl egui_wgpu::CallbackTrait for Release {
    fn prepare(
        &self,
        _device: &wgpu::Device,
        _queue: &wgpu::Queue,
        _screen: &egui_wgpu::ScreenDescriptor,
        _encoder: &mut wgpu::CommandEncoder,
        resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        if let Some(renderer) = resources.get_mut::<Renderer>() {
            renderer.release();
        }
        Vec::new()
    }

    fn paint(
        &self,
        _info: egui::PaintCallbackInfo,
        _pass: &mut wgpu::RenderPass<'static>,
        _resources: &egui_wgpu::CallbackResources,
    ) {
    }
}

struct SceneCallback {
    frame: Frame,
}

impl egui_wgpu::CallbackTrait for SceneCallback {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _screen: &egui_wgpu::ScreenDescriptor,
        _encoder: &mut wgpu::CommandEncoder,
        resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        resources
            .get_mut::<Renderer>()
            .map_or_default(|renderer| renderer.prepare(device, queue, &self.frame))
    }

    fn paint(
        &self,
        _info: egui::PaintCallbackInfo,
        pass: &mut wgpu::RenderPass<'static>,
        resources: &egui_wgpu::CallbackResources,
    ) {
        if let Some(renderer) = resources.get::<Renderer>() {
            renderer.paint(pass, self.frame.subject.serial());
        }
    }
}
