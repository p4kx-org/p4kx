# Audio walks share active banks and retain first read failures

This replaces the bank-count and retention policy of ADR-0158. A reference to a valid third
indexed soundbank must not lose its terminal merely because another foreign bank was visited.

## Decision

Select the actual indexed candidate before consulting storage. Current-bank missing-target
priority, language filtering and explicit indexed stinger positions remain unchanged. Home wins
among eligible candidates; otherwise the first selected foreign bank remains a preference when
eligible. A different eligible candidate is followed without replacing that preference. An
unreadable preferred source does not fall back to another candidate.

Each active path step retains the actual selected Loaded owner. Returning to a different object
in that active bank shares its existing decoded allocation. Returning to the same physical
bank/object pair remains a cycle. Public paths retain only their existing PathStep values.

The one-result Spare remains shared by events of a home bank. Completed successful banks that
are neither active nor the spare are released and may be read again later. There is no resident
map of every successful bank. Each event separately retains the first actual read/parse failure
per selected bank position, including failures inherited from the spare. That event never retries
such a source after the spare changes. Missing objects, kind mismatches, locale refusals, cycles
and graph-budget outcomes do not become cached whole-bank read failures.

## Costs and remaining policies

Decoded residency scales with active distinct bank allocations, the home bank and the spare;
recursive restore frames and path handles share those allocations. Failure storage scales with
distinct selected failed banks and their typed error payloads within that event. Concurrent home
walks have additive residency. This is not a constant-memory or measured peak-RSS guarantee.
Candidate lookup, path scanning, IO, parsing and emitted variants still incur their actual work.

Source-byte admission and the independent path, action-chain, child and object budgets remain
separate reader policies under review. Removing the bank-count fence does not establish their
sufficiency. The other_soundbank break kind now identifies an explicitly named bank absent from
the eligible index, without conflating that absence with a bank-count budget.

## Verification

Original Archive bytes, HIRC declarations and index membership precede terminal assertions.
Nested three-bank and sibling routes retain authored order and physical bank identity. Private
observations count actual Loaded::read calls and retain Weak references only: active A/B/A shares
its first A allocation, failed A/B/A preserves its first typed error without a retry, and completed
A/B/A releases then reloads A. Home and first-foreign priority, genuine cycles, missing targets
and explicitly indexed stingers retain independent controls. The historical cap oracle keeps
its exact three-bank source and requires both original terminal sounds under this decision.