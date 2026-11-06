# The catalog verification measures texts per key and kind, not by a global share

Decided for issue #916. Evidence: LIVE 4.10. Builds on ADR-0054 (additive catalog) and ADR-0266
(location templates, #902).

## Decision

- The real-Archive verification no longer asserts a global share of German or English texts. A
  share over all kinds measures how complete the game's German `global.ini` is, so every new kind
  with untranslated keys can break it (the contract descriptions added 5,143 texts the German file
  lacks and took the share to 0.469 against 0.5).
- It checks the exporter per kind and key against the Archive's own `global.ini` files:
  - the expected text of a key is built in the test from the file's lines, not with the exporter's
    lookup: the key without its `@`, ASCII case ignored, a `key,P=` line standing for `key`, the
    first non-empty text, `\n` decoded and the text trimmed. The catalog's text must equal it; a key
    the file lacks or holds empty must be `null` (the Basetool falls back to English). A
    contradiction fails and names the kind, the keys and both texts. A contract text with a
    `~mission(…)` token is rewritten by the exporter, so only its presence is compared;
  - every `name` and every field marked `x-gameText` in the JSON Schema is measured wherever it
    sits in an entry, so a new text field is covered without a change of the test;
  - on LIVE the keys a file lacks are counted per kind and language and must stay within the
    baseline of that kind, measured on LIVE and rounded up to the hundredth. A kind that is not
    listed has a baseline of zero, so a new kind with untranslated keys names itself and its
    baseline is added on purpose. Other Channels check only the contradictions (ADR-0038: no
    test-channel numbers in committed tests).
- The numbers of every kind are printed, with the first keys the English file lacks.
- `missionLocationTemplates[].names[]` exports `displayName` as a game text (`x-gameText`) again,
  like every other named kind: #910 had reduced it to the `nameKey` to stay above the German floor.
  The catalog stays additive within schema version 2 (ADR-0054).

## Consequences

- Translation gaps in the game never fail the verification; a regression of the exporter, or a
  kind whose untranslated share grows past its baseline, does and names the kind.
- The English baseline of `missionLocationTemplates` is 0.09 because the English file itself lacks
  274 of the 3,211 keyed names on LIVE (samples: `@generic_locations_blank` and
  `@mission_location_pyro_0001`). The test confirms the key is absent from the file, so these are
  gaps in the game's data, not in the exporter. `@generic_locations_blank` also heads the English
  gaps of `contracts` and `starmapObjects`.
- A German baseline of 1.0 (`blueprints`, `refiningProcesses`) means no German text exists for the
  kind on LIVE, so only contradictions are checked there.
- The oracle covers the lookup rules above. It does not cover other transformations of the
  exporter; the only one in the catalog's texts is the `~mission(…)` rule.
- A change of the game that moves a share past its baseline is a deliberate edit of the baseline,
  made after looking at the kind.

## Independent oracle and primary-test alignment

[ADR-0346](0346-localisation-verification-reads-source-lines-and-keeps-per-kind-baselines.md)
records the LIVE reproduction for #1041 and aligns the older primary catalog test with this
contract. The oracle now parses decoded source lines independently of the production INI index.
For duplicate lines it uses the last plain and last suffixed declarations, preferring a non-empty
plain value; this clarifies the abbreviated first-non-empty rule above. Per-kind baselines and
German floors remain unchanged.
