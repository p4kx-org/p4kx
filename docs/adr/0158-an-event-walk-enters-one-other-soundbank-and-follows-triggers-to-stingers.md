# An event walk enters one other soundbank and follows Trigger actions to stingers

Decided for issue #597. Amends the resolution rules of ADR-0115 and builds on ADR-0033.
The bank-entry and retention rules are superseded by ADR-0362. The historical decision and LIVE
measurements below describe the earlier bank-count policy, not the current reader's limits.

## Decision

- **Play and PlayEvent actions leave the soundbank.** A Play action whose target its own soundbank
  lacks is followed into the soundbank it names; a PlayEvent action whose Event its own soundbank
  lacks, into a soundbank that holds the Event (the index keeps the holders by Event ID,
  `event_holders`, and `AudioIndex::event_soundbanks` lists them). The soundbank is
  chosen by the language rules of ADR-0033 (`select_holders`): soundbanks that are not localised,
  and the localised ones of the first language of the `LanguageChoice` that has any; with no
  language chosen, the first in path order. A soundbank with the named ID in no selected language
  is a `no_localisation` break.
- **With no language chosen, a reference leaving the soundbank follows the first language only.**
  This amends ADR-0033, which has `LanguageChoice::any` follow every language: the start
  soundbank of an event is still followed in every language, but beyond it the walk reads one
  other soundbank, and following each language would read one soundbank per language and
  reference. The soundbanks of the other languages are not listed; a caller that needs one asks
  with `LanguageChoice::new`.
- **A Trigger action (`0x1Bxx`) fires a music trigger.** It is followed to every stinger that
  names the trigger as its trigger ID, to the segment the stinger plays, and from there as any
  segment. The stingers of all soundbanks are kept in the audio index by trigger ID, with the
  soundbank that declares them, because on LIVE none of the 33 Trigger actions lies in the
  soundbank that declares the stinger it fires. A stinger without a segment (ID 0) is not kept.
  The index keeps every non-null stinger from each admitted soundbank, each
  (trigger, soundbank, segment) once, in source order. Source-byte admission and the budgets of
  an individual resolution walk are independent of this complete membership. The stingers are
  followed in the soundbanks the
  language rules select, as for an Event; only when none of them is selected is it a
  `no_localisation` break. A Trigger action whose trigger no stinger names is a `missing` break
  with the trigger ID.
- **Indexed stingers bind their declaring soundbank.** An equal object ID in the current soundbank does not replace the stinger’s indexed bank. Segment children are followed locally after entering that selected bank.
- **Cycle identity.** Active objects are identified by their actually selected soundbank index position and object ID. Equal IDs in different Entries remain distinct. Target lookup precedes cycle checking; the soundbank-entry policy governs which targets may be loaded.
- **One extra soundbank read per event and soundbank.** A walk starts in the soundbank that holds
  its Event and reads at most one other soundbank, on the first reference that leaves the first.
  It then moves between those two without reading again; a reference into a third soundbank is
  an `other_soundbank` break. The events of one start soundbank share the last other soundbank
  read, so consecutive events that leave to the same soundbank read it once. A soundbank that
  cannot be read is an `unreadable` break that names that soundbank, once attempted never retried
  by the same walk. A Play action that names a soundbank the Archive does not hold is an
  `other_soundbank` break too.
- **Chains are bounded.** At most `MAX_ACTION_CHAIN` (8) PlayEvent and Trigger actions follow one
  another on a path; the next is a `chain_too_long` break (new kind). The walk keeps the path
  rule of ADR-0021, which cuts a reference back to an object on the path whatever soundbank holds
  it, so two soundbanks that post each other end in a `cycle` break, and its object budget and
  path depth count the objects of both soundbanks.

## Measured on LIVE

The 61 911 triggers, resolved with no language chosen:

| | before | after |
|---|---|---|
| events resolved to a sound | 54 406 | 54 428 |
| variants | 295 940 | 302 180 |
| variants through music | 0 | 5 982 |
| `other_soundbank` breaks | 238 | 2 |
| `missing` breaks | 507 | 523 |

The 2 remaining `other_soundbank` breaks name a soundbank the Archive does not hold. The 16 new
`missing` breaks are Trigger actions with no stinger and stingers whose segment is in a third
soundbank.

## Considered Options

- Read every soundbank a walk meets: unbounded reads of untrusted, up to 32 MiB soundbanks per
  event; not done.
- Look for a Trigger action's stinger in the soundbank of the action only: finds none of the 33
  on LIVE.
- Scan every soundbank for the stinger at resolution time: reads the whole Archive per trigger;
  the index keeps the few stingers instead.
