# Original effect, modulator and audio-device metadata

Wwise v154 effect share sets, custom effects and audio devices are represented
by `audio-bank-hirc-plugin`. The summary identifies the authored owner kind,
plug-in ID, exact parameter range, declared byte length, and counts of
bank-data references and state property overrides. The byte range addresses the
result's verified original bank artifact in `source.raw`. Zero bytes and a
truncated prefix have different coverage. The exporter keeps the opaque bytes in
their original representation.

The `audio-bank-hirc-plugin-bank-data` and
`audio-bank-hirc-plugin-state-override` rows use the physical bank/HIRC owner
and row ordinal. They preserve repeated entries, source IDs, variable-length
property IDs, accumulation codes and raw value words. Initial RTPC rows use
`audio-bank-hirc-plugin-rtpc` and its `-curve` and `-point` companions. Inline
State and device FX slots use the shared `audio-bank-hirc-inline-state` and
`audio-bank-hirc-node-fx` families. A device's slot count precedes its
conditional bypass byte and ordered slot rows.

`audio-bank-hirc-modulator` identifies the LFO, envelope or time owner and
reports the independent property and ranged modifier lists. The
`audio-bank-hirc-modulator-property` rows use a list index and physical
property ordinal. Unknown or repeated IDs and all value words retain their
original representation. The modulator RTPC companions carry the independent
control curves.

The `source_fields` branch shared by `bnk objects --json` and MCP exposes the
same ranges and data under `plugin` or `modulator`. Its `coverage` identifies
the original type, version and layout. Its `observed` object holds the checked
fields, and `initial_rtpc` holds the independent original graph where reached.
Both surfaces give source evidence per field. Raw original HIRC ranges remain
available for unsupported versions and malformed bodies.

[ADR-0391](adr/0391-v154-effects-modulators-and-audio-devices.md)
