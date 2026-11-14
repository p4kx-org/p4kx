# HIRC decoding reaches music objects, bank references and node parents; triggers are found by field name

Decided for issue #278, on top of ADR-0021 (the bounded `HIRC` walk) and ADR-0018 (the audio index). The resolution rules
below are amended by ADR-0158 for Trigger actions and ADR-0362 for indexed bank references,
active ownership and per-event failure retention.

## Decoding

`p4kx-audio` decodes, in bank version 154 only, besides Sound, Action, Event and the random,
sequence, switch and blend containers:

- the music segment (children, meter, stingers, duration, markers with their null-terminated
  names), the music track (sources, clips, track kind, the switch group of a switch track), the
  music switch container (children, transition rules skipped, the leaves of its decision tree) and
  the music playlist (children, items with their loop counts);
- the parent ID of a Sound from its node base parameters, which must end the body;
- for a Play action (`0x04xx`) the soundbank ID and type it names. `0x21xx` (PlayEvent) and
  `0x1Bxx` (Trigger) are told apart by `ActionTarget::is_play_event` and `is_trigger`.

The layouts were read from wwiser and StarBreaker (MIT) and then checked against all 2386 live
soundbanks (LIVE, all version 154): every object of those types decodes to exactly its last byte,
the same rule as for containers. A body that does not is kept raw as
`HircValue::Malformed` with the error; an object of a bank above version 154 is
`UnsupportedObjectLayout`, one below stays `Raw`. The music payloads are boxed so the size of a
`HircObject` stays that of a Sound. Floats in the music objects make `HircValue` and `HircObject`
lose `Eq` and `Hash`.

All counts and offsets are untrusted: a list is never allocated beyond what the remaining body can
hold, every skip is checked arithmetic, a decision tree is walked with an explicit stack whose
entries plus the nodes already visited never exceed the number of nodes (a node listed as a child
twice, or a node listing more children than nodes are left, ends the object as malformed before
anything is pushed), and a name that has no terminator ends the object as truncated. The Sound `cacheID` rule is `hirc::has_cache_id`, also used for the clips and sources of
a music track.

## Resolution

The walk of ADR-0021 follows a music playlist, switch container and segment to their children and a
music track to its sources, each a variant whose last path step is the track. Every source counts
against the object budget. A PlayEvent action is followed to the Event it posts in the same
soundbank; the path rule cuts an event that posts itself. A Play action that names another
soundbank as the holder of a target its own soundbank lacks, and a PlayEvent action whose Event
only other soundbanks hold (found through `AudioIndex::event_soundbanks`), are a broken link
`other_soundbank` (new kind) instead of `missing`; the walk does not enter that soundbank.

On LIVE the broken links of the trigger events changed from 554 `missing` to 507 `missing` and 236
`other_soundbank`: the 507 are Play actions whose target is in no soundbank, the 236 are 47 Play
actions and 189 PlayEvent actions (2 of them inside a posted event). The earlier jump to 696
`missing` was these PlayEvent targets, which were not followed before and are now reported.

## Trigger discovery

Until now Entities named triggers through `GlobalResourceAudio.audioTrigger` only. Measured on LIVE over all
117 709 Records: an `audioTrigger` string field outside `GlobalResourceAudio` does not exist (the
wider rule StarBreaker uses finds nothing more), but 183 further trigger names (180 of them ATL
triggers) stand in string fields whose name contains `audioTrigger`: 37 names (36 in the ATL) in
the `audioTriggerName` of the melee weapon components of Entities, 146 in the foot-lift and
heel-land fields of foley footstep definitions, which are no Entities. Trigger discovery now takes
those fields too (`TriggerScope::AudioTriggerFields`, the default of `trigger_references`); the
old rule stays available as `TriggerScope::GlobalResourceAudio`.

## Considered Options

- Follow a Play action into the soundbank it names: 47 of the 58 615 live Play actions name another
  soundbank that holds their target, but a soundbank ID is held by one soundbank per language, so
  the walk would need the language rules of ADR-0033 for a second soundbank and a second read per
  event. Not done; the break says which soundbank to look in.
- Scan every `u32` of an object body for known IDs, as scdatatools does: finds ID-like numbers in
  floats and sizes; not used.
- Decode the node base parameters of every object type: the parent graph is only needed for
  Sounds and containers today.
