// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::Value;

use super::stats_digest;
use crate::contract_stats::{
    ItemStats, ItemStatsFields, PublishedItemStats, QuantumDriveStats, ResourceFlow, ResourceState,
    ShieldStats, Signature, WeaponStats,
};
use crate::digest::Wide;

type TestResult = Result<(), Box<dyn std::error::Error>>;

const WEAPON: &str = r#"{"weapon":{"fireActions":[],"ammoCapacity":0,"ammo":null}"#;
const SHIELD: &str = concat!(
    r#","shield":{"maxHealth":8000.0,"maxRegen":160.0,"decayRatio":0.25,"#,
    r#""downedRegenDelay":5.0,"damagedRegenDelay":2.5}"#,
);
const QUANTUM: &str = concat!(
    r#","quantumDrive":{"fuelRequirement":-0.0,"jumpRange":50000.0,"disconnectRange":null,"#,
    r#""driveSpeed":12000.0,"cooldownTime":null,"spoolUpTime":7.0,"#,
    r#""stageOneAccelRate":null,"stageTwoAccelRate":null,"engageSpeed":40.0}"#,
);
const REMAINING: &str = concat!(
    r#","powerPlant":null,"cooler":null,"missile":null,"armor":null,"#,
    r#""clothing":null,"radar":null,"tractorBeam":null,"weaponModifier":null,"#,
    r#""miningLaser":null,"miningModifier":null,"health":450.0,"resourceStates":[{"#,
    r#""name":"Online","emSignature":{"nominal":-0.0,"decayRate":null},"irSignature":null,"#,
    r#""flows":[{"kind":"ItemResourceDeltaGeneration","direction":"generation","resource":"Power","#,
    r#""amount":1.5,"unit":"SPowerSegmentResourceUnit"}]}]}"#,
);

fn shield() -> ShieldStats {
    ShieldStats {
        max_health: Some(8_000.0),
        max_regen: Some(160.0),
        decay_ratio: Some(0.25),
        downed_regen_delay: Some(5.0),
        damaged_regen_delay: Some(2.5),
    }
}

fn quantum() -> QuantumDriveStats {
    QuantumDriveStats {
        fuel_requirement: Some(-0.0),
        jump_range: Some(50_000.0),
        disconnect_range: None,
        drive_speed: Some(12_000.0),
        cooldown_time: None,
        spool_up_time: Some(7.0),
        stage_one_accel_rate: None,
        stage_two_accel_rate: None,
        engage_speed: Some(40.0),
    }
}

fn stats(
    shield: Option<ShieldStats>,
    quantum_drive: Option<QuantumDriveStats>,
) -> ItemStats<'static> {
    ItemStats {
        weapon: Some(WeaponStats {
            fire_actions: Vec::new(),
            ammo_capacity: Some(0),
            ammo: None,
        }),
        shield,
        quantum_drive,
        fields: ItemStatsFields {
            power_plant: None,
            cooler: None,
            missile: None,
            armor: None,
            clothing: None,
            radar: None,
            tractor_beam: None,
            weapon_modifier: None,
            mining_laser: None,
            mining_modifier: None,
            health: Some(450.0),
            resource_states: vec![ResourceState {
                name: Some("Online"),
                em_signature: Some(Signature {
                    nominal: Some(-0.0),
                    decay_rate: None,
                }),
                ir_signature: None,
                flows: vec![ResourceFlow {
                    kind: "ItemResourceDeltaGeneration",
                    direction: "generation",
                    resource: Some("Power"),
                    amount: Some(1.5),
                    unit: Some("SPowerSegmentResourceUnit"),
                }],
            }],
        },
    }
}

#[test]
fn compact_item_variant_bytes_and_digest_keep_the_device_slots() -> TestResult {
    use std::hash::Hasher as _;

    for (source, drive, shield, quantum) in [
        (Some(shield()), Some(quantum()), SHIELD, QUANTUM),
        (None, None, r#","shield":null"#, r#","quantumDrive":null"#),
    ] {
        let stats = stats(source, drive);
        let expected = format!("{WEAPON}{shield}{quantum}{REMAINING}");
        assert_eq!(
            serde_json::to_vec(&stats)?,
            expected.as_bytes(),
            "the variant identity byte stream retains exact order, number formatting and nulls"
        );
        let mut digest = Wide::new();
        digest.write(expected.as_bytes());
        assert_eq!(
            stats_digest(Some(&stats)),
            Some(digest.digest()),
            "the real variant hash consumes the same compact bytes"
        );
    }
    assert_eq!(
        stats_digest(None),
        Some(Wide::new().digest()),
        "an item without legacy stats hashes zero bytes"
    );
    Ok(())
}

#[test]
fn published_stats_keep_fields_outside_device_configurations() -> TestResult {
    let published = stats(Some(shield()), Some(quantum()))
        .into_published()
        .ok_or("public stats still present")?;
    let expected = format!("{WEAPON}{REMAINING}");
    assert_eq!(
        serde_json::to_string(&published)?,
        expected,
        "the public view preserves values and wire order of the remaining fields"
    );
    let schema = serde_json::to_value(schemars::schema_for!(PublishedItemStats<'static>))?;
    let properties = schema
        .get("properties")
        .and_then(Value::as_object)
        .ok_or("published stat properties")?;
    let required = schema
        .get("required")
        .and_then(Value::as_array)
        .ok_or("published required fields")?;
    let expected = serde_json::from_str::<Value>(&expected)?;
    let expected = expected.as_object().ok_or("expected stats object")?;
    assert!(
        properties.keys().eq(expected.keys()) && required.len() == expected.len(),
        "schema keys are exactly the emitted keys"
    );
    for field in expected.keys() {
        assert!(
            required.iter().any(|name| name.as_str() == Some(field)),
            "the emitted field {field} is required even when nullable"
        );
    }
    Ok(())
}
