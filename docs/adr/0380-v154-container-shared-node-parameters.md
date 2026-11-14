# Version-154 container records expose the shared node parameters

Version 154 uses the same shared node base at the beginning of HIRC types 5
(Random/Sequence), 6 (Switch) and 9 (Blend/Layer). The owning container
continues with its own layout after that region. The common reader records the
original property and ranged lists, effect and metadata slots, auxiliary bus
declarations, positioning, advanced settings, inline State and RTPC regions.

The source-field iterator gives the container sequential cursors over checked
ranges from its owning bank. The ordinary container iterator continues to
supply the existing child cursor. Each Media field retains its exact source
receipt. The container's overall layout status and shared group coverage are
independent. Earlier groups remain complete when a later container-specific
field is truncated. A short shared region retains its readable original
prefix. Later groups that were never reached report unavailable coverage.

The version-qualified container records reuse the shared node property,
effect, routing, positioning, advanced settings and inline State payloads.
Container RTPC records have their own physical namespace and preserve the
existing Sound and ActorMixer record types. Container-specific decision
fields retain their existing layout and raw source ranges.

The shared prefix is established by [wwiser's Random/Sequence reader](https://github.com/bnnm/wwiser/blob/6d4b5bfb45afc52cda9d68fbbe0cbd97de0e26a3/wwiser/parser/wparser.py#L1501),
[Switch reader](https://github.com/bnnm/wwiser/blob/6d4b5bfb45afc52cda9d68fbbe0cbd97de0e26a3/wwiser/parser/wparser.py#L1592),
[Layer reader](https://github.com/bnnm/wwiser/blob/6d4b5bfb45afc52cda9d68fbbe0cbd97de0e26a3/wwiser/parser/wparser.py#L2054)
and [StarBreaker's container readers](https://github.com/diogotr7/StarBreaker/blob/7d3f01e76307354bbc56a64a70eb5167b1189b64/crates/starbreaker-wwise/src/hirc/containers.rs#L33).
