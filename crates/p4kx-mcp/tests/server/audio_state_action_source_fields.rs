// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{TestBank, TestEntry, TestHircObject};
use serde_json::{Value, json};

use crate::audio::serve;
use crate::support::{Install, TestResult};

const BANK: &str = "Data/Sounds/wwise/Source.bnk";
const NONFINITE_BITS: u32 = 0x7FC1_1234;

#[test]
fn standalone_state_sources_keep_old_layout_and_exact_encrypted_words() -> TestResult {
    let mut original = 2_u16.to_le_bytes().to_vec();
    original.extend([7_u16, 7].into_iter().flat_map(u16::to_le_bytes));
    original.extend(
        [(-1.0_f32).to_bits(), NONFINITE_BITS]
            .into_iter()
            .flat_map(u32::to_le_bytes),
    );
    let install = install(145, TestHircObject::raw(1, 9, original))?;
    let mut server = serve(&install)?;
    let listing = server.ok("list_soundbank_objects", &json!({"soundbank": BANK}))?;
    let object = listing.pointer("/objects/0").ok_or("State object")?;
    assert_eq!(
        object.pointer("/detail/property_count"),
        Some(&json!(2)),
        "legacy summary"
    );
    let source = object.get("source_fields").ok_or("original State fields")?;
    for (pointer, expected) in [
        ("/layout", json!("decoded")),
        ("/version", json!(145)),
        ("/bundle/property_count", json!(2)),
        ("/bundle/properties/0/id", json!(7)),
        ("/bundle/properties/1/id", json!(7)),
        ("/bundle/properties/1/value_bits", json!(NONFINITE_BITS)),
        ("/bundle/properties/1/value", Value::Null),
    ] {
        check(source, pointer, &expected)?;
    }
    let tool = server
        .tools()?
        .into_iter()
        .find(|tool| tool.get("name") == Some(&json!("list_soundbank_objects")))
        .ok_or("soundbank objects tool")?;
    assert!(
        tool.pointer("/outputSchema/$defs/StateObjectSourceFields")
            .is_some(),
        "typed State schema"
    );
    assert!(
        tool.pointer("/outputSchema/$defs/ActionObjectSourceFields")
            .is_some(),
        "typed Action schema"
    );
    Ok(())
}

#[test]
fn unsupported_and_malformed_standalone_state_have_distinct_raw_dispositions() -> TestResult {
    for (version, layout) in [(154, "unsupported"), (145, "malformed")] {
        let install = install(version, TestHircObject::raw(1, 9, [1, 0, 7, 0]))?;
        let mut server = serve(&install)?;
        let listing = server.ok("list_soundbank_objects", &json!({"soundbank": BANK}))?;
        let source = listing
            .pointer("/objects/0/source_fields")
            .ok_or("original State fields")?;
        check(source, "/layout", &json!(layout))?;
        assert!(source.get("bundle").is_none(), "no successful empty bundle");
        assert!(
            source
                .pointer("/raw_body_range/value/end_exclusive")
                .is_some(),
            "raw original body"
        );
    }
    Ok(())
}

#[test]
fn play_action_sources_keep_raw_flags_unknown_properties_and_explicit_zero_bank() -> TestResult {
    let install = install(154, TestHircObject::raw(3, 9, play_action()))?;
    let mut server = serve(&install)?;
    let listing = server.ok("list_soundbank_objects", &json!({"soundbank": BANK}))?;
    let object = listing.pointer("/objects/0").ok_or("Play Action")?;
    assert_eq!(
        object.pointer("/detail/soundbank_id"),
        Some(&json!(0)),
        "legacy Play bank"
    );
    let source = object
        .get("source_fields")
        .ok_or("original Play Action fields")?;
    for (pointer, expected) in [
        ("/layout", json!("decoded")),
        ("/target_id", json!(10)),
        ("/bank_id", json!(0)),
        ("/play/target_flags", json!(0x82)),
        ("/play/fade_flags", json!(0xFD)),
        ("/play/properties/rows/0/property_id", json!(0xF1)),
        ("/play/properties/rows/0/scalar_bits", json!(NONFINITE_BITS)),
        (
            "/play/ranged_modifiers/rows/0/minimum_bits",
            json!(0x8000_0000_u32),
        ),
    ] {
        check(source, pointer, &expected)?;
    }
    let other = listing
        .pointer("/objects/1/source_fields")
        .ok_or("non-Play action disposition")?;
    check(other, "/layout", &json!("absent"))?;
    assert!(
        other.get("play").is_none(),
        "non-Play has no planar property bundle"
    );
    Ok(())
}

