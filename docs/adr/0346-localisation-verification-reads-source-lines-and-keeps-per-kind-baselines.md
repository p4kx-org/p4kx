# Localisation verification reads source lines and keeps per-kind baselines

Decided for #1041. Builds on ADR-0270 and its per-key/per-kind contract.

## Evidence

The unchanged product code at `72259124` reproduces the reported failure against the installed
LIVE Archive: German summary counters are 31,780 resolved, 74,411 unresolved and 23,717
placeholders. The old `resolved > unresolved` assertion fails.

A separate test executable built from the same product code compares every exported keyed text
directly with decoded source INI lines. All 18 text-bearing kinds pass their existing baselines
and have no source contradictions. The largest German source gaps occur in contracts, mission
broker entries, items, contract templates and mission location templates. These are keys missing
or empty in the German source file, rather than values silently lost by the resolver.

Per-kind figures count serialized keyed-field occurrences, which can repeat through contract
fanout. Summary counters count resolver calls and therefore need not sum to the serialized
occurrences. Neither count measures unique source Records or unique localisation keys.

## Decision

- The primary real-Archive catalog test checks source texts per key and kind instead of demanding
  a majority of resolved texts over the entire catalog. Existing baselines, German floors and
  manufacturer-name checks remain. No translation-gap threshold is lowered.
- The test oracle parses decoded source lines without using `GlobalIni`, `TextSet::lookup` or
  the exporter's display normalizer. Keys ignore ASCII case and an input lookup's leading `@`.
  The last plain and last suffixed declarations are retained; a non-empty plain declaration
  takes precedence. Display normalization decodes literal `\n` and trims the result.
- A missing or empty source value requires an explicit JSON null. A contradictory, malformed
  or invented translation fails independently of every other resolved count. Placeholder keys
  stay separate, and missing-language behavior remains covered by synthetic catalog tests.
- The existing mission-token rewrite allowance checks for a present, non-empty result. Its
  transformation-specific tests remain separate from translation completeness.
- Synthetic oracle mutations run without a game Archive. Real-source tests remain ignored and
  private. Future language-map expansion must retain this independent regression check.

## Verification scope

The baseline reproduction and source-line concordance establish the cause of #1041 on LIVE.
They do not establish completeness of unrelated domain projections or validate a PTU Build.
The final milestone gate still runs the full current real-Archive suite after the remaining
catalog adaptations land.

## Canonical text verification

The ResourceGroup, gameplay-property, reputation scope and reputation standing name references
contribute to the same per-kind denominators. The new group and scope descriptions, standing perk
description, gameplay-property unit formats and name overrides have independent contradiction
checks. A source translation gap in an added field leaves the existing denominator unchanged.

The ignored LIVE text check runs a single unfiltered catalog writer before comparing the current
Build identity and English and German source lines:

~~~powershell
cargo test --locked -p p4kx-export --test real_archive -- installed_live_catalog_texts_match_the_independent_source_oracle --ignored --nocapture
~~~

Set the installed LIVE Archive through `P4KX_DATA_P4K` and use the configured testkit key source
and task-local `TMP` and `TEMP`. The test streams into a temporary file bounded by the catalog's
decompressed limit, then parses the JSON for the existing consumer oracle. The DOM and the
canonical producer can each need several gigabytes on a large Build. This targeted check
requires complete reads and projections for the text families with baselines. It allows
unrelated diagnostic partial publication. Keep raw output private. The final milestone's
full installed-Archive suite is a separate run.
