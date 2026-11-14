# Canonical trees have no implicit materialization quota

Canonical Record materialization has no fixed field, depth or Value-count quota; an optional caller allowance applies to one read and counts actual field and Array-element Value occurrences before output reservation. The implicit root, headers, source metadata and scratch storage are excluded, so this count is not a memory estimate. Source errors, checked platform limits, allocator refusal and observer stop remain distinct, and refusal never returns a prefix as a complete tree.

Original field/instance tuples use u64 so inherited logical fields are not limited by a narrower internal coordinate carrier; Array Step indices retain their actual format width through checked conversion.
