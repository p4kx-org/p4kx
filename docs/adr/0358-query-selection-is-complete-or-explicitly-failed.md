# Query selection is complete or explicitly failed

## Decision

`CompiledQuery::run` returns `Result<QueryRun, SelectionError>` and `run_while` returns
`Result<Option<QueryRun>, SelectionError>`. A stopped selection is `Ok(None)` and retains no
partial indexes. A demanded reading failure is an error with the physical Record definition
index, record path, zero-based comparison index and existing `ValueError` as its source. When
several Records fail, the reported error follows the same path/definition order as result rows.
Unreadable whole-Record layouts or instances retain the existing visible failed-row behavior.

Normal missing, null and incompatible values keep SQL/Kleene unknown semantics. Unknown is not
selected, its negation is unknown, false AND unknown is false and true OR unknown is true.
The catalog contract-location rule for unsupported conditions is a separate domain contract.

Comparisons reuse the compiled path walker and stream existential values without a fixed element
count cap or retained value list. Projection and shared-prefix recording retain their existing
per-row memory budget and error cells. Failed full-list admission does not spend the following
cell's remaining budget.

A true existential witness, true OR operand or false AND operand can prove a result despite an
earlier irrelevant read failure. Evaluation continues past provisional failures to find such a
proof; otherwise the first demanded failure is surfaced. Stopping is latched across workers and
checked during Record, path-step and array-element work, including type-filtered elements. A stop
is authoritative over every provisional proof and error and cannot be revived by a later true
callback result.

## Caller contract

CLI Query and Query-filtered field statistics fail with existing DataForge exit status 5 before
opening an output writer. MCP returns an existing tool-error response and only caches successful
complete selections. GUI run and export use the existing cancellable Query path and report a
failed or cancelled operation through their existing model states. Successful response and export
shapes are unchanged; no catalog schema or source-health transport is added. Atomic exports do
not replace a prior complete output when selection fails or stops.

For candidates with readable whole-Record layouts and instances, successful selection completes
demanded condition evaluation under SQL/Kleene semantics. Whole-Record failures remain explicit
failed rows. Successful selection makes no claim that every source field was read or that the
source is globally complete.

## Rationale and validation

A materialization cap is appropriate for retained projected arrays, but using that cap for a
streamed existential predicate can hide known matches and turn a complete false predicate into
unknown. Missing/null semantics cannot also stand for aborted work or a reading failure without
making successful zero-match results ambiguous.

Current-public-API regressions first demonstrate the 4096/4097 early witness, late witness and
complete-no-match-negation failures, while ordinary unknown and projection-accounting controls
pass. Interface-stage tests cover demanded errors, lazy Boolean/existential proofs, cancellation
within skipped/visited elements, deterministic error order, and CLI/MCP/headless-GUI consistency.
