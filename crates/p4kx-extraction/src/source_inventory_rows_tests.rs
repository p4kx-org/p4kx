// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ptr;
use std::sync::Arc;

use super::FrozenRows;

#[test]
fn reads_and_serialization_use_shared_rows_before_compatibility_slice_access()
-> Result<(), serde_json::Error> {
    let last = Arc::new(String::from("last"));
    let first = Arc::new(String::from("first"));
    let frozen = FrozenRows::new(vec![Arc::clone(&first), Arc::clone(&last)]);
    let snapshot = frozen.clone();
    assert_eq!(frozen.len(), 2, "the count needs no contiguous copy");
    assert!(
        frozen
            .iter()
            .next()
            .is_some_and(|row| ptr::eq(row, first.as_ref())),
        "borrowed iteration returns the original ordered row"
    );
    let direct = serde_json::to_vec(&frozen)?;
    assert!(
        frozen.0.contiguous.get().is_none(),
        "direct serialization leaves compatibility storage untouched"
    );
    assert_eq!(
        direct,
        serde_json::to_vec(snapshot.as_slice())?,
        "slice and borrowed bytes agree"
    );
    assert_eq!(
        frozen, snapshot,
        "cache initialization does not affect row equality"
    );
    assert!(
        ptr::eq(frozen.as_slice(), snapshot.as_slice()),
        "cloned snapshots materialize one compatibility slice"
    );
    Ok(())
}
