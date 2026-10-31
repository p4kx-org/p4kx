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

**Tests.** All headless GUI tests, the ones of the `it` and `real_archive` test binaries and the
unit test of the running package export window, take their wgpu setup from one shared file,
`crates/p4kx-gui/tests/support/software_wgpu.rs`. `software_wgpu_setup()` is the setup of
`egui_kittest`, which prefers a software adapter, with the instance limited to Direct3D 12 on
Windows (WARP) and to all backends elsewhere (lavapipe is a Vulkan adapter). A Windows test checks
that this instance lists WARP and no other backend.

**Window.** The owner chose to open Direct3D 12 only, with a fallback. `window::native_options`
asks `backends::choose` for the backends of the instance, in this order:

1. `WGPU_BACKEND`, when set, as before;
2. Direct3D 12 alone, when a Direct3D 12 only instance yields an adapter and a device;
3. otherwise eframe's default backends.

The probe runs on Windows only; elsewhere the default backends stay.

## Consequences

A GUI test no longer loads the Vulkan loader or the Vulkan drivers on Windows, and neither does the
window on a machine with a working Direct3D 12. A new headless GUI test takes
`software_wgpu_setup()`; a test that calls `default_wgpu_setup()` directly is a regression. A
machine whose Direct3D 12 yields no adapter or device opens the window as before, after one extra
short probe at start-up. A Direct3D 12 that passes the probe and fails later is not retried.
