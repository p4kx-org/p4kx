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

The `p4kx-render` lib test binary, run in a loop with four processes at once on the 2026-10-31
development machine, with all backends:

- about 1000 runs: 2 crashes with 2173, both after 14 s and 19 s runs (a normal run takes 3 to 5 s),
  and 18 runs of 10 s to 16 s;
- the instance creation took about 0.9 s per device under that load.

With only Direct3D 12 for the software adapter, 450 runs: no crash, no run above 8 s, the
slowest teardown 0.8 s, instance creation about 0.12 s. The same binary with all backends again
(switched by an environment variable, since removed): 7 runs of 10 s to 16 s in 450.

The stage reporting of ADR-0308 never fired on the software adapter: no teardown was abandoned,
and the slow runs spent their time before the first draw. The 130 s silent stall was not seen
again; it came before the watchdog of #939 and is not proven to be the same fault.

## Decision

`Adapters::Software` opens its instance with `Backends::DX12` on Windows and with all backends
elsewhere (lavapipe is a Vulkan adapter). `Adapters::Best` is unchanged. The test that opens a raw
device uses the same instance. A Windows test checks that the instance lists no other backend.

## Consequences

The root cause of the crash itself (which injected layer raises 0x87D) is not identified: no
debugger or dump was available. Not loading those drivers is what the evidence supports. A new
crash gets a new issue with its Windows error report.
