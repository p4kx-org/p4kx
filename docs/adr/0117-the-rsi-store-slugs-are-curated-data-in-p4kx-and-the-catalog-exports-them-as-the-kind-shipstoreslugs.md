# The RSI store slugs are curated data in P4KX, and the catalog exports them as the kind `shipStoreSlugs`

Decided for issue #452, by the owner on 2026-10-25. Builds on ADR-0002 and ADR-0054.

The Basetool matches a player's hangar, which comes from the RSI pledge store, against the ships of
the catalog. The store identifies a ship by a slug (`Aurora-Mk-I-ES`); the Archive does not carry
it, and the class names (`RSI_Aurora_GS_ES`) do not match it: in the Basetool report 95 of 253 slugs
had no matching class name. `variantOf` (#325) groups cosmetic variants but does not map a slug.

## Decision

- **The mapping lives in P4KX.** `crates/p4kx-export/data/ship-store-slugs.json` maps a slug, with
  the RSI ship id and the store name, to the `className` and `guid` of a catalog ship. It is curated
  data, not extracted data, and it is compiled into the tool. The Basetool keeps no copy, so the
  mapping and the GUIDs it points to change in one repository, in one commit.
- **It is exported as a kind of the one catalog,** `shipStoreSlugs`, additively within version 1
  (ADR-0054): a new kind with its own `completeness` entry, no change to an existing kind. An
  entry has `slug`, `rsiId`, `storeName`, `className`, `shipGuid` and `retired`. Entries are ordered
  by `slug`.
- **Only identifiers are kept:** slug, RSI id, store name, class name and GUID. No description,
  price or media, in the file or in the catalog; the store name is a short name.
- **Matching never guesses.** A slug is mapped by name, manufacturer and variant against the
  `player` ships of the `LIVE` Archive. An ambiguous name stays unmapped. The rules are in
  `docs/catalog.md`.
- **A slug that maps nothing is not exported,** and neither are the slugs of ships the game does not
  have. A slug whose ship no longer exists stays in the file marked `retired`, and is exported
  flagged with a `null` `shipGuid` and `className`, so the Basetool can tell a retired ship from an
  unknown slug.
- **A mapping that no longer resolves is stale.** A mapped, not retired slug whose GUID is no ship
  of the catalog, or whose ship has another class name, is not exported. `p4kx catalog` prints it
  and `--json` lists it in `store_slugs.stale`. A real-Archive test fails while any exists, so a
  patch that removes or renames a ship is noticed when the tests run on the new Build.
- **A refresh is done by a human.** The tool fetches nothing. The public store list is fetched
  once by a person, reduced to identifiers, and new slugs are added to the file unmapped; the
  matching, the stale list and the test then say what remains. The steps are in
  `docs/catalog.md`.

### Reviewed pledge production status

Catalog v3 also carries `shipStorePledges`, keyed by RSI ship ID. It keeps the exact slug,
short name, RSI manufacturer code and production status of each reviewed non-flight-ready
pledge. Its snapshot records the official source, date of capture, source digest, candidate
review date, curated revision and review scope. It is bundled and exported for every Channel
and Filter. The store identity refresh reviews the snapshot by hand under ADR-0245.

`shipStoreSlugs` keeps its existing selection and retirement rules. A curated pledge row and
a physical Ship row can coexist. The Archive supplies the original Ship identity and
`playability`. Absence in the two Store datasets leaves the Store status and mapping unknown
to the consumer.

## Considered Options

- **Keep the mapping in the Basetool:** rejected. The GUIDs it points to come from this repository,
  so every patch that changes one would need a change in a second repository, and the check that
  every GUID resolves could not run next to the data.
- **A separate versioned data set next to the catalog:** rejected. One upload, one import run and
  closed GUID references within one file are the reasons ADR-0054 gives for one catalog.
- **Export every slug, with a `null` ship for the unmapped:** rejected. 51 of 256 entries would only
  say "unknown", which a missing entry says as well, and every catalog would carry them.
- **Read the slug from the Archive:** not possible, the Archive holds no store slugs.

## Consequences

- The Basetool resolves a hangar entry in one lookup: the slug to `shipGuid`, a retired slug to a
  flag, a missing slug to unknown.
- Maintaining the file is part of every release against a new `LIVE` Build; the stale list is the
  checklist.
- The file names GUIDs of the `LIVE` Archive. On another Channel a mapped ship can be missing and
  its slug shows as stale; a production import takes `LIVE` only (ADR-0054).
