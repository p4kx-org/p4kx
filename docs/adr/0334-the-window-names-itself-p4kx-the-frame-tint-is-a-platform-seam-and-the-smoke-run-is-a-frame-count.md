# The window names itself p4kx, the frame tint is a platform seam and the smoke run is a frame count

Decided for issue #1020 (part of #827, Linux). The GUI keeps its look; this ADR records what the
Linux desktop needs around it.

## Application id

On Linux the window sets `io.github.p4kx_org.p4kx` as its application id (`window::APP_ID`): the
Wayland `app_id` and the X11 `WM_CLASS`. The Flatpak, installed desktop file and icons share that
identity from `.config/linux-packaging.json`. The source desktop file lives in `packaging/linux/`;
the packager names its installed copy and a test keeps `StartupWMClass` equal to the window ID.
Windows retains `p4kx` as its application id. The package contract is described in
[Linux packages](../linux-packages.md).

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
portal's on Linux. `eframe/clipboard` is enabled for `cfg(any(unix, windows))`, preserving Linux
and macOS behavior while providing the Windows system backend through arboard and clipboard-win.
Existing copy actions and editable fields use that backend. A configuration test establishes the
feature selection; cross-program behavior requires a source-built native GUI and a separate
Windows editor, including Unicode, multiline, empty and busy-clipboard cases
([verification contract](../windows-clipboard.md)).

## Third-party notices

Licence generation selects every native target from `.config/platforms.json`. The generated
notices cover the final clipboard feature graph, including Windows arboard/clipboard-win and
Unix clipboard/windowing dependencies. Exact resolved versions remain in `Cargo.lock`.

## Smoke run

`P4KX_SMOKE_FRAMES=<n>` makes `p4kx-gui` draw `n` frames, skip the update check and close itself.
It is a plain environment variable, not a test feature, so the packages of #1022 can run the
installed binary with it. The test `the_window_starts_draws_frames_and_exits` runs it with an
empty profile; a smoke run on an installed binary still saves the window settings to the profile
on exit. An absolute `P4KX_FOLDERS_ROOT` isolates those production folder lookups. The test is
`#[ignore]`d; native CI selects it explicitly
on Windows and under `xvfb-run` on Linux with software rendering.

## Not verified here

The Linux window itself (X11 and Wayland sessions, HiDPI factors, drag and drop, the portal
dialogs, the clipboard between programs) was written without a Linux machine; CI covers the
build, the smoke run under Xvfb and the render tests on lavapipe, not a Wayland compositor.
