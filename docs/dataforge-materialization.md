# DataForge tree materialization

`TreeBuilder::build` materializes the complete canonical Record tree without a fixed Value-count quota. Source validation, platform/container representability and allocation failures remain explicit.

A library caller can use `TreeBuilder::build_with_budget(record, maximum_values)` for one read. The allowance counts each actual field or Array-element `Value` occurrence once. Structs, Arrays, scalar values, References, Null and original ValueError occurrences all count. The implicit Record root, headers, declaration metadata, source-layout storage and scratch state do not count, so this is not a RAM or execution-time limit.

Before an output container reserves storage, it admits its direct child Value slots. Nested child containers add their own slots when reached. A budget refusal returns `DataForgeError::MaterializationBudgetExceeded` with the caller's limit and the next minimum total required. No partial tree is returned as complete; unvisited input is not asserted to be valid.

Addressable output reservations use fallible Vec reservation. Allocator refusal is `MaterializationAllocationRefused`; platform/container representability is `TooLarge`. Source-layout metadata has the distinct `LayoutAllocationRefused` error and is not charged as output values.

The allowance does not persist into another read. After success or refusal the same builder can be reused; an ordinary build is unbudgeted. Canonical observers receive each actual result once, and their existing stop takes priority as `ObservationStopped`. Resource failures are operation outcomes, not fabricated source ValueError facts.
