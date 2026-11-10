// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;
use std::error::Error;
use std::io::Read as _;

use p4kx_assembly::audio::{MAX_ATL_LEN, MAX_SOUNDBANK_LEN};
use p4kx_audio::{ActionTarget, HircValue, Soundbank};
use p4kx_extraction::Archive;
use p4kx_testkit::{ArchiveBuilder, TestAtl, TestEntry};

use crate::audio_bank_route_fixture::{self as fixture, Bank, Case, Object};

pub(crate) type TestResult = Result<(), Box<dyn Error>>;

pub(crate) fn with_archive(case: Case, check: impl FnOnce(&Archive) -> TestResult) -> TestResult {
    let banks = fixture::banks(case);
    assert_namespaces(&banks);
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("original-bank-route.p4k");
    let atl = TestAtl::new("OriginalBankRoute")
        .trigger(fixture::TRIGGER, &[fixture::EVENT])
        .to_xml();
    let atl_path = "Data\\Libs\\GameAudio\\original-bank-route.xml";
    let mut builder = ArchiveBuilder::new().entry(TestEntry::stored(atl_path, atl.as_bytes()));
    for bank in &banks {
        builder = builder.entry(TestEntry::stored(bank.path.as_str(), bank.bytes.as_slice()));
    }
    builder.write_to(&path)?;
    let archive = Archive::open(path)?;
    assert_bytes(&archive, atl_path, atl.as_bytes(), MAX_ATL_LEN)?;
    let expected_count = match case {
        Case::ThreeBanks => 3,
        Case::Missing => 1,
        _ => 2,
    };
    assert_eq!(
        banks.len(),
        expected_count,
        "all actual original source banks"
    );
    for bank in &banks {
        assert_bank(&archive, bank, case)?;
    }
    check(&archive)
}

fn assert_namespaces(banks: &[Bank]) {
    let ids: HashSet<_> = banks.iter().map(|bank| bank.id).collect();
    assert_eq!(
        ids.len(),
        banks.len(),
        "every original bank has a distinct header ID"
    );
    assert!(
        ![
            fixture::ROOT_ACTION,
            fixture::SECOND_ROOT_ACTION,
            fixture::FIRST_EVENT,
            fixture::FIRST_ACTION,
            fixture::FIRST_SOUND,
            fixture::SECOND_EVENT,
            fixture::SECOND_ACTION,
            fixture::SECOND_SOUND,
            fixture::MISSING_EVENT
        ]
        .contains(&fixture::root_event()),
        "the hashed root Event does not accidentally alias another fixture object"
    );
}

fn assert_bytes(archive: &Archive, path: &str, expected: &[u8], limit: u64) -> TestResult {
    let entry = archive
        .entries()
        .iter()
        .find(|entry| entry.path() == path)
        .ok_or("original source Entry")?;
    assert!(
        entry.uncompressed_size() < limit,
        "tiny fixture fits independent source-byte admission"
    );
    let mut bytes = Vec::new();
    archive.open_entry(entry)?.read_to_end(&mut bytes)?;
    assert_eq!(
        bytes, expected,
        "complete actual original bytes before mapping"
    );
    Ok(())
}

fn assert_bank(archive: &Archive, expected: &Bank, case: Case) -> TestResult {
    assert_bytes(archive, &expected.path, &expected.bytes, MAX_SOUNDBANK_LEN)?;
    let bank = Soundbank::parse(expected.bytes.as_slice())?;
    assert_eq!(bank.header().version, 154, "genuine expected source layout");
    assert_eq!(
        bank.header().id,
        expected.id,
        "actual distinct source bank identity"
    );
    assert!(
        bank.warnings().is_empty(),
        "no source warning may stand in for semantic RED"
    );
    let ids: Vec<_> = bank.objects().iter().map(|object| object.id).collect();
    let expected_ids: Vec<_> = expected.objects.iter().map(Object::id).collect();
    assert_eq!(
        ids, expected_ids,
        "every source HIRC occurrence in authored order"
    );
    assert_eq!(
        ids.iter().copied().collect::<HashSet<_>>().len(),
        ids.len(),
        "each individual bank has unique object IDs"
    );
    for object in &expected.objects {
        assert_object(&bank, object)?;
    }
    if matches!(case, Case::Missing) {
        assert!(
            bank.object(fixture::MISSING_EVENT).is_none(),
            "the target is genuinely absent from the only source bank"
        );
    }
    Ok(())
}

fn assert_object(bank: &Soundbank<&[u8]>, expected: &Object) -> TestResult {
    let object = bank
        .object(expected.id())
        .ok_or("actual original HIRC object")?;
    match expected {
        Object::Event { actions, .. } => {
            let HircValue::Event(actual) = &object.value else {
                return Err("original Event type".into());
            };
            assert_eq!(
                actual, actions,
                "exact ordered original Event action references"
            );
        }
        Object::Action { kind, target, .. } => {
            let HircValue::Action(actual) = &object.value else {
                return Err("original Action type".into());
            };
            assert_action(actual, *kind, *target);
        }
        Object::Sound { wem, .. } => {
            let HircValue::Sound(actual) = &object.value else {
                return Err("original Sound type".into());
            };
            assert_eq!(actual.source_id, *wem, "genuine original terminal WEM ID");
        }
    }
    Ok(())
}

fn assert_action(actual: &ActionTarget, kind: u16, target: u32) {
    assert_eq!(actual.action_type, kind, "actual original action class");
    assert_eq!(
        actual.target_id, target,
        "actual original selected target ID"
    );
    let expected_bank = (kind == fixture::PLAY).then_some((0, 0));
    assert_eq!(
        actual.bank.map(|bank| (bank.bank_id, bank.bank_type)),
        expected_bank,
        "actual local Play or posted Event selector"
    );
}

#[path = "audio_bank_range_tests.rs"]
mod range_tests;
