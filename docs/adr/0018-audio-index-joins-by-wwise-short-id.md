# The audio index joins ATL triggers and soundbank events by Wwise short ID

An Entity's Record names audio triggers through `GlobalResourceAudio.audioTrigger`; the ATL
definitions below `Data\Libs\GameAudio\` map each trigger to Wwise event names; soundbanks know
their events only by ID. The resolution lives in `p4kx-assembly` (`audio`), so `p4kx-audio` stays
a format crate. The audio index reads every ATL definition and every `.bnk` Entry once per open
Archive, in parallel, and keeps only names and IDs: triggers by lower-case name, and for every
event ID the soundbanks whose `HIRC` section holds an Event object with it, both in hash maps.
An event name is matched to an ID by Wwise's short ID, the 32-bit FNV-1 hash of the name in ASCII
lower case; in the live data more than 95 % of the ships' triggers reach a soundbank this way.
When two definitions name the same trigger, the first by Entry path wins.

An admitted ATL trigger retains every authored Wwise event in definition order, including
repeated event names and each localised flag. Searching event names examines this complete
list. Source Entry size, XML-node admission and the total trigger-index bound are independent
policies; result paging does not shorten a trigger’s retained source membership.

The trigger references of an Entity are read from its Record when it is asked for, not indexed
for every Entity; only the Entity's own Record tree is searched. The reverse index from trigger
to Entities that #144 needs will be built lazily, on its first use, in its own `OnceLock` next to
the audio index, so listing one Entity never pays for it.

Each Entry is read whole and dropped once parsed, below a cap per kind: 32 MiB for a soundbank
and 8 MiB for an ATL definition, against a largest live soundbank of 5.2 MiB (surveyed on LIVE,
see `docs/audio-survey.md`) and a largest live ATL definition of 1.32 MiB;
a text ATL definition may hold at most 2^20 XML nodes. The buffer is allocated for the stated size
up to the cap, so the build holds at most one capped Entry per rayon thread. Parsing a soundbank
costs about 70 bytes per `HIRC` object, and the smallest object takes 9 bytes of the bank, so a
hostile bank at the cap holds up to about 3.7 million objects, roughly 0.26 GiB while it is
parsed, per rayon thread; 64 MiB would double that, far above what any real bank needs. Entries
above the cap or that cannot be read are kept as issues; the rest of the index still builds.

## Considered Options

- Follow the `ATLPreload` of a trigger to the soundbank files its preload request names: cheaper,
  but a trigger's event often lives in a shared soundbank the preload does not name, and the
  preload says nothing about whether the event is really there.
- Index the trigger references of every Entity with the rest: makes reverse lookups possible, but
  reads every Entity's Record tree up front, seconds of work that listing one Entity never needs.

## Open Questions

- Whether to follow References from an Entity's Record to non-Entity Records one level deep, for
  triggers held in audio or effect Records the Entity refers to, is left to #147.
