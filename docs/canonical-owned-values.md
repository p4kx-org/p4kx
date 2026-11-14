# Canonical owned Record values

The canonical reader traverses valid owned values in original field and array order. It uses
heap-backed continuations rather than Rust call-stack recursion for construction, Clone,
equality, canonical-tree destruction, error traversal, Reference indexing, JSON/XML/flat
writers, raw field statistics and inline pointer-offset discovery. There is no owned-depth
64 or mixed value/work 1,048,576 cutoff. These numbers remain named regression inputs.

This is not a universal memory, runtime or source-size guarantee. Actual layouts, instance
ranges, source table widths, ownership errors and each independent consumer's documented
admission still apply. A small source can describe many zero-size Class instances; the
materialized tree requires storage for every retained occurrence.

## Original ownership and failure

Fields retain the actual declaration's FieldStorage. RecordTree and Reference retain the same
borrowed Forge ownership; equal descriptors never establish original authority. The canonical
observer still sees exactly the completed original read outcome before any projection returns.

Strong pointers and Class-array elements claim actual physical instances once per Record.
A repeated claim is OwnedTwice. Non-array inline Class fields instead track active declaration
types within that actual owned instance. Repeating an active type produces InlineClassCycle
at that declaration. Finite repeated inline occurrences in different branches remain valid.

InlineClassCycle.structIndex comes from the selected scalar Class declaration's target.
The source codec verifies the same sealed occurrence, Class storage, non-array shape and target
before exporting its one declaration $target Selector. It does not invent a stored pointer word.
Null, malformed range, invalid pointer, dangling Reference and query-validation facts remain
distinct. CanonicalTreeInvariant reports inconsistent private construction state explicitly.

## Retained state and cost

Construction retains one parent-linked edge for each structural prefix or original weak slot,
rather than copying every ancestor tuple at every visit. Ordinary scalar array elements do not
create position edges. A reusable scratch buffer reconstructs an original tuple only when needed.
The owned-instance map and continuation stack retain their actual logical lengths and capacities.

Weak targets retain one cached Location per distinct demanded owned instance. Its exact semantic
steps and original physical positions are shared through the existing Arc. Repeated References
reuse the same cached Location and original borrowed type text. No new per-field Arc, Registry,
source-identity token or target lookup is introduced. Source-slot replacement still navigates
each pending weak slot's original tuple; its work is the sum of those source tuple lengths.

Clone copies the owned result and shares existing Reference locations. Equality and Debug borrow
the original tree. Canonical-tree Drop drains owned field/array buffers with heap continuations.
The public SDK can also construct arbitrary Value arrays outside a StructValue; this does not
add a custom Drop implementation to Value or promise an unbounded arbitrary-SDK-array destructor.

Compact JSON and flat Debug avoid nested formatter call stacks. Pretty JSON/XML still emit the
indentation prescribed by their wire formats; output bytes and indentation work can grow with
the sum of occurrence depths. Flat output likewise emits complete paths when its format requires
them. Iteration does not make those bytes disappear.

The source-first 512-by-64 regression has 66,050 semantic values. Its API-dependent profile checks
the actual tree and builder and reports CANONICAL_OWNED_CAPACITY logical lengths/capacities for
values, fields, owned instances, position edges, weak slots, location cache and scratch. It is
not an RSS, allocation-byte, throughput or native-ABI claim. Root execution supplies the measured
capacity values; size_of(Value) alone is not a tree-memory bound.

## Public and consumer changes

MAX_NESTING_DEPTH and MAX_RECORD_VALUES and their Extraction reexports are removed.
ValueError::TooDeep and TooManyValues and DataForgeError::RecordTooLarge are removed.
ValueError::InlineClassCycle and DataForgeError::CanonicalTreeInvariant replace no valid
source values with a new quota.

The source error schema has fourteen current typed variants: the two retired policy errors are
replaced by the one genuine inline cycle error. Unknown future error variants still refuse
lossless source conversion. Generated catalog schemas must be regenerated with this source.

Record JSON, XML, flat output and descriptive Reference/Location wire contracts remain unchanged.
Value/StructValue Debug changes to a flat, explicit event sequence with field ordinals, storage,
names, element ordinals, full scalar values and balanced container boundaries. It is not byte
compatible with the recursive derived Debug form. Standard Debug flags are forwarded through
standard formatting builders; no token or string is shortened.

A default serde_json::Value reader still has its separate recursion-depth policy. The 65-node
canonical JSON test and the flat catalog source inventories exercise ordinary default parsing;
they do not prove arbitrary-depth consumers. No unbounded parser option, alternate dialect or
new parser quota is introduced here. Query paths, Tag handling, raw-survey pointer-count policy,
layout field counts and caller publication/resource policies remain separate open contracts.