// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;
use std::path::Path;

use super::accounting::{Counts, DeclarationState};
use super::baseline::{self, LEGACY_EFFECT_FIELDS, LEGACY_EFFECT_OWNER, QUANTUM_DRIVE_OWNER};
use super::coverage::{QUANTUM_DRIVES, QUANTUM_PROFILES};
use super::inventory::{Disposition, FieldKey};
use super::published_io;
use super::published_model::MAX_METADATA_BYTES;
use super::real_archive;

const PROFILES: [&str; 2] = ["params", "splineJumpParams"];

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn quantum_field_inventory_observes_both_original_profiles() -> super::Result<()> {
    let filter =
        real_archive::record_filter()?.ok_or("an explicit Quantum Record filter is required")?;
    let mut inventory = baseline::quantum_inventory();
    if let Some(path) = std::env::var_os("P4KX_FIELD_EXPECTED_ABSENCES") {
        inventory.expected_absences =
            published_io::read_json(Path::new(&path), MAX_METADATA_BYTES, &|| false)?;
    }
    let counts = real_archive::verify_field_inventory(&inventory, Some(&filter))?;
    require_profiles(&counts)
}

fn require_profiles(counts: &BTreeMap<FieldKey, Counts>) -> super::Result<()> {
    for field in PROFILES {
        let observed = counts.get(&FieldKey::new(QUANTUM_DRIVE_OWNER, field));
        if !observed.is_some_and(|count| {
            count.declaration == DeclarationState::Declared
                && count.populated > 0
                && count.exported > 0
        }) {
            return Err(
                format!("the selected Quantum source must witness and export {field}").into(),
            );
        }
    }
    Ok(())
}

#[test]
fn the_focused_inventory_retains_every_reviewed_quantum_mapping() -> super::Result<()> {
    let all = baseline::inventory();
    let expected = all
        .fields
        .iter()
        .filter(|mapping| {
            matches!(
                &mapping.disposition,
                Disposition::Exported { group, .. }
                    if matches!(group.as_str(), QUANTUM_DRIVES | QUANTUM_PROFILES)
            )
        })
        .collect::<Vec<_>>();
    let quantum = baseline::quantum_inventory();
    assert_eq!(
        quantum.roots,
        [QUANTUM_DRIVE_OWNER],
        "only the owned Quantum declaration closure is selected",
    );
    assert_eq!(
        serde_json::to_value(&quantum.fields)?,
        serde_json::to_value(expected)?,
        "no baseline Quantum mapping may be removed or changed by a focused survey",
    );
    let reviewed = quantum.reviewed()?;
    for name in LEGACY_EFFECT_FIELDS {
        assert!(
            reviewed.contains_key(&FieldKey::new(LEGACY_EFFECT_OWNER, name)),
            "the focused invocation retains every reviewed Legacy Tag Reference",
        );
    }
    Ok(())
}

#[test]
fn both_quantum_profiles_need_populated_source_and_export_evidence() {
    let mut counts = BTreeMap::new();
    for field in PROFILES {
        assert!(
            require_profiles(&counts).is_err(),
            "no positive witness is inferred from an empty or one-mode selection",
        );
        let _ = counts.insert(
            FieldKey::new(QUANTUM_DRIVE_OWNER, field),
            Counts {
                declaration: DeclarationState::Declared,
                populated: 1,
                exported: 1,
                ..Counts::default()
            },
        );
    }
    assert!(
        require_profiles(&counts).is_ok(),
        "both modes have an observed source and an exported value",
    );
}
