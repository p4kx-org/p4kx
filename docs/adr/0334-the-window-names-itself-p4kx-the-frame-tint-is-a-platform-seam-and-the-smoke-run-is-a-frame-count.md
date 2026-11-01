# The window names itself p4kx, the frame tint is a platform seam and the smoke run is a frame count

Decided for issue #1020 (part of #827, Linux). The GUI keeps its look; this ADR records what the
Linux desktop needs around it.

## Application id

The window sets `p4kx` as its application id (`window::APP_ID`): the Wayland `app_id` and the X11
`WM_CLASS`. A desktop environment matches it to `p4kx.desktop` for the icon and the name in its
task switcher. The desktop file lives in `packaging/linux/` with its `StartupWMClass`, and a test
keeps the two equal. The reverse-DNS id of the Flatpak is decided in #1022; the desktop file name
follows it there.

## Frame tint

`frame::tint(&CreationContext)` is the one call of `main`. On Windows it forwards to the DWM
module (`cfg(windows)`, `unsafe_code = "deny"` in the manifest, unchanged); elsewhere it is a
`const` no-op, because a Linux window manager draws its own frame and the theme reaches the client
area through `theme::apply` alone. The `cfg` is inside the seam, not at the call site.

## Backends

The instance opens `WGPU_BACKEND` when set, else Direct3D 12 when it works (Windows only, ADR-0317),
else eframe's defaults: Vulkan first with a GL fallback (the `gles` feature of wgpu comes with
eframe's `wgpu` feature). A test pins Vulkan and GL in the default set.

## Dialogs and clipboard

`rfd` keeps its defaults, which include `xdg-portal` and `wayland`, so the file dialogs are the
portal's on Linux. `eframe/clipboard` is enabled for `cfg(unix)`, which covers macOS too: without
it egui-winit has no system clipboard backend and keeps copied text inside the process. Windows
keeps its current dependency set and therefore has that limitation today (#1040).

## Third-party notices

`about.toml` scopes `THIRD-PARTY-NOTICES.md` to `x86_64-pc-windows-msvc`, so the crates only the
Linux build compiles (`smithay-clipboard`, both `smithay-client-toolkit` versions, `calloop` and
the `wayland-protocols` crates, among others) are not listed. No Linux build is shipped before
#1022, which must add the Linux target to the notices (noted there).

## Smoke run

`P4KX_SMOKE_FRAMES=<n>` makes `p4kx-gui` draw `n` frames, skip the update check and close itself.
It is a plain environment variable, not a test feature, so the packages of #1022 can run the
installed binary with it. The test `the_window_starts_draws_frames_and_exits` runs it with an
empty profile; a smoke run on an installed binary still saves the window settings to the profile
on exit, which an empty `HOME` avoids. The test is `#[ignore]`d and runs in the ubuntu-24.04 job under `xvfb-run` on lavapipe.

## Not verified here

The Linux window itself (X11 and Wayland sessions, HiDPI factors, drag and drop, the portal
dialogs, the clipboard between programs) was written without a Linux machine; CI covers the
build, the smoke run under Xvfb and the render tests on lavapipe, not a Wayland compositor.
