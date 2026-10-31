// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The wgpu setup of the headless GUI tests, behind the `wgpu` feature so that the other users of
//! this crate do not build eframe.

use eframe::egui_wgpu::WgpuSetup;
#[cfg(windows)]
use eframe::egui_wgpu::wgpu::Backends;
use egui_kittest::wgpu::default_wgpu_setup;

/// Returns the setup of `egui_kittest`, which prefers a software adapter, with the instance
/// limited on Windows to Direct3D 12 (WARP).
///
/// An instance loads the driver of every backend it sees, with the capture and overlay layers
/// other programs inject into them. Elsewhere every backend stays, as the software adapter there
/// (lavapipe) is a Vulkan one. The setup inherits `WGPU_BACKEND` from the environment: a value
/// that names no Direct3D 12 backend is kept as it is.
#[must_use]
pub fn software_wgpu_setup() -> WgpuSetup {
    let mut setup = default_wgpu_setup();
    #[cfg(windows)]
    if let WgpuSetup::CreateNew(create) = &mut setup {
        let limited = create.instance_descriptor.backends & Backends::DX12;
        if !limited.is_empty() {
            create.instance_descriptor.backends = limited;
        }
    }
    setup
}
