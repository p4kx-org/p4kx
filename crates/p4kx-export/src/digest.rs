// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::convert::Infallible;
use std::hash::{Hash, Hasher};

use p4kx_extraction::{Value, ValueError};
use sha2::{Digest as _, Sha256};

/// A 128-bit digest of structural data (SHA-256 cut to 16 bytes), equal exactly when the data
/// hashed is equal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct Digest(u128);

impl Digest {
    /// Returns the digest the 16 bytes `bytes` spell.
    pub(crate) const fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(u128::from_le_bytes(bytes))
    }
}

/// A [`Hasher`] that feeds every byte into SHA-256.
pub(crate) struct Wide {
    sha: Sha256,
}

impl Wide {
    pub(crate) fn new() -> Self {
        Self { sha: Sha256::new() }
    }

    pub(crate) fn digest(&self) -> Digest {
        let full = self.sha.clone().finalize();
        let mut bytes = [0_u8; 16];
        for (slot, byte) in bytes.iter_mut().zip(full.iter()) {
            *slot = *byte;
        }
        Digest::from_bytes(bytes)
    }
}

impl Hasher for Wide {
    fn write(&mut self, bytes: &[u8]) {
        self.sha.update(bytes);
    }

    fn finish(&self) -> u64 {
        let mut bytes = [0_u8; 8];
        for (slot, byte) in bytes.iter_mut().zip(self.sha.clone().finalize().iter()) {
            *slot = *byte;
        }
        u64::from_le_bytes(bytes)
    }
}

/// Feeds `value` into `state`: scalars by their bytes, structs by type name, field count and
/// fields, arrays by length and elements, References by the GUID they link to and the location of the instance. Localisation keys
/// and the fields `skip` names are left out.
///
/// Returns the least certain outcome across the compared values.
pub(crate) fn hash_value<H: Hasher>(
    state: &mut H,
    value: &Value<'_>,
    skip: &dyn Fn(&str) -> bool,
) -> ComparedValue {
    hash_value_observed(state, value, skip, &mut |_| Ok::<_, Infallible>(()))
        .unwrap_or_else(|never| match never {})
}

/// Comparison outcome ordered by precedence when combining operands.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum ComparedValue {
    Comparable,
    UnresolvedLink,
    Unavailable,
}

/// Visits each original value consumed by the comparison.
pub(crate) fn hash_value_observed<'read, 'source, H: Hasher, E>(
    state: &mut H,
    value: &'read Value<'source>,
    skip: &dyn Fn(&str) -> bool,
    observe: &mut impl FnMut(&'read Value<'source>) -> Result<(), E>,
) -> Result<ComparedValue, E> {
    observe(value)?;
    match value {
        Value::Bool(v) => (0_u8, v).hash(state),
        Value::Int8(v) => (1_u8, i64::from(*v)).hash(state),
        Value::Int16(v) => (1_u8, i64::from(*v)).hash(state),
        Value::Int32(v) => (1_u8, i64::from(*v)).hash(state),
        Value::Int64(v) => (1_u8, v).hash(state),
        Value::UInt8(v) => (2_u8, u64::from(*v)).hash(state),
        Value::UInt16(v) => (2_u8, u64::from(*v)).hash(state),
        Value::UInt32(v) => (2_u8, u64::from(*v)).hash(state),
        Value::UInt64(v) => (2_u8, v).hash(state),
        Value::Single(v) => (3_u8, v.to_bits()).hash(state),
        Value::Double(v) => (4_u8, v.to_bits()).hash(state),
        Value::String(v) => (5_u8, v).hash(state),
        Value::Enum(v) => (6_u8, v).hash(state),
        Value::Guid(v) => (7_u8, v).hash(state),
        Value::Locale(_) => 8_u8.hash(state),
        Value::Reference(reference) => {
            (11_u8, reference.guid(), reference.location().is_some()).hash(state);
            if let Some(location) = reference.location() {
                location.as_str().hash(state);
            }
        }
        Value::Null => 13_u8.hash(state),
        Value::Struct(inner) => {
            let fields: Vec<_> = inner
                .fields()
                .iter()
                .filter(|field| !skip(field.name()))
                .collect();
            (9_u8, inner.type_name(), fields.len()).hash(state);
            let mut outcome = ComparedValue::Comparable;
            for field in fields {
                field.name().hash(state);
                outcome = outcome.max(hash_value_observed(state, field.value(), skip, observe)?);
            }
            return Ok(outcome);
        }
        Value::Array(items) => {
            (10_u8, items.len()).hash(state);
            let mut outcome = ComparedValue::Comparable;
            for item in items {
                outcome = outcome.max(hash_value_observed(state, item, skip, observe)?);
            }
            return Ok(outcome);
        }
        Value::Error(ValueError::DanglingReference { .. } | ValueError::OutsideRecord { .. }) => {
            return Ok(ComparedValue::UnresolvedLink);
        }
        _ => return Ok(ComparedValue::Unavailable),
    }
    Ok(ComparedValue::Comparable)
}

