// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Objects are decoded only for the bank versions whose layout is known, and `HIRC` type numbers
//! depend on the version.

use p4kx_audio::{BankError, HircKind, HircValue, SectionId, Soundbank};
use p4kx_testkit::{TestBank, TestContainerKind, TestHircObject, TestNodeBase};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn an_event_of_version_160_is_unsupported_not_misdecoded() -> TestResult {
    let bytes = TestBank::new(1)
        .version(160)
        .wem(5, [1, 2, 3])
        .object(TestHircObject::event(9, &[10, 11]))
        .bytes();

    let bank = Soundbank::parse(bytes.as_slice())?;

    let event = bank.object(9).ok_or("event 9")?;
    assert_eq!(event.kind, HircKind::Event);
    assert_eq!(
        event.value,
        HircValue::Malformed(BankError::UnsupportedObjectLayout {
            kind: HircKind::Event,
            version: 160
        })
    );
    assert_eq!(
        event.error().and_then(BankError::section),
        Some(SectionId::HIRC)
    );
    assert_eq!(
        bank.object_body(event).len(),
        1 + 8 + 4,
        "the raw body is kept"
    );
    assert_eq!(bank.wem(5).map(|wem| wem.bytes().len()), Some(3), "WEM 5");
    Ok(())
}

#[test]
fn every_decoded_kind_is_unsupported_above_version_154() -> TestResult {
    let objects = [
        (HircKind::Sound, TestHircObject::sound(1, 2, 0, 3)),
        (HircKind::Action, TestHircObject::action(2, 0x0403, 1)),
        (
            HircKind::RandomSequenceContainer,
            TestHircObject::container(TestContainerKind::Random, 3, &[1], TestNodeBase::Plain),
        ),
    ];
    for version in [155, 168, 65_535] {
        for (kind, object) in &objects {
            let bytes = TestBank::new(1)
                .version(version)
                .object(object.clone())
                .bytes();
            let bank = Soundbank::parse(bytes.as_slice())?;
            let value = bank.objects().first().map(|object| object.value.clone());
            assert_eq!(
                value,
                Some(HircValue::Malformed(BankError::UnsupportedObjectLayout {
                    kind: *kind,
                    version
                })),
                "{kind:?} in version {version}"
            );
        }
    }
    Ok(())
}

#[test]
fn objects_that_are_never_decoded_stay_raw_in_any_version() -> TestResult {
    let bytes = TestBank::new(1)
        .version(168)
        .object(TestHircObject::raw(0xa8, 1, [1, 2]))
        .bytes();

    let bank = Soundbank::parse(bytes.as_slice())?;

    assert_eq!(
        bank.objects().first().map(|object| &object.value),
        Some(&HircValue::Raw)
    );
    Ok(())
}

#[test]
fn type_numbers_up_to_version_126_follow_the_old_numbering() {
    let old = [
        (0x0F, HircKind::DialogueEvent),
        (0x10, HircKind::Other(0x10)),
        (0x11, HircKind::Other(0x11)),
        (0x12, HircKind::FxShareSet),
        (0x13, HircKind::FxCustom),
        (0x14, HircKind::AuxiliaryBus),
        (0x15, HircKind::Lfo),
        (0x16, HircKind::Envelope),
        (0x17, HircKind::AudioDevice),
        (0x18, HircKind::Other(0x18)),
    ];
    for (byte, kind) in old {
        assert_eq!(HircKind::of(126, byte), kind, "{byte:#04x} in version 126");
    }
    let new = [
        (0x10, HircKind::FxShareSet),
        (0x11, HircKind::FxCustom),
        (0x12, HircKind::AuxiliaryBus),
        (0x13, HircKind::Lfo),
        (0x14, HircKind::Envelope),
        (0x15, HircKind::AudioDevice),
        (0x16, HircKind::TimeModulator),
        (0x17, HircKind::Other(0x17)),
    ];
    for version in [128, 154] {
        for (byte, kind) in new {
            assert_eq!(
                HircKind::of(version, byte),
                kind,
                "{byte:#04x} in version {version}"
            );
        }
    }
}

#[test]
fn a_bank_reads_its_type_numbers_by_its_own_version() -> TestResult {
    let bytes = TestBank::new(1)
        .version(120)
        .object(TestHircObject::raw(0x14, 7, []))
        .bytes();

    let bank = Soundbank::parse(bytes.as_slice())?;

    assert_eq!(
        bank.object(7).map(|object| object.kind),
        Some(HircKind::AuxiliaryBus)
    );
    Ok(())
}

#[test]
fn the_unsupported_layout_message_names_the_type_and_the_version() {
    let error = BankError::UnsupportedObjectLayout {
        kind: HircKind::Event,
        version: 160,
    };

    assert_eq!(
        error.to_string(),
        "the soundbank section HIRC holds an object of type Event in version 160, whose layout is not known"
    );
}
