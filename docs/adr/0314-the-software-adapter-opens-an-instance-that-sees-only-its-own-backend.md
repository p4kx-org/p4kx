# The software adapter opens an instance that sees only its own backend

Decided for issue #934. It follows ADR-0308, which left the stalls on the software adapter open.

## Context

On Windows, `wgpu::Instance` creation with all backends loads the driver of every backend, so a
software run (WARP) also loaded the Vulkan loader, the NVIDIA and AMD Vulkan drivers and the
capture and overlay layers that other programs inject into them. The Windows error report of a
failed test process lists those modules and the exception `0x0000087d` (decimal 2173) raised from
`KERNELBASE.dll`: the "exit code 2173" of #934 is that unhandled exception, not an exit status of
the test code.

## Evidence

Loop runs of the `p4kx-render` lib test binary, four processes at once: with all backends two
runs crashed with 2173 and runs of 10 s to 16 s (normal: 3 s to 5 s) were common; with Direct3D 12
only, none crashed and none was slow, and instance creation fell from about 0.9 s to 0.12 s. The
ADR-0308 stage reporting never fired on the software adapter: the time went before the first
draw. The numbers are in the pull request.

## Decision

`Adapters::Software` opens its instance with `Backends::DX12` on Windows and with all backends
elsewhere (lavapipe is a Vulkan adapter). The software fallback of `Adapters::Best` does the same.
The test that opens a raw device uses the same instance, and a Windows test checks that the
instance lists WARP and no other backend.

## Consequences

The 130 s silent stall and the injected layer that raises 0x87D are not identified: no debugger or
dump was available. #934 closes with this change; if either problem comes back, it gets a new
issue that references #934, with the Windows error report. A Vulkan software
adapter on Windows is no longer used for `Software`.
