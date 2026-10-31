// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The graphics backends the window opens its wgpu instance with.
//!
//! An instance loads the driver of every backend it sees, with the capture and overlay layers
//! other programs inject into them, so the window opens Direct3D 12 only and falls back to the
//! default backends when that yields no usable adapter and device (ADR-0317).

use eframe::egui_wgpu::wgpu::{
    Backends, DeviceDescriptor, Instance, InstanceDescriptor, RequestAdapterOptions,
};

/// Returns the backends of the window: `from_env` (`WGPU_BACKEND`) when set, else Direct3D 12
/// when `dx12_usable` says it yields an adapter and a device, else `fallback`.
pub(crate) fn choose(
    from_env: Option<Backends>,
    fallback: Backends,
    dx12_usable: impl FnOnce() -> bool,
) -> Backends {
    match from_env {
        Some(backends) => backends,
        None if dx12_usable() => Backends::DX12,
        None => fallback,
    }
}

/// Returns whether a Direct3D 12 only instance yields an adapter and a device; always `false`
/// off Windows.
pub(crate) fn dx12_usable() -> bool {
    cfg!(windows) && pollster::block_on(opens_device(Backends::DX12))
}

async fn opens_device(backends: Backends) -> bool {
    let instance = Instance::new(InstanceDescriptor {
        backends,
        ..InstanceDescriptor::new_without_display_handle()
    });
    let Ok(adapter) = instance
        .request_adapter(&RequestAdapterOptions::default())
        .await
    else {
        return false;
    };
    adapter
        .request_device(&DeviceDescriptor::default())
        .await
        .is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::cell::Cell;

    #[test]
    fn the_environment_wins_without_asking_whether_direct3d_12_works() {
        let asked = Cell::new(false);

        let chosen = choose(Some(Backends::VULKAN), Backends::PRIMARY, || {
            asked.set(true);
            true
        });

        assert_eq!(chosen, Backends::VULKAN, "the environment's backends");
        assert!(!asked.get(), "the probe did not run");
    }

    #[test]
    fn direct3d_12_is_chosen_when_it_works() {
        assert_eq!(
            choose(None, Backends::PRIMARY, || true),
            Backends::DX12,
            "Direct3D 12 alone"
        );
    }

    #[test]
    fn the_fallback_is_chosen_when_direct3d_12_does_not_work() {
        assert_eq!(
            choose(None, Backends::PRIMARY | Backends::GL, || false),
            Backends::PRIMARY | Backends::GL,
            "the fallback backends"
        );
    }
}