#[test]
fn truncated_play_properties_retain_checked_words_and_report_malformed() -> TestResult {
    let mut body = play_action();
    body.truncate(size_of::<u16>() + size_of::<u32>() + 3 + size_of::<u32>());
    let install = install(154, TestHircObject::raw(3, 9, body))?;
    let mut server = serve(&install)?;
    let listing = server.ok("list_soundbank_objects", &json!({"soundbank": BANK}))?;
    let source = listing
        .pointer("/objects/0/source_fields")
        .ok_or("original Play prefix")?;
    check(source, "/layout", &json!("malformed"))?;
    check(
        source,
        "/play/properties/rows/0/scalar_bits",
        &json!(NONFINITE_BITS),
    )?;
    assert!(
        source.pointer("/play/ranged_modifiers").is_none(),
        "the next count was unreadable"
    );
    assert!(
        source.pointer("/play/fade_flags").is_none(),
        "later flags were unreadable"
    );
    Ok(())
}

#[test]
fn a_truncated_play_bank_tail_does_not_invalidate_complete_properties() -> TestResult {
    let mut body = play_action();
    body.truncate(body.len() - size_of::<u32>());
    let install = install(154, TestHircObject::raw(3, 9, body))?;
    let mut server = serve(&install)?;
    let listing = server.ok("list_soundbank_objects", &json!({"soundbank": BANK}))?;
    let object = listing
        .pointer("/objects/0")
        .ok_or("original Play object")?;
    assert_eq!(
        object.get("status"),
        Some(&json!("malformed")),
        "the bank type is truncated"
    );
    let source = object.get("source_fields").ok_or("original Play fields")?;
    check(source, "/layout", &json!("decoded"))?;
    check(source, "/play/fade_flags", &json!(0xFD))?;
    check(source, "/bank_id", &json!(0))?;
    assert!(
        source.get("bank_type").is_none(),
        "unread bank type is omitted"
    );
    Ok(())
}

fn play_action() -> Vec<u8> {
    let mut body = 0x0403_u16.to_le_bytes().to_vec();
    body.extend(10_u32.to_le_bytes());
    body.extend([0x82, 1, 0xF1]);
    body.extend(NONFINITE_BITS.to_le_bytes());
    body.extend([1, 0xFA]);
    body.extend([0x8000_0000_u32, 0].into_iter().flat_map(u32::to_le_bytes));
    body.push(0xFD);
    body.extend([0; 8]);
    body
}

fn install(version: u32, object: TestHircObject) -> TestResult<Install> {
    let bank = TestBank::new(20)
        .version(version)
        .object(object)
        .object(TestHircObject::action(11, 0x0102, 10))
        .bytes();
    Install::new(vec![TestEntry::zstd(BANK, bank).encrypted()])
}

fn check(source: &Value, pointer: &str, expected: &Value) -> TestResult {
    let datum = source.pointer(pointer).ok_or("observed field")?;
    assert_eq!(datum.get("value"), Some(expected), "{pointer}");
    assert_eq!(
        datum.get("containsEncryptedSource"),
        Some(&json!(true)),
        "{pointer} encryption"
    );
    assert_eq!(
        datum.get("sourceSetComplete"),
        Some(&json!(true)),
        "{pointer} coverage"
    );
    let references = datum["sourceUsesRefs"].as_array().ok_or("source uses")?;
    let inventory = source
        .pointer("/source_inventory/uses")
        .and_then(Value::as_array)
        .ok_or("inventory")?;
    assert!(!references.is_empty(), "physical field receipt");
    assert!(
        references.iter().all(|reference| inventory
            .iter()
            .any(|used| used.get("id") == Some(reference))),
        "all original field receipts resolve"
    );
    Ok(())
}
