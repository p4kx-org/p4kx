// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::super::CoreInspection;
use super::{FixtureImage, FixtureKeys, FixtureValue, KeyPresence};

#[test]
fn key_presence_never_admits_a_child_without_its_parent() {
    use KeyPresence::{Absent, Present};
    assert!(FixtureKeys::new(Absent, Absent, Present, Absent).is_err());
    assert!(FixtureKeys::new(Absent, Present, Absent, Present).is_err());
    let keys = FixtureKeys::new(Absent, Absent, Absent, Absent).expect("empty keys");
    assert!(
        FixtureImage::new(
            keys,
            Some(FixtureValue::new(1, vec![]).expect("empty present value")),
            None,
            None,
            None
        )
        .is_err()
    );
}

#[test]
fn a_path_slot_enforces_its_own_limit_without_native_access() {
    use KeyPresence::{Absent, Present};
    let keys = FixtureKeys::new(Present, Absent, Absent, Absent).expect("Environment");
    let oversized = FixtureValue::new(1, vec![0; (1 << 20) + 1]).expect("below outer raw bound");
    assert!(FixtureImage::new(keys, Some(oversized), None, None, None).is_err());
}

#[test]
fn the_independent_wal4_empty_state_is_decoded_without_changing_raw_bytes() {
    use KeyPresence::{Absent, Present};
    let mut wal = b"P4KXPATH\x04".to_vec();
    wal.extend_from_slice(&[0; 8]);
    wal.extend_from_slice(&[0; 5]);
    let keys = FixtureKeys::new(Absent, Present, Present, Absent).expect("state keys");
    let image = FixtureImage::new(
        keys,
        None,
        Some(FixtureValue::new(3, wal.clone()).expect("WAL")),
        Some(FixtureValue::new(1, vec![b'0', 0, 0, 0]).expect("Sequence")),
        None,
    )
    .expect("image");
    assert!(matches!(
        image.inspect_core(),
        CoreInspection::Valid {
            sequence: 0,
            phase: None,
            owner: None,
            ..
        }
    ));
    assert_eq!(image.state.as_ref().expect("retained state").bytes(), wal);
}

#[test]
fn malformed_core_envelopes_are_captured_but_not_claimed_as_valid() {
    use KeyPresence::{Absent, Present};
    let keys = FixtureKeys::new(Absent, Present, Present, Absent).expect("state keys");
    let image = FixtureImage::new(
        keys,
        None,
        None,
        Some(FixtureValue::new(2, vec![b'0', 0, 0, 0]).expect("wrong Sequence kind")),
        None,
    )
    .expect("raw image");
    assert!(matches!(
        image.inspect_core(),
        CoreInspection::Rejected { .. }
    ));
}

fn reserved_image(sealed: bool) -> Result<FixtureImage, Box<dyn std::error::Error>> {
    use KeyPresence::{Absent, Present};
    let mut wal = b"P4KXPATH\x04".to_vec();
    wal.extend_from_slice(&0u64.to_le_bytes());
    wal.extend_from_slice(&[0, 0, 0, 1]);
    wal.extend_from_slice(&0u64.to_le_bytes());
    wal.extend_from_slice(&1u128.to_le_bytes());
    wal.extend_from_slice(&2u128.to_le_bytes());
    wal.extend_from_slice(&[9; 32]);
    if sealed {
        wal.extend_from_slice(&[1, 0, 1]);
        let lease: Vec<u8> = r"C:\P4KX\"
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect();
        wal.extend_from_slice(&u32::try_from(lease.len())?.to_le_bytes());
        wal.extend_from_slice(&lease);
    } else {
        wal.push(0);
    }
    wal.push(0);
    let binding = format!("v1|0|{:032x}|{:032x}|{}", 1, 2, "09".repeat(32));
    let raw_binding: Vec<u8> = binding
        .encode_utf16()
        .chain(std::iter::once(0))
        .flat_map(u16::to_le_bytes)
        .collect();
    Ok(FixtureImage::new(
        FixtureKeys::new(Absent, Present, Present, Present)?,
        None,
        Some(FixtureValue::new(3, wal)?),
        Some(FixtureValue::new(1, vec![b'0', 0, 0, 0])?),
        Some(FixtureValue::new(1, raw_binding)?),
    )?)
}

#[test]
fn independently_authored_wal4_reservations_report_neutral_and_sealed_without_a_receipt()
-> Result<(), Box<dyn std::error::Error>> {
    use super::super::ReservedPhase;
    for sealed in [false, true] {
        let inspection = reserved_image(sealed)?.inspect_core();
        assert!(
            matches!(inspection, CoreInspection::Valid { sequence: 0, phase: None, owner: None, reserved_binding: Some(_), reservation_phase: Some(phase), .. } if matches!((sealed, phase), (false, ReservedPhase::Neutral) | (true, ReservedPhase::Sealed))),
            "the actual codec distinguishes identity reservation from sealed intent without inventing Prepared"
        );
    }
    Ok(())
}
