// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The wgpu setup of the headless GUI tests opens an instance of the backends it needs only.
//!
//! The tests assume `WGPU_BACKEND` is not set, as the setup inherits it.

use eframe::egui_wgpu::WgpuSetup;
use p4kx_testkit::wgpu_setup::software_wgpu_setup;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn the_setup_creates_a_new_instance_with_some_backend() -> TestResult {
    let WgpuSetup::CreateNew(setup) = software_wgpu_setup() else {
        return Err("the setup creates its own instance".into());
    };

    let backends = setup.instance_descriptor.backends;
    assert!(!backends.is_empty(), "the instance sees a backend");
    Ok(())
}

#[cfg(windows)]
#[test]
fn the_setup_lists_a_cpu_adapter_and_none_of_another_backend() -> TestResult {
    use eframe::egui_wgpu::wgpu::{Backend, Backends, DeviceType, Instance};
    use p4kx_testkit::watchdog::arm;

    let guard = arm("the_setup_lists_a_cpu_adapter_and_none_of_another_backend");
    guard.phase("enumerate adapters");
    let WgpuSetup::CreateNew(setup) = software_wgpu_setup() else {
        return Err("the setup creates its own instance".into());
    };

    let adapters = pollster::block_on(
        Instance::new(setup.instance_descriptor).enumerate_adapters(Backends::all()),
    );

    assert!(
        adapters
            .iter()
            .any(|adapter| adapter.get_info().device_type == DeviceType::Cpu),
        "an adapter of device type Cpu, which is WARP on Direct3D 12, is listed: {adapters:?}"
    );
    assert!(
        adapters
            .iter()
            .all(|adapter| adapter.get_info().backend == Backend::Dx12),
        "only Direct3D 12 adapters are listed: {adapters:?}"
    );
    Ok(())
}
