# Leftover-like ships are excluded by the words `temp`, `display` and `hijacked`

Decided for issue #423. Builds on ADR-0002 and ADR-0054, and on the playability rules of
`docs/catalog.md`.

## Decision

- **A ship is a development leftover when its record path or name holds the whole word `temp`,
  `display` or `hijacked`**, ignoring ASCII case, like the tokens of `EXCLUDED_TOKENS`. It is left
  out of `ships` and counted as skipped. The owner decided this after the evidence below showed no
  structural rule. The words are `SHIP_EXCLUDED_TOKENS`, applied to the ships kind only
  (`is_kind_leftover`).
- **Only ships.** The same words in `EXCLUDED_TOKENS` would also drop 537 entity Records of the
  other kinds on LIVE, among them the `radar_display_*` ship components, 362 shop displays and
  `*_temp` seats. Those stay in the catalog.
- **LIVE: 21 ships become leftovers, 15 of them `player`:** `AEGS_Avenger_Stalker_Display`,
  `AEGS_Reclaimer_BIS2024_Temp`, `AEGS_Reclaimer_PU_Hijacked` and its `_A`, `_B`, `_C`,
  `ANVL_Lightning_F8C_BIS2024_Temp`, `ANVL_Terrapin_BIS2024_Temp`, `CRUS_Spirit_C1_BIS2024_Temp`,
  `DRAK_Caterpillar_PU_Hijacked`, `GLSN_Shiv_TEMP_CosA`, `_CosB`, `_CosC`, `ORIG_890Jump_Hijacked`,
  `ORIG_890Jump_Hijacked_Pisces`, `RSI_Aurora_Mk2_MissileModule_TEMP`,
  `RSI_Constellation_Andromeda_GS_Display`, `RSI_Constellation_Mk5_Centaurus_Display`,
  `RSI_Constellation_Phoenix_GS_TEMP`, `RSI_Meteor_TEMP_LaserRepeater`, `RSI_Skylark_Display`.
  Every one carries the word as a whole word of its name, and a ship whose name merely contains
  one (`temperature`, `displays`, `hijacker`) stays.
- **The selection of `ships` changes, so this is a breaking change** that rides on
  `schemaVersion` 2 (ADR-0054, #671).
- **`p4kx ships` applies the same rule**, because it reads the ships kind. The owner decided to
  raise the `schemaVersion` of the ship base data to 2 (`SHIP_BASE_VERSION`) for it.

## Evidence for no structural rule

- **Reverse References do not tell them apart.** The Reference index of the DataForge was read for
  the 323 `player` ships of LIVE: the leftover-like ships have no referrer, but 162 of the 323
  have none, among them `AEGS_Avenger_Titan`, `AEGS_Sabre` and `ORIG_400i`.
- **The Records give no other marker.** `AEGS_Reclaimer_BIS2024_Temp` is a copy of its base hull:
  its Record differs from `AEGS_Reclaimer` only in GUID, class name and the links to itself, so
  Tags, components and the `vehicleDefinition` equal the base, and every `player` ship lies in
  `entities/spaceships/`. 33 `player` ships share name and description keys with their base
  hull, and event ships (`BIS2950`, `Plat`, `Fleetweek`) are among them, so that is no marker
  either.

## Consequences

- `ships` has 21 entries fewer on LIVE, and `skipped` of the ships kind 21 more.
- A new ship whose name holds one of the words is left out without a check of its other Records.
