# The GUI opens Direct3D 12 first and its headless tests only the software backend

Decided for issue #998. It amends ADR-0314, which limited the instance of the software adapter of
`p4kx-render`.

## Context

ADR-0314 found that a `wgpu::Instance` with all backends loads the driver of every backend on
Windows, with the capture and overlay layers that other programs inject into them, and that one of
them crashed test processes with 0x87D under load. The GUI tests render through
`egui_kittest::default_wgpu_setup()`, which keeps every native backend, and the window of the
application uses eframe's default backends, so both were exposed to the same layers.

## Decision

**Tests.** All headless GUI tests take their wgpu setup from `p4kx_testkit::wgpu_setup::
software_wgpu_setup()`. It is the setup of `egui_kittest`, which prefers a software adapter, with
the instance limited on Windows to Direct3D 12 (WARP) and left at all backends elsewhere (lavapipe
is a Vulkan adapter). The module sits behind the `wgpu` feature of the testkit, so no other user of
the testkit builds eframe; `p4kx-gui` enables it as a dev-dependency. The setup inherits
`WGPU_BACKEND`: a value that names no Direct3D 12 backend is kept. A Windows test checks that the
instance lists an adapter of device type Cpu (WARP on Direct3D 12) and no other backend.

**Window.** The owner chose to open Direct3D 12 only, with a fallback. `window::native_options`
asks `backends::choose` for the backends of the instance, in this order:

1. `WGPU_BACKEND`, when it names a backend, as before;
2. Direct3D 12 alone, when a Direct3D 12 only instance yields an adapter and a device;
3. otherwise eframe's default backends.

The probe of step 2 runs on Windows only, on its own thread, and the window waits for it at most
4 seconds; a probe that is still running then counts as failed and its thread is left behind. It
uses the power preference and the device descriptor of the `WgpuSetupCreateNew` of the native
options, so a hybrid-GPU machine probes the adapter eframe opens. A custom adapter selector would
not be applied; eframe sets none. The probe waits at most 500 ms for the device to go idle, then
drops the queue, device, adapter and instance in that order, inside its thread, and logs its
duration at debug level.

## Consequences

A GUI test no longer loads the Vulkan loader or the Vulkan drivers on Windows, and neither does the
window on a machine with a working Direct3D 12. A new headless GUI test takes
`software_wgpu_setup()`; a test that calls `default_wgpu_setup()` directly is a regression. A
machine whose Direct3D 12 yields no adapter or device, or hangs, opens the window as before after a
probe of at most 4 seconds. A Direct3D 12 that passes the probe and fails later is not retried.
