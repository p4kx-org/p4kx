# Read original v154 effects, modulators and audio devices

Decided for #1112. ADR-0250 defines the bank hierarchy layout statuses. ADR-0357
defines field-level input provenance.

## Decision

Admit the original bodies of effect share sets, custom effects, LFOs, envelopes,
audio devices and time modulators for version 154. Dispatch properties and
ranged modifiers by their authored list positions. Retain value words as
integers representing the original bits. Parameter codes, accumulation,
interpolation and bypass bytes retain their original values.

An audio device owns a full effect base and a trailing owned-effect slot list.
The device list starts at the slot count. Its bypass byte is present when the
count is positive. Every effect base includes a size-prefixed opaque parameter
block, bank-data mappings, initial RTPC, inline State and original property
overrides.

The shared audio readers bound the parameter range and all counts by the
physical owner. Media stores an exact range into the verified original bank
artifact under `source.raw`, with separate receipts for the authored size and
the bytes. ObjectView exposes the same byte coordinates. A prefix retains every
admitted field and the original remaining extent. Unknown versions report an
unsupported layout. Original bank artifacts and the separate HIRC occurrence
rows keep all bytes.

The playback resolver treats these families as unsupported
targets. Authored bank references contribute their own stored IDs.

## Source evidence

The [pinned wwiser parser](https://github.com/bnnm/wwiser/blob/24eb88f3b2a99d54c1062d8438a7bd04be88335d/wwiser/parser/wparser.py)
specifies the common effect prefix, the full effect base before device slots,
and the shared modulator layout. The pinned StarBreaker effect parser (`crates/starbreaker-wwise/src/hirc/bus.rs` at `7d3f01e76307354bbc56a64a70eb5167b1189b64`)
and modulator parser (`crates/starbreaker-wwise/src/hirc/modulator.rs` at `7d3f01e76307354bbc56a64a70eb5167b1189b64`)
independently describe those parts. The StarBreaker device candidate (`crates/starbreaker-wwise/src/hirc/misc.rs` at `7d3f01e76307354bbc56a64a70eb5167b1189b64`)
would read the slot count immediately after the bank mappings. That candidate
left source bytes after its declared fields in all six sampled devices.

An independent cursor checked 16 CRC-verified original soundbanks per Build.
LIVE 13165946 had 231 complete effect share sets and PTU 13189690 had 232.
Each had 408 custom effects, three devices, ten LFOs, two envelopes and
64 time modulators. Every checked body reached its exact owner end. Multiple
device slots and populated inline State bindings and groups have synthetic
witnesses.
