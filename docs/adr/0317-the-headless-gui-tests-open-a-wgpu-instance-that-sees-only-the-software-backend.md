# The headless GUI tests open a wgpu instance that sees only the software backend

Decided for issue #998. It amends ADR-0314, which limited the instance of the software adapter of
`p4kx-render`.

## Context

ADR-0314 found that a `wgpu::Instance` with all backends loads the driver of every backend on
Windows, with the capture and overlay layers that other programs inject into them, and that one of
them crashed test processes with 0x87D under load. The GUI tests render through
`egui_kittest::default_wgpu_setup()`, which keeps every native backend, so they were exposed to
the same layers.

## Decision

All headless GUI tests, the ones of the `it` and `real_archive` test binaries and the unit test of
the running package export window, take their wgpu setup from one shared file,
`crates/p4kx-gui/tests/support/software_wgpu.rs`. `software_wgpu_setup()` is the setup of
`egui_kittest`, which prefers a software adapter, with the instance limited to Direct3D 12 on
Windows (WARP) and to all backends elsewhere (lavapipe is a Vulkan adapter). A Windows test checks
that this instance lists WARP and no other backend.

The window of the application keeps the setup of eframe unchanged by this decision; whether to
restrict its backends is a product decision that is taken separately.

## Consequences

A GUI test no longer loads the Vulkan loader or the Vulkan drivers on Windows. A new headless GUI
test takes `software_wgpu_setup()`; a test that calls `default_wgpu_setup()` directly is a
regression.
