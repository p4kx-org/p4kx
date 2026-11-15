// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ptr;

use super::append_owned;

#[test]
fn the_first_system_rows_keep_their_allocation_and_later_rows_keep_their_order() {
    let first = vec![1, 2, 3];
    let allocation = first.as_ptr();
    let mut rows = Vec::new();

    append_owned(&mut rows, first);
    assert!(
        ptr::eq(rows.as_ptr(), allocation),
        "the first buffer is moved"
    );
    append_owned(&mut rows, Vec::new());
    assert!(
        ptr::eq(rows.as_ptr(), allocation),
        "an empty system leaves the buffer in place"
    );
    append_owned(&mut rows, vec![4, 5]);

    assert_eq!(rows, [1, 2, 3, 4, 5], "system and source row order");
}
