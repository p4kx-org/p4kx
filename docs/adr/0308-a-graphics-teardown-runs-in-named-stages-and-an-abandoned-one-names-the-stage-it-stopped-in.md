# A graphics teardown runs in named stages and an abandoned one names the stage it stopped in

Decided for issues #947 and #934. It builds on the bounded teardown of the offscreen renderer
(`Offscreen::close`, `Released`) and the exit guard of the window.

## Context

**Root cause: not found, open.** Dropping a graphics device on the real driver once never
returned. The bound that followed (idle wait, drop on a worker thread, exit guard of the window)
keeps the caller alive but does not say what the drop waits on. A new hang gets a new issue that
names the stage it reports.

## Decision

**The teardown runs in defined stages.** `Gpu::release` waits for the queue to go idle
(`Stage::Idle`). `Stage::Renderer` drops the renderer's resources and then waits, bounded, for the
deferred frees that follow, so they are attributed to it. `Stage::Queue` drops the queue and
`Stage::Device` drops the device. The `Queue` stage is a point in the sequence, not a
separate driver step: the queue and the device share their internal state. The sequence lives in
`run_stages`, whose order and stage entry a unit test checks with recording values.

**An abandoned teardown names its stage.** `Released::Abandoned` carries the `Stage` the worker was
in when the limit passed. `p4kx ui hologram` prints it in its warning and the real-adapter tests
print it on failure. `Stage` and `Released` are `#[non_exhaustive]`.

**The bounds stay.** `TEARDOWN_LIMIT` (15 s) for the caller, the worker thread for the drop, and
the exit guard of the window (status 4) for the device that eframe drops after the exit hook.
The device keeps its `Instance` and `Adapter` alive, so they are released with it.

## Evidence

On the discrete GPU (RTX 5090, Vulkan) the hang did not reproduce through the renderer's paths:
teardowns took 35 ms to 3.5 s depending on the load of the machine. None was abandoned over a
threaded run of devices created, drawn on and closed at once, over runs with the idle wait off, and over a raw wgpu device dropped with
copies in flight. That makes an unfinished readback or submission, the number of devices alive at
once, and the drop thread unlikely causes. The per-run numbers are in the pull request.

## Consequences

#947 closes with a bounded, staged teardown and no cause. #934 stays open for the stalls on the
software adapter. The ignored real-adapter tests (`P4KX_REAL_GPU_RUNS`) are the way to look for
the hang again.
