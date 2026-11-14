// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::audio::{
    ArchiveAudio, BreakReason, BrokenLink, IndexedSoundbank, LanguageChoice,
};
use p4kx_extraction::Archive;

use crate::audio_bank_route_fixture::{self as fixture, Case};
use crate::audio_bank_route_source::{self as source, TestResult};

#[test]
fn original_bank_route_reaches_both_foreign_banks_with_distinct_object_ids() -> TestResult {
    source::with_archive(Case::ThreeBanks, |archive| {
        let audio = ArchiveAudio::new(archive);
        admitted(&audio, &["A-first.bnk", "B-second.bnk", "Z-home.bnk"]);
        let sounds = audio
            .resolve_trigger_in(fixture::TRIGGER, &LanguageChoice::any())
            .ok_or("original trigger")?;
        let actual: Vec<_> = sounds
            .variants()
            .iter()
            .map(|variant| (variant.wem_id(), variant.soundbank().file_name()))
            .collect();
        assert_eq!(
            actual,
            [(Some(7001), "A-first.bnk"), (Some(7002), "B-second.bnk")],
            "both real selected foreign targets retain source order"
        );
        assert!(
            sounds.breaks().is_empty(),
            "a bank-count policy cannot replace the second real target"
        );
        let paths: Vec<Vec<_>> = sounds
            .variants()
            .iter()
            .map(|variant| variant.path().iter().map(|step| step.id).collect())
            .collect();
        assert_eq!(
            paths,
            [
                vec![fixture::root_event(), 100, 200, 201, 210],
                vec![fixture::root_event(), 101, 300, 301, 310]
            ],
            "independent original full branches"
        );
        Ok(())
    })
}

#[test]
fn original_bank_route_equal_ancestor_id_in_another_bank_is_not_a_cycle() -> TestResult {
    source::with_archive(Case::EqualAncestor, |archive| {
        let audio = ArchiveAudio::new(archive);
        admitted(&audio, &["A-first.bnk", "Z-home.bnk"]);
        let sounds = audio
            .resolve_trigger_in(fixture::TRIGGER, &LanguageChoice::any())
            .ok_or("original trigger")?;
        let actual: Vec<_> = sounds
            .variants()
            .iter()
            .map(|variant| (variant.wem_id(), variant.soundbank().file_name()))
            .collect();
        assert_eq!(
            actual,
            [(Some(7001), "A-first.bnk")],
            "distinct original bank objects with equal IDs must not become the same object"
        );
        assert!(
            sounds.breaks().is_empty(),
            "no false ancestor cycle before actual bank selection"
        );
        let variant = sounds
            .variants()
            .first()
            .ok_or("genuine selected terminal")?;
        let ids: Vec<_> = variant.path().iter().map(|step| step.id).collect();
        assert_eq!(
            ids,
            [fixture::root_event(), 100, 200, 100, 210],
            "both different original Action100 occurrences remain in the path"
        );
        Ok(())
    })
}

#[test]
fn original_bank_route_return_to_the_same_actual_event_stays_a_cycle() -> TestResult {
    source::with_archive(Case::TrueCycle, |archive| {
        let audio = ArchiveAudio::new(archive);
        admitted(&audio, &["A-first.bnk", "Z-home.bnk"]);
        let sounds = audio
            .resolve_trigger_in(fixture::TRIGGER, &LanguageChoice::any())
            .ok_or("original trigger")?;
        assert!(
            sounds.variants().is_empty(),
            "a real cycle has no invented sound"
        );
        let reasons: Vec<_> = sounds.breaks().iter().map(BrokenLink::reason).collect();
        assert_eq!(
            reasons,
            [&BreakReason::Cycle {
                id: fixture::root_event()
            }],
            "the actual original home Event is revisited"
        );
        let refusal = sounds.breaks().first().ok_or("actual cycle break")?;
        let ids: Vec<_> = refusal.path().iter().map(|step| step.id).collect();
        assert_eq!(
            ids,
            [fixture::root_event(), 100, 200, 201],
            "all genuine pre-cycle source steps"
        );
        Ok(())
    })
}

#[test]
fn original_bank_route_ordinary_and_genuinely_missing_targets_keep_their_contract() -> TestResult {
    source::with_archive(Case::Ordinary, ordinary)?;
    source::with_archive(Case::Missing, |archive| {
        let audio = ArchiveAudio::new(archive);
        admitted(&audio, &["Z-home.bnk"]);
        let sounds = audio
            .resolve_trigger_in(fixture::TRIGGER, &LanguageChoice::any())
            .ok_or("original missing-target trigger")?;
        assert!(
            sounds.variants().is_empty(),
            "an actually absent target has no invented sound"
        );
        let reasons: Vec<_> = sounds.breaks().iter().map(BrokenLink::reason).collect();
        assert_eq!(
            reasons,
            [&BreakReason::Missing {
                id: fixture::MISSING_EVENT
            }],
            "exact actual absence reason"
        );
        Ok(())
    })
}

fn ordinary(archive: &Archive) -> TestResult {
    let audio = ArchiveAudio::new(archive);
    admitted(&audio, &["A-first.bnk", "Z-home.bnk"]);
    let sounds = audio
        .resolve_trigger_in(fixture::TRIGGER, &LanguageChoice::any())
        .ok_or("ordinary original trigger")?;
    let actual: Vec<_> = sounds
        .variants()
        .iter()
        .map(|variant| (variant.wem_id(), variant.soundbank().file_name()))
        .collect();
    assert_eq!(
        actual,
        [(Some(7001), "A-first.bnk")],
        "ordinary selected bank and terminal unchanged"
    );
    assert!(
        sounds.breaks().is_empty(),
        "ordinary genuine graph remains readable"
    );
    Ok(())
}

fn admitted(audio: &ArchiveAudio<'_>, names: &[&str]) {
    assert!(
        audio.index().issues().is_empty(),
        "no fixture/index admission failure masks graph behavior"
    );
    let actual: Vec<_> = audio
        .index()
        .soundbanks()
        .iter()
        .map(IndexedSoundbank::file_name)
        .collect();
    assert_eq!(
        actual, names,
        "all genuine bank inputs in deterministic index order"
    );
}
