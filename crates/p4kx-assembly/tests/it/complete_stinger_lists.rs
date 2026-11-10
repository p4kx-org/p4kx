// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::complete_stinger_fixture::{self as fixture, Case};
use crate::complete_stinger_source::{self as source, TestResult};
use p4kx_assembly::audio::{ArchiveAudio, BreakReason, BrokenLink, IndexedSoundbank};

#[test]
fn stingers_complete_bank_boundary_reaches_the_last_original_stinger() -> TestResult {
    resolves(Case::SingleBoundary, &["Z-tail.bnk"])
}

#[test]
fn stingers_complete_global_boundary_reaches_the_last_original_stinger() -> TestResult {
    resolves(Case::GlobalBoundary, &["Z-tail.bnk"])
}

#[test]
fn stingers_complete_deduplicates_only_equal_tuples_within_each_bank() -> TestResult {
    resolves(Case::Duplicates, &["A-prefix.bnk", "Z-tail.bnk"])
}

fn resolves(case: Case, names: &[&str]) -> TestResult {
    source::with_archive(case, |archive| {
        let audio = ArchiveAudio::new(archive);
        let sounds = audio
            .resolve_trigger(fixture::TRIGGER)
            .ok_or("actual indexed ATL trigger")?;
        assert!(
            audio.index().issues().is_empty(),
            "valid original sources are admitted"
        );
        let indexed: Vec<_> = audio
            .index()
            .soundbanks()
            .iter()
            .map(IndexedSoundbank::file_name)
            .collect();
        let expected_banks = match case {
            Case::GlobalBoundary | Case::Duplicates => vec!["A-prefix.bnk", "Z-tail.bnk"],
            Case::SingleBoundary | Case::Missing | Case::Small => vec!["Z-tail.bnk"],
        };
        assert_eq!(
            indexed, expected_banks,
            "all real banks retain their deterministic source order"
        );
        let actual: Vec<_> = sounds
            .variants()
            .iter()
            .map(|variant| (variant.wem_id(), variant.soundbank().file_name()))
            .collect();
        let expected: Vec<_> = names
            .iter()
            .map(|name| (Some(fixture::WEM), *name))
            .collect();
        assert_eq!(
            actual, expected,
            "the genuine selected late source reaches its real target"
        );
        assert!(
            sounds.breaks().is_empty(),
            "no prefix cutoff may become a missing target"
        );
        for variant in sounds.variants() {
            let kinds: Vec<_> = variant.path().iter().map(|step| step.kind.name()).collect();
            assert_eq!(
                kinds,
                ["event", "action", "music_segment", "music_track"],
                "actual resolution path"
            );
        }
        Ok(())
    })
}

#[test]
fn stingers_complete_small_and_genuinely_missing_source_controls() -> TestResult {
    resolves(Case::Small, &["Z-tail.bnk"])?;
    source::with_archive(Case::Missing, |archive| {
        let audio = ArchiveAudio::new(archive);
        let sounds = audio
            .resolve_trigger(fixture::TRIGGER)
            .ok_or("the actual posted trigger")?;
        assert!(
            sounds.variants().is_empty(),
            "an actually missing source has no invented target"
        );
        let reasons: Vec<_> = sounds.breaks().iter().map(BrokenLink::reason).collect();
        assert_eq!(
            reasons,
            [&BreakReason::Missing { id: fixture::LATE }],
            "original missing reason retained"
        );
        Ok(())
    })
}
