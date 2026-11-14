// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::io::Cursor;

use p4kx_audio::{
    BankHircSemantic, BankHircSound, BankSoundContinuation, BankSoundRead, BankSource, HircValue,
    Soundbank,
};
use p4kx_testkit::{TestBank, TestHircObject};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

const SOUND_TAG: u8 = 2;
const SOUND_ID: u32 = 53;
const MEDIA_ID: u32 = 27;
const MEDIA_SIZE: u32 = 91;
const CACHE_ID: u32 = MEDIA_ID ^ 0x5a5a_5a5a;
const PARENT_ID: u32 = 0x2c21_1b2e;

#[test]
fn sound_source_matches_legacy_and_preserves_the_original_cache_and_parent() -> TestResult {
    for version in [150, 151, 153, 154] {
        let object = if version == 150 {
            TestHircObject::sound_without_cache_id(SOUND_ID, MEDIA_ID, 2, MEDIA_SIZE)
        } else {
            TestHircObject::sound(SOUND_ID, MEDIA_ID, 2, MEDIA_SIZE)
        };
        let bytes = bank(version, object);
        let legacy = Soundbank::parse(bytes.as_slice())?;
        let value = &legacy.object(SOUND_ID).ok_or("legacy Sound")?.value;
        let semantic = sound(&bytes)?.ok_or("bounded Sound")?;
        let BankSoundRead::Decoded(source) = semantic.status else {
            return Err("known Sound layout".into());
        };
        assert_eq!(value, &HircValue::Sound(source), "same decoded value");
        let fields = semantic.fields;
        let start = semantic.object.body_offset() + 4;
        assert_eq!(
            fields.cache_id.map(|word| (word.value, word.range)),
            (version >= 151).then_some((CACHE_ID, start + 9..start + 13)),
            "version-selected original cache word"
        );
        if version == 154 {
            let node = fields.node.and_then(|node| node.base).ok_or("node base")?;
            let parent = node.fields.parent_id.ok_or("original parent")?;
            assert_eq!(parent.value, PARENT_ID, "original parent word");
            assert_eq!(parent.range, start + 26..start + 30, "exact parent bytes");
            assert!(
                node.trailing.is_some_and(|range| range.is_empty()),
                "complete node traversal"
            );
        } else {
            let BankSoundContinuation::Opaque { range } = fields.continuation else {
                return Err("opaque range of an earlier Sound layout".into());
            };
            assert_eq!(
                range,
                start + if version == 150 { 14 } else { 18 }..semantic.object.body_end_exclusive(),
                "exact remaining old layout bytes"
            );
        }
    }
    Ok(())
}

#[test]
fn absent_sound_flags_remain_distinct_from_authored_zero() -> TestResult {
    for bits in [None, Some(0)] {
        let body = source_body(bits);
        let bytes = bank(154, TestHircObject::raw(SOUND_TAG, SOUND_ID, body));
        let semantic = sound(&bytes)?.ok_or("bounded Sound")?;
        let BankSoundRead::Decoded(source) = semantic.status else {
            return Err("optional flags and node are absent".into());
        };
        assert_eq!(source.source_bits, 0, "legacy default");
        assert_eq!(
            semantic.fields.source_bits.map(|word| word.value),
            bits,
            "only original bytes have a receipt"
        );
        assert!(
            matches!(semantic.fields.continuation, BankSoundContinuation::Absent),
            "no node is authored"
        );
    }
    Ok(())
}

#[test]
fn malformed_plugin_parameters_keep_the_exact_length_and_legacy_failure() -> TestResult {
    let mut body = source_body(Some(0xe1));
    body.extend_from_slice(&12_u32.to_le_bytes());
    body.extend_from_slice(&[1, 2]);
    let bytes = bank(154, TestHircObject::raw(SOUND_TAG, SOUND_ID, body));
    let semantic = sound(&bytes)?.ok_or("bounded Sound")?;
    let BankSoundRead::Malformed(error) = semantic.status else {
        return Err("incomplete plug-in parameters".into());
    };
    let legacy = Soundbank::parse(bytes.as_slice())?;
    let expected = legacy.object(SOUND_ID).ok_or("legacy Sound")?.error();
    assert_eq!(Some(&error), expected, "same original format failure");
    let node = semantic.fields.node.ok_or("selected continuation")?;
    let length = node.plugin_parameter_length.ok_or("readable length")?;
    let end = semantic.object.body_end_exclusive();
    assert_eq!(length.value, 12, "original authored length");
    assert_eq!(length.range, end - 6..end - 2, "exact length coordinates");
    assert!(node.plugin_parameters.is_none(), "unadmitted extent");
    assert!(node.base.is_none(), "node traversal never started");
    Ok(())
}

#[test]
fn unsupported_and_older_sound_layouts_retain_the_physical_object() -> TestResult {
    let old = bank(149, TestHircObject::raw(SOUND_TAG, SOUND_ID, []));
    assert!(sound(&old)?.is_none(), "older Sound stays raw");
    let future = bank(155, TestHircObject::raw(SOUND_TAG, SOUND_ID, []));
    let semantic = sound(&future)?.ok_or("unsupported Sound receipt")?;
    assert!(
        matches!(semantic.status, BankSoundRead::Unsupported(_)),
        "future layout is explicit"
    );
    assert_eq!(semantic.object.id(), SOUND_ID, "original envelope survives");
    assert!(semantic.fields.plugin_id.is_none(), "layout was never read");
    Ok(())
}

fn source_body(bits: Option<u8>) -> Vec<u8> {
    let mut body = 0x0004_0002_u32.to_le_bytes().to_vec();
    body.push(0);
    for word in [MEDIA_ID, CACHE_ID, MEDIA_SIZE] {
        body.extend_from_slice(&word.to_le_bytes());
    }
    body.extend(bits);
    body
}

fn bank(version: u32, object: TestHircObject) -> Vec<u8> {
    TestBank::new(71).version(version).object(object).bytes()
}

fn sound(bytes: &[u8]) -> TestResult<Option<Box<BankHircSound>>> {
    let mut source = BankSource::open(Cursor::new(bytes), u64::try_from(bytes.len())?)?;
    let mut sections = source.section_cursor();
    let header = source
        .next_section(&mut sections)
        .transpose()?
        .ok_or("BKHD")?;
    let hirc = source
        .next_section(&mut sections)
        .transpose()?
        .ok_or("HIRC")?;
    let observed = source.read_observed_header_for(&header)?;
    let mut hierarchy = source.hirc(&hirc)?;
    match hierarchy.next_semantic(&observed).transpose()? {
        Some(BankHircSemantic::Sound(sound)) => Ok(Some(sound)),
        Some(BankHircSemantic::Raw(object)) if object.id() == SOUND_ID => Ok(None),
        _ => Err("expected a physical Sound object".into()),
    }
}
