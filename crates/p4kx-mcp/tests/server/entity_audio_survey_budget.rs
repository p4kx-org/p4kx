// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{
    DataForgeBuilder, TestAtl, TestBank, TestEntity, TestEntry, TestHircObject, audio_schema,
    entity_schema, wwise_short_id,
};
use serde_json::{Value, json};

use crate::audio::serve;
use crate::audio_origin_assertions as proof;
use crate::support::{Install, TestResult};

const BANK_COUNT: u32 = 900;
const ENTITY: &str = "TEST_Audio_Survey";
const EVENT: &str = "Play_Unheld_Event";
const ATL: &str = "Data/Libs/GameAudio/SC_Survey.xml";
const EVENT_ROW: &str = "/structuredContent/triggers/0/events/0";
const COUNT_FIELD: &str = "/structuredContent/triggers/0/events/0/soundbank_count";
const RESPONSE_LIMIT: usize = 256 * 1024;

#[test]
fn entity_audio_keeps_exact_encrypted_witness_when_the_bank_survey_exceeds_budget() -> TestResult {
    let (install, witness_len) = large_bank_survey()?;
    let mut server = serve(&install)?;
    let reply = server.ok("get_entity_sounds", &json!({"entity": ENTITY, "limit": 1}))?;

    assert_eq!(reply.get("total"), Some(&json!(1)));
    assert_eq!(reply.get("issues_total"), Some(&json!(0)));
    assert_eq!(
        reply.pointer("/triggers/0/events/0/name"),
        Some(&json!(EVENT))
    );
    assert_eq!(
        reply.pointer("/triggers/0/events/0/id"),
        Some(&json!(wwise_short_id(EVENT)))
    );
    assert_eq!(
        reply.pointer("/triggers/0/events/0/soundbank_count"),
        Some(&json!(0))
    );
    assert!(
        serde_json::to_vec(&reply)?.len() <= RESPONSE_LIMIT,
        "the structured response fits with the receipt"
    );

    for member in ["name", "id"] {
        proof::assert_marker(&reply, &format!("{EVENT_ROW}/{member}"), false)?;
    }
    let origin = proof::origin(&reply, COUNT_FIELD)?;
    assert_eq!(origin.get("containsEncryptedSource"), Some(&json!(true)));
    assert_eq!(origin.get("sourceSetComplete"), Some(&json!(false)));
    assert_encrypted_witness(&reply, witness_len)
}

fn large_bank_survey() -> TestResult<(Install, usize)> {
    const TRIGGER: &str = "Play_Unheld";
    let entity =
        TestEntity::new(928, ENTITY, "entities/test_audio_survey.xml").audio_trigger(TRIGGER);
    let record = audio_schema(entity_schema(DataForgeBuilder::new()))
        .record(entity.record())
        .build();
    let definition = TestAtl::new("Survey")
        .localised_trigger(TRIGGER, EVENT)
        .to_cryxml();
    let mut entries = vec![
        TestEntry::stored("Data/Game2.dcb", record),
        TestEntry::stored(ATL, definition),
    ];

    let unrelated_id = wwise_short_id(EVENT) ^ 1;
    let bank = |ordinal| {
        TestBank::new(ordinal + 1)
            .object(TestHircObject::event(unrelated_id, &[]))
            .bytes()
    };
    let path = |ordinal| format!("Data/Sounds/wwise/Survey_{ordinal:04}.bnk");
    let witness = BANK_COUNT - 1;
    entries.extend((0..witness).map(|ordinal| TestEntry::stored(path(ordinal), bank(ordinal))));
    let encrypted = bank(witness);
    let witness_len = encrypted.len();
    entries.push(TestEntry::deflated(path(witness), encrypted).encrypted());
    Ok((Install::new(entries)?, witness_len))
}

fn assert_encrypted_witness(reply: &Value, length: usize) -> TestResult {
    let sites = proof::uses(reply, COUNT_FIELD)?;
    let encrypted = sites
        .iter()
        .find(|site| site.get("effectiveEncryption").and_then(Value::as_str) == Some("encrypted"))
        .ok_or("an encrypted admitted bank must witness the survey")?;
    assert_eq!(
        encrypted.get("role"),
        Some(&json!("selector")),
        "the inspected bank contributes to the no-match result"
    );
    assert_eq!(
        encrypted.pointer("/location/namespace"),
        Some(&json!("entryDecodedBytes")),
        "the witness points to the decoded source"
    );
    assert_eq!(
        encrypted.pointer("/location/indices"),
        Some(&json!(["0", length.to_string()])),
        "the recorded extent equals the bytes supplied to the bank parser"
    );

    let source = proof::physical_source(reply, encrypted)?;
    assert_eq!(
        source.get("storedEncryption"),
        Some(&json!("encrypted")),
        "the physical bank is stored encrypted"
    );
    assert_eq!(
        source.get("kind"),
        Some(&json!("archiveEntryProof")),
        "bounded evidence still identifies a physical archive Entry"
    );
    assert_eq!(
        source.get("entryIndex"),
        Some(&json!((BANK_COUNT + 1).to_string())),
        "the last physical Entry supplies the encrypted witness"
    );
    Ok(())
}
