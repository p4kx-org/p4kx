// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! `HIRC` objects decoded into typed values with their IDs.

use p4kx_audio::{
    ActionTarget, HircKind, HircValue, MediaStorage, PlayBank, SectionId, SoundSource, Soundbank,
};
use p4kx_testkit::{TestBank, TestHircObject};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const PLAY: u16 = 0x0403;

#[test]
fn objects_are_found_by_id_the_first_in_file_order_when_shared() -> TestResult {
    let bytes = TestBank::new(1)
        .object(TestHircObject::raw(8, 900, []))
        .object(TestHircObject::action(30, PLAY, 900))
        .object(TestHircObject::event(20, &[30]))
        .object(TestHircObject::raw(7, 30, []))
        .bytes();
    let bank = Soundbank::parse(bytes.as_slice())?;

    let event = bank.object(20).ok_or("event 20")?;
    assert_eq!(event.value, HircValue::Event(vec![30]), "event 20");
    assert_eq!(
        bank.object(30).map(|object| object.kind),
        Some(HircKind::Action),
        "the first object with ID 30"
    );
    assert_eq!(
        bank.object(900).map(|object| object.kind),
        Some(HircKind::AudioBus),
        "bus 900"
    );
    assert!(bank.object(31).is_none(), "no object 31");
    assert!(bank.object(0).is_none(), "no object 0");
    assert!(bank.object(u32::MAX).is_none(), "no object u32::MAX");
    Ok(())
}

#[test]
fn sounds_actions_and_events_are_decoded_with_their_ids() -> TestResult {
    let bytes = TestBank::new(1)
        .object(TestHircObject::sound(0x07B2_3F35, 617_647_119, 0, 252_487))
        .object(TestHircObject::sound(0x0848_042F, 809_049_904, 2, 0))
        .object(TestHircObject::action(0x0AC5_1C54, PLAY, 0x07B2_3F35))
        .object(TestHircObject::event(0x0367_9A46, &[0x0AC5_1C54]))
        .bytes();
    let bank = Soundbank::parse(bytes.as_slice())?;

    let objects: Vec<(HircKind, u32, &HircValue)> = bank
        .objects()
        .iter()
        .map(|object| (object.kind, object.id, &object.value))
        .collect();
    assert_eq!(
        objects,
        [
            (
                HircKind::Sound,
                0x07B2_3F35,
                &HircValue::Sound(SoundSource {
                    plugin_id: 0x0004_0001,
                    storage: MediaStorage::Embedded,
                    source_id: 617_647_119,
                    in_memory_size: 252_487,
                    source_bits: 0,
                    parent_id: Some(0x2C21_1B2E),
                })
            ),
            (
                HircKind::Sound,
                0x0848_042F,
                &HircValue::Sound(SoundSource {
                    plugin_id: 0x0004_0001,
                    storage: MediaStorage::Streamed,
                    source_id: 809_049_904,
                    in_memory_size: 0,
                    source_bits: 0,
                    parent_id: Some(0x2C21_1B2E),
                })
            ),
            (
                HircKind::Action,
                0x0AC5_1C54,
                &HircValue::Action(ActionTarget {
                    action_type: PLAY,
                    target_id: 0x07B2_3F35,
                    bank: Some(PlayBank {
                        bank_id: 0,
                        bank_type: 0
                    }),
                })
            ),
            (
                HircKind::Event,
                0x0367_9A46,
                &HircValue::Event(vec![0x0AC5_1C54])
            ),
        ],
        "typed objects"
    );
    Ok(())
}

#[test]
fn an_event_with_more_than_127_actions_reads_a_two_byte_count() -> TestResult {
    let actions: Vec<u32> = (1..=200).collect();
    let bytes = TestBank::new(1)
        .object(TestHircObject::event(9, &actions))
        .bytes();
    let bank = Soundbank::parse(bytes.as_slice())?;

    assert_eq!(
        bank.objects().first().map(|object| &object.value),
        Some(&HircValue::Event(actions)),
        "all 200 actions"
    );
    Ok(())
}

#[test]
fn other_objects_keep_their_kind_id_and_raw_body() -> TestResult {
    let bytes = TestBank::new(1)
        .version(153)
        .object(TestHircObject::raw(8, 0x1234, [9, 8, 7]))
        .object(TestHircObject::raw(0x30, 0x5678, []))
        .bytes();
    let bank = Soundbank::parse(bytes.as_slice())?;

    let [bus, unknown] = bank.objects() else {
        return Err("two objects".into());
    };
    assert_eq!(
        (bus.kind, bus.id, &bus.value),
        (HircKind::AudioBus, 0x1234, &HircValue::Raw),
        "audio bus"
    );
    assert_eq!(
        bank.object_body(bus),
        [0x34, 0x12, 0, 0, 9, 8, 7],
        "raw body"
    );
    assert_eq!(unknown.kind, HircKind::Other(0x30), "unknown kind");
    Ok(())
}

#[test]
fn a_sound_of_a_bank_before_version_150_stays_raw() -> TestResult {
    let bytes = TestBank::new(1)
        .version(145)
        .object(TestHircObject::sound(1, 2, 0, 3))
        .bytes();
    let bank = Soundbank::parse(bytes.as_slice())?;

    assert_eq!(
        bank.objects()
            .first()
            .map(|object| (object.kind, &object.value)),
        Some((HircKind::Sound, &HircValue::Raw)),
        "kind kept, body raw"
    );
    assert!(bank.section(SectionId::HIRC).is_some(), "HIRC section");
    Ok(())
}
