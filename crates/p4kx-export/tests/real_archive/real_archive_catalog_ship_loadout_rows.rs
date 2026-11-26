// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt;

use serde::Deserialize;
use serde::de::{DeserializeSeed, Deserializer, Error as _, SeqAccess, Visitor};
use serde_json::Value;
use sha2::{Digest as _, Sha256};

use super::digest_runs::Fingerprint;

pub(super) const LOADOUTS: &str = "/loadouts";

#[derive(Default)]
pub(super) struct ShipLoadoutDigest {
    count: usize,
    identities: Sha256,
}

impl ShipLoadoutDigest {
    pub(super) fn append(&mut self, record: u32) -> Result<(), &'static str> {
        self.count = self
            .count
            .checked_add(1)
            .ok_or("too many Ship Loadout roots")?;
        self.identities.update(record.to_be_bytes());
        Ok(())
    }

    pub(super) fn finish(self) -> ShipLoadoutRows {
        ShipLoadoutRows {
            count: self.count,
            identities: self.identities.finalize().into(),
        }
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(in super::super) struct ShipLoadoutRows {
    count: usize,
    identities: Fingerprint,
}

impl ShipLoadoutRows {
    pub(super) const fn count(self) -> usize {
        self.count
    }
}

impl<'de> DeserializeSeed<'de> for ShipLoadoutRows {
    type Value = Value;

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Self::Value, D::Error> {
        deserializer.deserialize_seq(self)
    }
}

impl<'de> Visitor<'de> for ShipLoadoutRows {
    type Value = Value;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("the canonical Loadout owner array")
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<Self::Value, A::Error> {
        let mut observed = ShipLoadoutDigest::default();
        while let Some(owner) = sequence.next_element::<WireOwner>()? {
            if matches!(owner.entity_kind.value, OwnerKind::Ship) {
                observed
                    .append(owner.source_record_index.value)
                    .map_err(A::Error::custom)?;
            }
        }
        if observed.finish() != self {
            return Err(A::Error::custom(
                "Ship Loadout root count, identity or order differs",
            ));
        }
        Ok(Value::from(self.count))
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireOwner {
    entity_kind: PresentCell<OwnerKind>,
    source_record_index: PresentCell<u32>,
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum OwnerKind {
    Ship,
    Item,
}

#[derive(Deserialize)]
struct PresentCell<T> {
    #[serde(rename = "state")]
    _state: Present,
    value: T,
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum Present {
    Present,
}

#[test]
fn resolved_ship_loadout_order_keeps_published_empty_ship_roots() -> super::super::TestResult {
    use serde_json::json;

    let row = |record, kind: &str| {
        json!({
            "entityKind": {"state": "present", "value": kind},
            "sourceRecordIndex": {"state": "present", "value": record},
            "ports": []
        })
    };
    let first = row(9, "ship");
    let empty = row(3, "ship");
    let item = row(7, "item");
    let mut roots = ShipLoadoutDigest::default();
    roots.append(9)?;
    roots.append(3)?;
    let expected = roots.finish();
    let parse = |rows: Value| {
        expected.deserialize(&mut serde_json::Deserializer::from_str(&rows.to_string()))
    };
    assert_eq!(parse(json!([first, item, empty]))?, json!(2));
    assert!(parse(json!([first, item])).is_err());
    assert!(parse(json!([empty, item, first])).is_err());
    assert!(parse(json!([first, empty, empty])).is_err());
    Ok(())
}
