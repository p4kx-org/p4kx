// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The wgpu setup of the headless GUI tests opens an instance of the backends it needs only.

use crate::software_wgpu::{SOFTWARE_BACKENDS, software_wgpu_setup};

use eframe::egui_wgpu::WgpuSetup;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn the_setup_creates_a_new_instance_limited_to_the_software_backends() -> TestResult {
    let WgpuSetup::CreateNew(setup) = software_wgpu_setup() else {
        return Err("the setup creates its own instance".into());
    };

    let backends = setup.instance_descriptor.backends;
    assert!(
        !backends.is_empty() && SOFTWARE_BACKENDS.contains(backends),
        "the instance sees the software backends only: {backends:?}"
    );
    Ok(())
}

#[cfg(windows)]
mod windows {
    use super::{TestResult, WgpuSetup, software_wgpu_setup};

    use std::future::Future;
    use std::pin::pin;
    use std::sync::Arc;
    use std::task::{Context, Poll, Wake, Waker};
    use std::thread::{self, Thread};

    use eframe::egui_wgpu::wgpu::{Backend, Backends, DeviceType, Instance};
    use p4kx_testkit::watchdog::arm;

    struct Unpark(Thread);

    impl Wake for Unpark {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }
    }

    /// Runs `future` on this thread; the test watchdog ends a run that never finishes.
    fn block_on<F: Future>(future: F) -> F::Output {
        let mut future = pin!(future);
        let waker = Waker::from(Arc::new(Unpark(thread::current())));
        let mut context = Context::from_waker(&waker);
        loop {
            if let Poll::Ready(output) = future.as_mut().poll(&mut context) {
                return output;
            }
            thread::park();
        }
    }

    #[test]
    fn the_setup_lists_a_warp_adapter_and_none_of_another_backend() -> TestResult {
        let guard = arm("the_setup_lists_a_warp_adapter_and_none_of_another_backend");
        guard.phase("enumerate adapters");
        let WgpuSetup::CreateNew(setup) = software_wgpu_setup() else {
            return Err("the setup creates its own instance".into());
        };

        let adapters =
            block_on(Instance::new(setup.instance_descriptor).enumerate_adapters(Backends::all()));

        assert!(
            adapters
                .iter()
                .any(|adapter| adapter.get_info().device_type == DeviceType::Cpu),
            "WARP is listed: {adapters:?}"
        );
        assert!(
            adapters
                .iter()
                .all(|adapter| adapter.get_info().backend == Backend::Dx12),
            "only Direct3D 12 adapters are listed: {adapters:?}"
        );
        Ok(())
    }
}
