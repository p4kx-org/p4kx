// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The graphics backends the window opens its wgpu instance with.
//!
//! An instance loads the driver of every backend it sees, with the capture and overlay layers
//! other programs inject into them, so the window opens Direct3D 12 only and falls back to the
//! default backends when that yields no usable adapter and device in time (ADR-0317).

use std::sync::Arc;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use eframe::egui_wgpu::WgpuSetupCreateNew;
use eframe::egui_wgpu::wgpu::{
    Adapter, Backends, DeviceDescriptor, Instance, InstanceDescriptor, PollType, PowerPreference,
    RequestAdapterOptions,
};

/// How long the Direct3D 12 probe may take; a probe that is still running then counts as failed.
const PROBE_LIMIT: Duration = Duration::from_secs(4);

/// How long the probe waits for its device to go idle before it drops it.
const TEARDOWN_LIMIT: Duration = Duration::from_millis(500);

/// Returns the backends of the window: `from_env` (`WGPU_BACKEND`) when it names some, else
/// Direct3D 12 when `dx12_usable` says it yields an adapter and a device, else `fallback`.
///
/// `from_env` is a parameter, not read here, so that a set variable skips the probe and so that
/// the order is testable.
pub(crate) fn choose(
    from_env: Option<Backends>,
    fallback: Backends,
    dx12_usable: impl FnOnce() -> bool,
) -> Backends {
    match from_env.filter(|backends| !backends.is_empty()) {
        Some(backends) => backends,
        None if dx12_usable() => Backends::DX12,
        None => fallback,
    }
}

/// Runs `probe` on its own thread and returns its answer, or `false` when it gives none within
/// `limit`. A probe that hangs keeps its thread; the process does not wait for it.
pub(crate) fn within(limit: Duration, probe: impl FnOnce() -> bool + Send + 'static) -> bool {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        sender.send(probe()).ok();
    });
    receiver.recv_timeout(limit).unwrap_or(false)
}

/// Returns whether a Direct3D 12 only instance with the power preference and device
/// descriptor of `setup`, the ones eframe opens its own device with, yields an adapter and a
/// device. Always `false` off Windows.
///
/// A custom `native_adapter_selector` of `setup` is not applied: the probe asks for the adapter
/// by power preference, which is what eframe does without a selector.
pub(crate) fn dx12_usable(setup: &WgpuSetupCreateNew) -> bool {
    if !cfg!(windows) {
        return false;
    }
    let descriptor = InstanceDescriptor {
        backends: Backends::DX12,
        flags: setup.instance_descriptor.flags,
        backend_options: setup.instance_descriptor.backend_options.clone(),
        ..InstanceDescriptor::new_without_display_handle()
    };
    let power = setup.power_preference;
    let device = Arc::clone(&setup.device_descriptor);
    within(PROBE_LIMIT, move || {
        let started = Instant::now();
        let usable = pollster::block_on(opens_device(descriptor, power, &*device));
        log::debug!(
            "Direct3D 12 probe: {} after {:?}",
            if usable { "usable" } else { "not usable" },
            started.elapsed()
        );
        usable
    })
}

/// Opens an adapter and a device on a new instance and drops device, adapter and instance in that
/// order, after the device has gone idle.
async fn opens_device(
    descriptor: InstanceDescriptor,
    power: PowerPreference,
    device_descriptor: &(dyn Fn(&Adapter) -> DeviceDescriptor<'static> + Send + Sync),
) -> bool {
    let instance = Instance::new(descriptor);
    let Ok(adapter) = instance
        .request_adapter(&RequestAdapterOptions {
            power_preference: power,
            ..RequestAdapterOptions::default()
        })
        .await
    else {
        return false;
    };
    let Ok((device, queue)) = adapter.request_device(&device_descriptor(&adapter)).await else {
        return false;
    };
    let idle = device
        .poll(PollType::Wait {
            submission_index: None,
            timeout: Some(TEARDOWN_LIMIT),
        })
        .is_ok();
    drop(queue);
    drop(device);
    drop(adapter);
    drop(instance);
    idle
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
    fn an_empty_environment_counts_as_unset() {
        assert_eq!(
            choose(Some(Backends::empty()), Backends::PRIMARY, || true),
            Backends::DX12,
            "Direct3D 12 alone"
        );
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

    #[test]
    fn a_probe_that_answers_in_time_gives_its_answer() {
        assert!(within(Duration::from_secs(5), || true), "answered true");
        assert!(!within(Duration::from_secs(5), || false), "answered false");
    }

    #[test]
    fn a_probe_that_hangs_counts_as_not_usable_after_the_limit() {
        let (release, hold) = mpsc::channel::<()>();

        let usable = within(Duration::from_millis(50), move || {
            hold.recv().ok();
            true
        });

        assert!(!usable, "the stuck probe was given up");
        drop(release);
    }
}
