// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::hash::{BuildHasherDefault, Hasher};

use super::super::{Encryption, Role};
use super::{USES_PER_CHUNK, UseFacts, UseIndex, UseRows};

#[derive(Default)]
struct CollidingHasher;

impl Hasher for CollidingHasher {
    fn write(&mut self, _bytes: &[u8]) {}

    fn finish(&self) -> u64 {
        0
    }
}

const CLEAR: UseFacts = UseFacts {
    role: Role::Payload,
    encryption: Encryption::Unencrypted,
    opaque: false,
};
const OPAQUE: UseFacts = UseFacts {
    role: Role::Attempted,
    encryption: Encryption::Encrypted,
    opaque: true,
};

#[test]
fn exact_use_key_equality_survives_full_hash_collisions_and_growth() -> Result<(), &'static str> {
    let mut index = UseIndex::<BuildHasherDefault<CollidingHasher>>::default();
    for ordinal in 0..100 {
        index.insert(&format!("same prefix {ordinal}"), CLEAR)?;
    }
    for key in ["Exact", "exact", "exact\0", "exáct", "same prefix"] {
        index.insert(key, OPAQUE)?;
    }
    assert_eq!(
        index.len(),
        105,
        "colliding byte strings remain separate IDs"
    );
    for key in ["Exact", "exact", "exact\0", "exáct", "same prefix"] {
        let facts = index.get(key).ok_or("missing exact ID")?;
        assert!(
            !facts.contributes() && facts.opaque() && facts.encryption() == Encryption::Encrypted,
            "the exact opaque attempt remains distinct after table growth"
        );
    }
    for ordinal in 0..100 {
        let facts = index
            .get(&format!("same prefix {ordinal}"))
            .ok_or("missing grown ID")?;
        assert!(
            facts.contributes() && !facts.opaque() && facts.encryption() == Encryption::Unencrypted,
            "every entry retained through table growth resolves its own facts"
        );
    }
    assert!(
        index.get("exact ").is_none() && index.get("exa\u{0301}ct").is_none(),
        "a missing, hash-identical ID cannot use another observation"
    );
    assert_eq!(
        index.insert("exact", CLEAR),
        Err("invalid or duplicate source-use ID"),
        "a true duplicate is rejected"
    );
    assert_eq!(
        index.insert("", CLEAR),
        Err("invalid or duplicate source-use ID"),
        "an empty ID is rejected"
    );
    assert!(
        index.get("exact").is_some_and(UseFacts::opaque),
        "the original facts survive the rejected duplicate"
    );
    Ok(())
}

#[test]
fn dense_use_rows_keep_exact_ends_at_chunk_boundaries() -> Result<(), &'static str> {
    let mut rows = UseRows::default();
    for _ in 1..USES_PER_CHUNK {
        rows.push(b"prior", CLEAR)?;
    }
    let last = rows.push(b"last\0key", OPAQUE)?;
    let next = rows.push("next é".as_bytes(), CLEAR)?;
    assert_eq!(last.checked_add(1), Some(next), "ordinals stay contiguous");
    assert_eq!(
        rows.id(last),
        Some(b"last\0key".as_slice()),
        "the last key uses its original end"
    );
    assert_eq!(
        rows.id(next),
        Some("next é".as_bytes()),
        "the next key starts at the new chunk"
    );
    assert!(
        rows.facts(last).is_some_and(UseFacts::opaque),
        "the last facts remain tied to the key"
    );
    assert!(
        rows.facts(next).is_some_and(UseFacts::contributes),
        "the next facts remain tied to the key"
    );
    assert!(
        rows.id(next + 1).is_none(),
        "spare chunk capacity has no ID"
    );
    Ok(())
}