#[cfg(test)]
mod tests {
    use p4kx_extraction::{DataForge, TreeBuilder};
    use p4kx_testkit::{
        DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue,
    };

    use super::*;

    fn digest_of(value: &Value<'_>) -> (Digest, bool) {
        let mut state = Wide::new();
        let complete = hash_value(&mut state, value, &|_| false);
        (state.digest(), complete == ComparedValue::Comparable)
    }

    #[test]
    fn null_and_error_differ_and_an_error_is_incomplete() {
        let (null, null_complete) = digest_of(&Value::Null);
        let (error, error_complete) =
            digest_of(&Value::Error(ValueError::UnknownEnum { index: 0 }));
        assert_ne!(null, error);
        assert!(null_complete);
        assert!(!error_complete);
    }

    #[test]
    fn weak_pointers_into_one_record_differ_by_the_location_of_their_instance()
    -> Result<(), Box<dyn std::error::Error>> {
        const GUID: u128 = 0xcbee_34fd_6441_4000_b99f_924f_7f26_e19f;
        let bytes = DataForgeBuilder::new()
            .structure(TestStruct::new("Node").field("name", TestType::String))
            .structure(
                TestStruct::new("Scene")
                    .field("first", TestType::WeakPointer("Node".to_owned()))
                    .field("second", TestType::WeakPointer("Node".to_owned()))
                    .field("again", TestType::WeakPointer("Node".to_owned()))
                    .array("nodes", TestType::StrongPointer("Node".to_owned())),
            )
            .instance("a", TestInstance::new("Node"))
            .instance("b", TestInstance::new("Node"))
            .record(
                TestRecord::new(GUID, "Scene", "Scene.One", "scenes/one.xml")
                    .value("first", TestValue::pointer("a"))
                    .value("second", TestValue::pointer("b"))
                    .value("again", TestValue::pointer("a"))
                    .value(
                        "nodes",
                        TestValue::Array(vec![TestValue::pointer("a"), TestValue::pointer("b")]),
                    ),
            )
            .build();
        let forge = DataForge::open(&bytes)?;
        let record = forge
            .record_by_guid("cbee34fd-6441-4000-b99f-924f7f26e19f".parse()?)
            .ok_or("record")?;
        let tree = TreeBuilder::new().build(&record)?;
        let digest_of_field = |name: &str| -> Result<Digest, String> {
            let value = tree.field(name).ok_or("field")?;
            Ok(digest_of(value).0)
        };
        assert_eq!(digest_of_field("first")?, digest_of_field("again")?);
        assert_ne!(digest_of_field("first")?, digest_of_field("second")?);
        Ok(())
    }

    #[test]
    fn arrays_are_prefix_free() {
        let one = Value::Array(Box::new([Value::Array(Box::new([Value::Bool(true)]))]));
        let two = Value::Array(Box::new([Value::Array(Box::new([])), Value::Bool(true)]));
        assert_ne!(digest_of(&one).0, digest_of(&two).0);
    }
}
