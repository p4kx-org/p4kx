// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::Value;

use super::original::Witness;
use super::{TestResult, proof};

pub(super) fn check(catalog: &Value, witness: &Witness<'_>) -> TestResult {
    let ammo = proof::rows(catalog, "/definitions/ammoDefinitions/rows")?;
    let [definition] = ammo else {
        return Err("one required ammunition definition for the filtered weapon".into());
    };
    assert_eq!(
        proof::number(definition, "/recordIndex/value")?,
        u64::from(witness.ammo.record_index()),
        "the definition retains the original primary ammunition Record"
    );
    assert_eq!(
        proof::text(definition, "/selection")?,
        "dependency",
        "the weapon-only Filter retains ammunition as a dependency"
    );
    let owner = u64::from(
        witness
            .magazine
            .unwrap_or_else(|| witness.owner.record_index()),
    );
    let slot = u64::try_from(witness.container)?;
    let containers = proof::rows(catalog, "/configurations/ammoContainers/rows")?;
    let mut selected = containers.iter().filter(|row| {
        row.pointer("/ownerRecordIndex/value")
            .and_then(Value::as_u64)
            == Some(owner)
            && row
                .pointer("/componentOrdinal/value")
                .and_then(Value::as_u64)
                == Some(slot)
    });
    let container = selected.next().ok_or("retained primary container")?;
    assert!(
        selected.next().is_none(),
        "one row per physical primary slot"
    );
    let expected = if witness.magazine.is_some() {
        "dependency"
    } else {
        "selected"
    };
    assert_eq!(
        proof::text(container, "/selection")?,
        expected,
        "the primary container selection follows its physical owner"
    );
    Ok(())
}
