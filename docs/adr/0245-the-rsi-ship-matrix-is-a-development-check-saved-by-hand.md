# The RSI ship matrix is a development check, saved by hand

Decided for issue #809. Applies ADR-0002 (the Archive is the source of the catalog).

## Context

The `playability` of ships (ADR-0054, "Playability" in `docs/catalog.md`) is derived from
Record data and has never been compared with an outside reference. Another extractor takes "flight
ready" from the public RSI ship matrix, `https://robertsspaceindustries.com/ship-matrix/index`
(`production_status == "flight-ready"`). The same data can serve as a development check of our
rule.

## Terms of use

Checked on 2026-10-28 in the RSI Terms of Service (`https://robertsspaceindustries.com/en/tos`)
and in `https://robertsspaceindustries.com/robots.txt`:

- Section IV (Rules of Conduct) forbids "automated measures like bots, spiders, scrapers or
  similar means or technologies" and names data scraping or harvesting.
- Section XIII.B (Intellectual Property Rights) forbids copying or downloading RSI Content from the
  RSI Services without permission, and selling, distributing, publishing or modifying it.
- Section XIII.D (Personal and Fansite Use) forbids creating derivative works of the RSI Fansite
  Content.
- `robots.txt` disallows only `/media/` and has no rule for `/ship-matrix`.
- No clause names crawlers, robots.txt or data mining.

An automated fetch, by P4KX or by CI, is an automated measure under section IV. P4KX therefore
never fetches the ship matrix.

## Decision

- The ship matrix JSON is saved **by hand** in a browser by the owner, as `ship-matrix.json`, to
  `local-data/ship-matrix/ship-matrix.json` in the repository folder. `/local-data/` is excluded
  locally in `.git/info/exclude`, not in `.gitignore`, so the file never reaches git and never
  leaves the PC. The local wrapper `tools\real-archive.ps1` (not tracked) may set
  `P4KX_REF_SHIP_MATRIX` to it when it exists.
- No code of P4KX requests anything from RSI. CI has no ship matrix: the check is `#[ignore]`d,
  skips without `P4KX_REF_SHIP_MATRIX`, and its comparison logic is tested with a small synthetic
  matrix written for the test.
- Nothing from the ship matrix is committed, copied into a fixture or doc, or exported: no
  descriptions, images or stats, and no value is read from it (its stats are often out of date).
  Docs hold only counts and Archive class names derived by the check.
- The check reads the matrix as `{"data": [...]}` or a bare list. A matrix entry maps to a catalog
  ship through the curated `shipStoreSlugs` (ADR-0117): by the RSI ship id (`id`), then by the last
  segment of `url` against the slug. Matrix names the slugs do not map are mapped by an explicit
  name table in the test (`NAME_SUPPLEMENT`), read off the Archive's class names and used only
  there; no name enters the `playability` rule.
- The `playability` rule is refined only on a structural signal, never by name. The first
  comparison (see "Comparison" in `docs/catalog.md`) showed none.
- The second look (#838) found one for two ships: `ANVL_Hornet_F7C_Mk2_AI_Super` and
  `RSI_Perseus_AI_Super` were `player` although the matrix does not know them. Their hull values,
  crew manifest, tags and inclusion mode equal their `player` siblings. What separates them from
  every `player` ship, and puts them with the other 13 ships of the same kind, is the Record tag
  (`$tag`) `SystemsDesign`: of the 15 ships that carry it, 10 were `ai` and 3 `null` already, and
  none of the 307 other `player` ships has it. A Record tag is Record data like the folder of the
  record path, so rule 1 of the ship rule takes it as an AI marker. The tag names the authoring
  team, not a behaviour, so the rule stays a marker found on LIVE and is re-checked with each
  comparison; the real-Archive test `no_player_ship_carries_the_ai_record_tag` fails then. Failure
  modes: a `player` ship that gains the tag is demoted to `ai`, an `AI_Super` ship that loses it
  becomes `player` again, and in a version 6 DataForge (no Record tag) the rule does nothing.
- The seven `null` base ships the matrix calls flight ready (Basher, Sabre Raven EX, Skylark,
  Railen, S-65 Stingray, Tyilui, Constellation Mk V Centaurus) share no component, field, tag,
  Reference or folder that is rare elsewhere, but they do not need one: the only ships `null`
  was left for were `ReadyToInclude` ships with `powerPlantsCanExplodeOnDeath` and no other AI
  marker (crew manifest, critical-part explosion, `SystemsDesign`), exactly these seven and
  `KRIG_S65_Stingray_BALLISTIC`, whose Record has the same fields as the Stingray. The owner
  confirmed all seven flyable. Rule 4 therefore no longer reads the hull: `ReadyToInclude` alone
  gives `player`. No ship but these eight changes; the hull value stays an AI marker for ships
  that are not `ReadyToInclude`.
- Known misclassification, flagged by the owner: `KRIG_S65_Stingray_BALLISTIC` is `player` but is
  not flyable in the game. Its Record differs from `KRIG_S65_Stingray` only in the weapon
  Entities of its default Loadout (ballistic instead of laser repeaters) and in self-referencing
  paths; inclusion mode, tags, `$tag`, components, hull values and crew manifest are equal. No
  structural signal separates it, so it stays `player` and no name exception is made.
- The check warns when a `NAME_SUPPLEMENT` class is no catalog ship and when a catalog ship or
  store slug entry cannot be read. A catalog class counts as flight ready when any of its matrix
  entries is, so a flight-ready variant mapped to the same class (Mole editions, Carrack w/C8X)
  hides a base entry that is not.

## Consequences

- The comparison is repeatable by anyone who saves the matrix the same way, without any automated
  access to RSI.
- The ship matrix changes over time; the stored counts date from the file of 2026-10-28.
- If RSI permits automated access to the ship matrix, a later ADR may add a fetch.
