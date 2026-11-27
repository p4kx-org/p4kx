// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_audio::{
    BankHircBus, BankHircField, BankHircSemantic, BankNodeFxKind, BankNodePropertyExtent,
    HircLayout, HircObject, Soundbank,
};
use p4kx_testkit::{TestBank, TestHircObject};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const BUS: u8 = 8;
const AUX: u8 = 18;
const VERSION: u32 = 154;
const ID: u32 = 29;
const DUCK_VOLUME: u32 = 0x7fc1_2345;

#[test]
fn v154_buses_keep_exact_optional_routes_fields_and_declared_groups() -> TestResult {
    for (kind, parent) in [(BUS, 0), (AUX, 37)] {
        let bytes = bank(kind, VERSION, &body(parent));
        let original = Soundbank::parse(&bytes)?;
        original
            .with_hirc_source_fields(0, |_, original| {
                let BankHircSemantic::Bus(mut bus) = original else {
                    return Err("the exact version owner must expose typed bus fields".into());
                };
                assert_route_and_primary(&bus, parent);
                assert_duck_and_aux(&mut bus)?;
                assert_effects_and_state(&mut bus)
            })?
            .ok_or("original HIRC occurrence")??;
    }
    Ok(())
}

fn assert_route_and_primary<R>(bus: &BankHircBus<'_, R>, parent: u32) {
    let original = bus.fields();
    assert!(
        bus.format_error().is_none(),
        "valid layout consumes the complete bus body"
    );
    assert!(
        bus.raw_remainder().is_empty(),
        "no unread body suffix remains"
    );
    verify_word(
        original.override_bus_id.as_ref(),
        Some(parent),
        "the original routing word",
    );
    verify_word(
        original.device_shareset_id.as_ref(),
        (parent == 0).then_some(0),
        "absent child slot and authored root zero stay distinct",
    );
    verify_word(
        original.settings.playback_flags.as_ref(),
        Some(0x80),
        "unknown playback bit",
    );
    verify_word(
        original.settings.max_instances.as_ref(),
        Some(0),
        "explicit zero limit",
    );
    verify_word(
        original.settings.channel_config_bits.as_ref(),
        Some(0xa120_abcd),
        "the exact packed config",
    );
    verify_word(
        original.settings.hdr_flags.as_ref(),
        Some(0xf4),
        "unknown HDR bits",
    );
    verify_word(
        original.recovery_time.as_ref(),
        Some(-31),
        "the original signed recovery word",
    );
    verify_word(
        original.max_duck_volume_bits.as_ref(),
        Some(0xff80_0000),
        "nonfinite maximum raw word",
    );
    assert!(
        original
            .properties
            .as_ref()
            .is_some_and(BankNodePropertyExtent::is_complete),
        "physical bus property list"
    );
    assert!(
        original.positioning.complete_range().is_some(),
        "shared positioning span"
    );
}

fn verify_word<T: std::fmt::Debug + PartialEq + Copy>(
    field: Option<&BankHircField<T>>,
    expected: Option<T>,
    label: &str,
) {
    assert_eq!(field.map(|field| field.value), expected, "{label}");
}

fn assert_duck_and_aux<R: std::io::Read + std::io::Seek>(
    bus: &mut BankHircBus<'_, R>,
) -> TestResult {
    assert_eq!(
        bus.user_aux_buses()?
            .and_then(|rows| rows.into_iter().next().flatten())
            .map(|value| value.value),
        Some(0),
        "the declared aux slot retains its zero ID",
    );
    let duck = bus
        .ducks()
        .and_then(|mut rows| rows.next())
        .transpose()?
        .ok_or("first original duck target")?;
    assert_eq!(duck.ordinal, 0, "original list index");
    assert_eq!(duck.bus_id.value, 0, "explicit zero target");
    assert_eq!(
        duck.volume_bits.value, DUCK_VOLUME,
        "nonfinite original duck word"
    );
    assert_eq!(
        (duck.fade_out_time.value, duck.fade_in_time.value),
        (-8, 17),
        "independent fade words"
    );
    assert_eq!(
        (duck.fade_curve.value, duck.target_property.value),
        (0xf3, 0xf6),
        "unknown original curve and property codes"
    );
    Ok(())
}

