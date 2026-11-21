# Audio walks share active banks and retain first read failures

This replaces the bank-count and retention policy of ADR-0158. A reference to a valid third
indexed soundbank must not lose its terminal merely because another foreign bank was visited.

## Decision

Select the actual indexed candidate before consulting storage. Current-bank missing-target
priority, language filtering and explicit indexed stinger positions remain unchanged. Home wins
among eligible candidates; otherwise the first selected foreign bank remains a preference when
eligible. A different eligible candidate is followed without replacing that preference. An
unreadable preferred source does not fall back to another candidate.

An explicit traversal stack retains the actual selected bank owner and HIRC object occurrence
for each active path step. Each cursor advances through its source list in authored order.
Returning to a different object in an active bank shares its decoded allocation. A reference to
an active physical bank position and object ID is a cycle. The same reference at two sibling
positions is walked twice. Path origins retain the actual occurrence and reference ordinal.

The one-result Spare remains shared by events of a home bank. Completed successful banks that
are neither active nor the spare are released and may be read again later. There is no resident
map of every successful bank. Each event separately retains the first actual read/parse failure
per selected bank position, including failures inherited from the spare. That event never retries
such a source after the spare changes. Missing objects, kind mismatches, locale refusals, cycles
and graph-budget outcomes do not become cached whole-bank read failures.

## Costs and remaining policies

Decoded residency scales with active distinct bank allocations, the home bank and the spare.
Each active path step occupies one traversal frame. Materialized variants and breaks retain their
paths and can grow with the number and depth of authored routes. Failure storage scales with the
distinct failed banks and their typed errors in that event. Concurrent home walks have additive
residency.

The default walk has no fixed depth, chain-length, child-count or visit quota. An optional visit
budget applies to the whole trigger. Debit each attempted HIRC object, including a missing target
or cycle, and each visited MusicTrack source. Reading the index is separate work. A budgeted walk
visits authored Events and eligible bank holders in the result order. The first unaffordable visit
ends it and records `too_many_objects` with the selected limit. Inspection exposes the reached
prefix. File exporters refuse it before creating or changing output. The unlimited walk can still
follow different home banks in parallel. Cancellation stops at traversal boundaries and discards
the partial result.

Source-byte admission remains a separate reader policy. The `other_soundbank` break identifies
an explicitly named bank absent from the eligible index.

## Verification

Original Archive bytes, HIRC declarations and index membership precede terminal assertions.
Nested three-bank and sibling routes retain authored order and physical bank identity. Private
observations count actual Loaded::read calls and retain Weak references only: active A/B/A shares
its first A allocation, failed A/B/A preserves its first typed error without a retry, and completed
A/B/A releases then reloads A. Home and first-foreign priority, genuine cycles, missing targets
and explicitly indexed stingers retain independent controls. The historical cap oracle keeps
its exact three-bank source and requires both original terminal sounds under this decision.
Synthetic fixtures cover deep paths, long PlayEvent chains, wide child and track source lists,
and a terminal after more than 16,384 visits. Budget fixtures cover exact exhaustion across
Events and holders and preservation of earlier output files.