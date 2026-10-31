// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The wgpu setup of the headless GUI tests, shared by the test binaries that include this file.

use eframe::egui_wgpu::WgpuSetup;
use eframe::egui_wgpu::wgpu::Backends;
use egui_kittest::wgpu::default_wgpu_setup;

/// The backends a headless test needs: on Windows only Direct3D 12 (WARP), because an instance
/// loads the driver of every backend it sees, with the capture and overlay layers injected into
/// them. Elsewhere every backend stays, as the software adapter there (lavapipe) is a Vulkan one.
#[cfg(windows)]
pub(crate) const SOFTWARE_BACKENDS: Backends = Backends::DX12;
#[cfg(not(windows))]
pub(crate) const SOFTWARE_BACKENDS: Backends = Backends::all();

/// Returns the setup of `egui_kittest`, which prefers a software adapter, limited to
/// [`SOFTWARE_BACKENDS`].
pub(crate) fn software_wgpu_setup() -> WgpuSetup {
    match default_wgpu_setup() {
        WgpuSetup::CreateNew(mut setup) => {
            setup.instance_descriptor.backends &= SOFTWARE_BACKENDS;
            WgpuSetup::CreateNew(setup)
        }
        other @ WgpuSetup::Existing(_) => other,
    }
}