fn assert_effects_and_state<R: std::io::Read + std::io::Seek>(
    bus: &mut BankHircBus<'_, R>,
) -> TestResult {
    for (kind, index, bits) in [
        (BankNodeFxKind::Effects, 3, 0x81),
        (BankNodeFxKind::Metadata, 2, 0xb5),
    ] {
        let fx = bus.fields().fx(kind);
        assert!(
            fx.fields.override_parent_bits.is_none(),
            "the bus header does not contain a parent override"
        );
        assert_eq!(
            fx.declared_count.map(|value| value.value),
            Some(1),
            "the original slot count follows the actual bus header"
        );
        assert!(
            fx.complete_range().is_some(),
            "exact original Bus group extent"
        );
        let original = bus
            .fx_slots(kind)
            .and_then(|mut rows| rows.next())
            .transpose()?
            .ok_or("one complete original slot")?;
        assert_eq!(
            original.index.value, index,
            "distinct effect and metadata indices"
        );
        assert_eq!(
            original.bits.map(|value| value.value),
            Some(bits),
            "unmodified original flags"
        );
    }
    assert!(
        bus.rtpc()
            .is_some_and(|rows| rows.checked_curve_count() == 1),
        "one original RTPC"
    );
    assert!(
        bus.states().is_some(),
        "shared state cursor reads the original populated chunk"
    );
    Ok(())
}

#[test]
fn bus_short_lists_and_trailing_bytes_keep_the_next_object_and_raw_extent() -> TestResult {
    let complete = body(0);
    for suffix in [&[][..], &[0xaf][..]] {
        let mut first = complete.clone();
        if suffix.is_empty() {
            const ROOT_DUCK_SLOT_START: usize = 57;
            const INCOMPLETE_DUCK_SLOT_BYTES: usize = 17;
            first.truncate(ROOT_DUCK_SLOT_START + INCOMPLETE_DUCK_SLOT_BYTES);
        } else {
            first.extend(suffix);
        }
        let bytes = bank(BUS, VERSION, &first);
        let original = Soundbank::parse(&bytes)?;
        original
            .with_hirc_source_fields(0, |_, source| -> TestResult {
                let BankHircSemantic::Bus(bus) = source else {
                    return Err("malformed object retains its typed prefix".into());
                };
                assert!(
                    bus.format_error().is_some(),
                    "structural suffix is explicitly reported"
                );
                let remainder = bus.raw_remainder();
                assert!(
                    remainder.start <= remainder.end,
                    "checked exact half-open remainder"
                );
                assert_eq!(
                    remainder.end,
                    bus.object().body_end_exclusive(),
                    "the raw remainder cannot escape the object"
                );
                if suffix.is_empty() {
                    assert_eq!(
                        remainder.end - remainder.start,
                        17,
                        "the incomplete duck slot is preserved raw"
                    );
                } else {
                    assert_eq!(
                        remainder.end - remainder.start,
                        1,
                        "one raw byte after an otherwise complete bus"
                    );
                }
                Ok(())
            })?
            .ok_or("physical bus object")??;
        assert_eq!(
            original.objects().get(1).map(|object| object.id),
            Some(ID),
            "the next original object with the same short ID remains independent"
        );
    }
    Ok(())
}

#[test]
fn damaged_bus_counts_and_rtpc_varints_stay_inside_their_object() -> TestResult {
    const ROOT_DUCK_COUNT_START: usize = 53;
    const ROOT_RTPC_PARAMETER_START: usize = 98;
    let corruptions: [(usize, &[u8]); 2] = [
        (ROOT_DUCK_COUNT_START, &u32::MAX.to_le_bytes()),
        (ROOT_RTPC_PARAMETER_START, &[0x80; 5]),
    ];
    for (start, invalid) in corruptions {
        let mut first = body(0);
        first
            .get_mut(start..start + invalid.len())
            .ok_or("original test field span")?
            .copy_from_slice(invalid);
        let bytes = bank(BUS, VERSION, &first);
        let original = Soundbank::parse(&bytes)?;
        assert_eq!(
            original.objects().first().map(HircObject::layout),
            Some(HircLayout::Malformed),
            "eager layout accounting agrees with the physical reader"
        );
        original
            .with_hirc_source_fields(0, |_, source| -> TestResult {
                let BankHircSemantic::Bus(bus) = source else {
                    return Err("the checked prefix remains inspectable".into());
                };
                assert!(bus.format_error().is_some(), "explicit malformed status");
                assert_eq!(
                    bus.raw_remainder().end,
                    bus.object().body_end_exclusive(),
                    "remaining bytes stop at the current owner"
                );
                if start == ROOT_DUCK_COUNT_START {
                    assert_eq!(
                        bus.fields().duck_count.as_ref().map(|count| count.value),
                        Some(u32::MAX)
                    );
                    assert!(bus.fields().duck_rows.as_ref().is_some_and(|rows| {
                        rows.end - rows.start <= u64::try_from(first.len()).unwrap_or_default()
                    }));
                }
                Ok(())
            })?
            .ok_or("first original object")??;
        assert_eq!(
            original.objects().get(1).map(HircObject::layout),
            Some(HircLayout::Decoded),
            "the following valid object still decodes"
        );
    }
    Ok(())
}

