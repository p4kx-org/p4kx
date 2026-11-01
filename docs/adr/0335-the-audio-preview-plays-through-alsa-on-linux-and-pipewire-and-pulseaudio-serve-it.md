# The audio preview plays through ALSA on Linux, and PipeWire and PulseAudio serve it

Decided for issue #1021, part of #827 (full Linux compatibility). Builds on ADR-0017 (rodio plays
the audio preview).

## Decision

- Linux needs no second audio code path. rodio 0.23 with its `playback` feature opens the system's
  default output device through cpal 0.18, whose Linux backend is ALSA. PipeWire and PulseAudio
  provide that `default` device through `pipewire-alsa` and `pulseaudio-alsa`, which desktop
  distributions install; the native cpal `pipewire` and `pulseaudio` features are not enabled.
- Building needs the ALSA headers (`libasound2-dev`). `.github/actions/setup-rust` installs them for
  every Linux job and CONTRIBUTING names them.
- A host without an output device (a CI runner, a container) is a normal case: `DeviceOutput` opens
  the device on its own thread and reports the opening error, typically
  `DeviceSinkError::NoDevice`, through the `FailureReporter`; the Audio tab shows the reason.
  Nothing panics and the window never waits. The opener is injectable, so a unit test fails it with
  `NoDevice` on every host. A Linux-only smoke test plays through the real default output, which
  typically has no device on the `ubuntu-24.04` job, and only checks that nothing panics.
- Decoding and conversion (`Pcm`, `decode`, the transport) never touch a device; their tests drive a
  fake `AudioOutput`.
- Windows is unchanged: the same code opens WASAPI.

## Considered Options

- cpal's native `pipewire` feature: needs `libpipewire-dev` at build time and a PipeWire daemon at
  run time, so it would fail on PulseAudio-only and ALSA-only systems that the ALSA path serves.
- cpal's native `pulseaudio` feature: needs the PulseAudio client libraries and is redundant with
  `pulseaudio-alsa`.
- Picking the backend at run time: more code and a larger dependency tree for no gain over the ALSA
  `default` device.

## Consequences

- Start, stop and seek of a decoded Wwise sound through the PipeWire-PulseAudio and ALSA backends
  were not checked on a Linux desktop when this was written; the owner's manual check is open and is
  recorded here when done.
