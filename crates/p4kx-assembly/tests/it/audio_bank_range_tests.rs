// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::audio::{
    ArchiveAudio, IndexedSoundbank, LanguageChoice, MAX_ATL_LEN, TriggerSounds,
};
use p4kx_extraction::Archive;
use p4kx_testkit::{ArchiveBuilder, TestAtl, TestEntry};

use super::TestResult;
use super::fixture::ranges::Case;
use super::fixture::{self, ranges};

fn with_source(case: Case, check: impl FnOnce(&Archive) -> TestResult) -> TestResult {
    let banks = ranges::banks(case);
    super::assert_namespaces(&banks);
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("original-active-bank-route.p4k");
    let atl = TestAtl::new("OriginalActiveBankRoute")
        .trigger(fixture::TRIGGER, &[fixture::EVENT])
        .to_xml();
    let atl_path = "Data\\Libs\\GameAudio\\original-active-bank-route.xml";
    let mut builder = ArchiveBuilder::new().entry(TestEntry::stored(atl_path, atl.as_bytes()));
    for bank in &banks {
        builder = builder.entry(TestEntry::stored(bank.path.as_str(), bank.bytes.as_slice()));
    }
    builder.write_to(&path)?;
    let archive = Archive::open(path)?;
    super::assert_bytes(&archive, atl_path, atl.as_bytes(), MAX_ATL_LEN)?;
    assert_eq!(
        banks.len(),
        3,
        "three actual original banks precede resolution"
    );
    for bank in &banks {
        super::assert_bank(&archive, bank, fixture::Case::Ordinary)?;
    }
    let root_occurrences = banks
        .iter()
        .flat_map(|bank| &bank.objects)
        .filter(|object| object.id() == fixture::root_event())
        .count();
    assert_eq!(
        root_occurrences, 1,
        "only the home originally declares the start Event"
    );
    check(&archive)
}

fn resolve(case: Case, check: impl FnOnce(&TriggerSounds<'_>) -> TestResult) -> TestResult {
    with_source(case, |archive| {
        let audio = ArchiveAudio::new(archive);
        let index = audio.index();
        assert!(
            index.issues().is_empty(),
            "all original bank/index inputs are readable"
        );
        let names: Vec<_> = index
            .soundbanks()
            .iter()
            .map(IndexedSoundbank::file_name)
            .collect();
        assert_eq!(
            names,
            ["A-second.bnk", "M-first.bnk", "Z-home.bnk"],
            "actual index order"
        );
        let homes: Vec<_> = index
            .event_soundbanks(fixture::root_event())
            .map(IndexedSoundbank::file_name)
            .collect();
        assert_eq!(
            homes,
            ["Z-home.bnk"],
            "a single actual home starts this walk"
        );
        if matches!(case, Case::Preference | Case::PreferenceAfterOther) {
            let candidates: Vec<_> = index
                .event_soundbanks(400)
                .map(IndexedSoundbank::file_name)
                .collect();
            assert_eq!(
                candidates,
                ["A-second.bnk", "M-first.bnk"],
                "the prior foreign bank is not first in index order"
            );
        }
        let sounds = audio
            .resolve_trigger_in(fixture::TRIGGER, &LanguageChoice::any())
            .ok_or("the actual original trigger")?;
        check(&sounds)
    })
}

fn variants(sounds: &TriggerSounds<'_>, expected: &[(u32, &str, &[u32])]) {
    assert_eq!(
        sounds.variants().len(),
        expected.len(),
        "all original terminal occurrences"
    );
    assert!(
        sounds.breaks().is_empty(),
        "no bank-count or false-cycle replacement: {:?}",
        sounds.breaks()
    );
    for (actual, (wem, bank, path)) in sounds.variants().iter().zip(expected) {
        assert_eq!(actual.wem_id(), Some(*wem), "exact original terminal WEM");
        assert_eq!(
            actual.soundbank().file_name(),
            *bank,
            "actual selected physical bank"
        );
        let ids: Vec<_> = actual.path().iter().map(|step| step.id).collect();
        assert_eq!(ids, *path, "full original path in authored DFS order");
    }
}

#[test]
fn original_active_bank_route_follows_the_nested_third_bank_before_local_siblings() -> TestResult {
    resolve(Case::Nested, |sounds| {
        variants(
            sounds,
            &[
                (
                    7002,
                    "A-second.bnk",
                    &[fixture::root_event(), 100, 200, 201, 300, 301, 310],
                ),
                (
                    7001,
                    "M-first.bnk",
                    &[fixture::root_event(), 100, 200, 202, 210],
                ),
            ],
        );
        Ok(())
    })
}

#[test]
fn original_active_bank_route_returns_to_a_different_object_in_active_a() -> TestResult {
    resolve(Case::ActiveReturn, |sounds| {
        variants(
            sounds,
            &[
                (
                    7003,
                    "M-first.bnk",
                    &[
                        fixture::root_event(),
                        100,
                        200,
                        201,
                        300,
                        301,
                        400,
                        401,
                        410,
                    ],
                ),
                (
                    7002,
                    "A-second.bnk",
                    &[fixture::root_event(), 100, 200, 201, 300, 302, 310],
                ),
                (
                    7001,
                    "M-first.bnk",
                    &[fixture::root_event(), 100, 200, 202, 210],
                ),
            ],
        );
        Ok(())
    })
}

#[test]
fn original_active_bank_route_preserves_existing_foreign_and_home_preference() -> TestResult {
    resolve(Case::Preference, |sounds| {
        variants(
            sounds,
            &[
                (
                    7001,
                    "M-first.bnk",
                    &[fixture::root_event(), 100, 200, 201, 210],
                ),
                (
                    7003,
                    "M-first.bnk",
                    &[fixture::root_event(), 101, 400, 401, 410],
                ),
                (
                    7005,
                    "Z-home.bnk",
                    &[fixture::root_event(), 102, 500, 501, 510],
                ),
            ],
        );
        Ok(())
    })
}

#[test]
fn original_active_bank_route_keeps_the_first_foreign_hint_after_another_bank() -> TestResult {
    resolve(Case::PreferenceAfterOther, |sounds| {
        variants(
            sounds,
            &[
                (
                    7001,
                    "M-first.bnk",
                    &[fixture::root_event(), 100, 200, 201, 210],
                ),
                (
                    7002,
                    "A-second.bnk",
                    &[fixture::root_event(), 101, 300, 301, 310],
                ),
                (
                    7003,
                    "M-first.bnk",
                    &[fixture::root_event(), 102, 400, 401, 410],
                ),
                (
                    7005,
                    "Z-home.bnk",
                    &[fixture::root_event(), 103, 500, 501, 510],
                ),
            ],
        );
        Ok(())
    })
}