#[test]
fn bus_source_opt_in_keeps_unsupported_versions_byte_identical() -> TestResult {
    for (kind, version) in [(BUS, 153), (AUX, 155), (0x14, 126)] {
        let fields = body(0);
        let bytes = bank(kind, version, &fields);
        let original = Soundbank::parse(&bytes)?;
        original
            .with_hirc_source_fields(0, |_, source| {
                assert!(
                    matches!(source, BankHircSemantic::Raw(_)),
                    "other versions remain raw"
                );
            })?
            .ok_or("physical occurrence with unsupported layout")?;
        let physical = original
            .objects()
            .first()
            .ok_or("original untyped object")?;
        let mut expected = ID.to_le_bytes().to_vec();
        expected.extend(fields);
        assert_eq!(
            original.object_body(physical),
            expected,
            "raw bytes include the source ID and exact body"
        );
    }
    Ok(())
}

fn bank(kind: u8, version: u32, fields: &[u8]) -> Vec<u8> {
    TestBank::new(1)
        .version(version)
        .object(TestHircObject::raw(kind, ID, fields))
        .object(TestHircObject::event(ID, &[]))
        .bytes()
}

pub(super) fn body(parent: u32) -> Vec<u8> {
    let mut bytes = parent.to_le_bytes().to_vec();
    if parent == 0 {
        bytes.extend(0_u32.to_le_bytes());
    }
    bytes.extend([1, 0xf4]);
    bytes.extend(0x8000_0000_u32.to_le_bytes());
    bytes.extend([3, 0xa0, 0x98]);
    bytes.extend([0_u32, 2, 3, 4, 0].into_iter().flat_map(u32::to_le_bytes));
    bytes.push(0x80);
    bytes.extend(0_u16.to_le_bytes());
    bytes.extend(0xa120_abcd_u32.to_le_bytes());
    bytes.push(0xf4);
    bytes.extend((-31_i32).to_le_bytes());
    bytes.extend(
        [0xff80_0000_u32, 1, 0, DUCK_VOLUME]
            .into_iter()
            .flat_map(u32::to_le_bytes),
    );
    bytes.extend([-8_i32, 17].into_iter().flat_map(i32::to_le_bytes));
    bytes.extend([0xf3, 0xf6, 1, 0x80, 3]);
    bytes.extend(0_u32.to_le_bytes());
    bytes.extend([0x81, 1, 2]);
    bytes.extend(0_u32.to_le_bytes());
    bytes.push(0xb5);
    append_rtpc_and_states(&mut bytes);
    bytes
}

fn append_rtpc_and_states(bytes: &mut Vec<u8>) {
    bytes.extend(1_u16.to_le_bytes());
    bytes.extend(55_u32.to_le_bytes());
    bytes.extend([0x72, 0x98, 0x82, 0x2c]);
    bytes.extend(0_u32.to_le_bytes());
    bytes.push(0xfa);
    bytes.extend(1_u16.to_le_bytes());
    bytes.extend(
        [0xff80_0000_u32, 0x8000_0000, 0xffff_fff1]
            .into_iter()
            .flat_map(u32::to_le_bytes),
    );
    bytes.extend([1, 0x82, 0x2c, 0x11, 0x82, 1]);
    bytes.extend(12_u32.to_le_bytes());
    bytes.extend([0xef, 1]);
    bytes.extend(4_u32.to_le_bytes());
    bytes.extend([1_u16, 4].into_iter().flat_map(u16::to_le_bytes));
    bytes.extend(0x7fc0_1234_u32.to_le_bytes());
}
