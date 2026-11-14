// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::rc::{Rc, Weak};

use super::super::walk::{self, Found};
use super::bank_probe::BankProbe;
use super::{Banks, Loaded, Spare};
use crate::audio::{
    AudioFileError, AudioIndex, BreakReason, IndexedSoundbank, LanguageChoice, MAX_EVENT_OBJECTS,
};

#[path = "bank_probe_source.rs"]
mod source;

use source::{Case, Fixture, TestResult};

fn enabled_spare<'index>() -> Spare<'index> {
    Spare {
        probe: Some(BankProbe::default()),
        ..Spare::default()
    }
}

fn run<'index>(
    fixture: &Fixture,
    index: &'index AudioIndex,
    spare: &mut Spare<'index>,
) -> TestResult<Found<'index>> {
    let home = Loaded::read(
        &fixture.archive,
        index,
        source::position(index, "Z-home.bnk")?,
    )?;
    let choice = LanguageChoice::any();
    let banks = Banks::new(&fixture.archive, index, &choice, home, spare);
    Ok(walk::event(banks, source::event(index)?, MAX_EVENT_OBJECTS))
}

fn load_error(result: Result<Loaded<'_>, AudioFileError>) -> TestResult<AudioFileError> {
    match result {
        Err(error) => Ok(error),
        Ok(_) => Err("expected the actual stored-header failure".into()),
    }
}

fn assert_variants(found: &Found<'_>, expected: &[(u32, &str)]) {
    let actual: Vec<_> = found
        .variants
        .iter()
        .map(|sound| (sound.wem_id(), sound.soundbank().file_name()))
        .collect();
    let expected: Vec<_> = expected
        .iter()
        .map(|(wem, bank)| (Some(*wem), *bank))
        .collect();
    assert_eq!(
        actual, expected,
        "actual worker terminals preserve branch order and physical banks"
    );
}

#[test]
fn actual_bank_probe_original_graphs_are_readable_before_worker_observation() -> TestResult {
    for case in [
        Case::Nested,
        Case::ActiveReturn,
        Case::Preference,
        Case::PreferenceAfterOther,
    ] {
        source::with_source(case, |fixture, index| {
            for position in 0..index.soundbanks().len() {
                let loaded = Loaded::read(&fixture.archive, index, u32::try_from(position)?)?;
                assert_eq!(
                    loaded.bank.header().id,
                    loaded.soundbank.id(),
                    "actual original worker parser agrees with admitted header"
                );
            }
            Ok(())
        })?;
    }
    Ok(())
}

#[test]
fn actual_bank_probe_counts_a_real_spare_hit_once() -> TestResult {
    source::with_source(Case::Preference, |fixture, index| {
        let mut spare = enabled_spare();
        let at = source::position(index, "M-first.bnk")?;
        let first = spare.get(&fixture.archive, index, at)?;
        let second = spare.get(&fixture.archive, index, at)?;
        assert!(
            Rc::ptr_eq(&first.bank, &second.bank),
            "the actual cached allocation is reused"
        );
        assert_eq!(
            spare.probe.as_ref().ok_or("enabled probe")?.reads,
            [at],
            "one actual returned loader call, no cache-hit tick"
        );
        Ok(())
    })
}

#[test]
fn actual_bank_probe_keeps_the_first_real_error_even_after_source_recovery() -> TestResult {
    source::with_source(Case::Preference, |fixture, index| {
        let expected = fixture.corrupt_first(index)?;
        let mut spare = enabled_spare();
        let at = source::position(index, "M-first.bnk")?;
        assert_eq!(
            load_error(spare.get(&fixture.archive, index, at))?,
            expected,
            "actual cached parse error"
        );
        fixture.restore_first(index)?;
        assert_eq!(
            load_error(spare.get(&fixture.archive, index, at))?,
            expected,
            "a cache hit cannot retry the now-readable original"
        );
        assert_eq!(
            spare.probe.as_ref().ok_or("enabled probe")?.reads,
            [at],
            "only the original failed worker load was called"
        );
        Ok(())
    })
}

#[test]
fn actual_bank_probe_reuses_the_original_active_a_allocation() -> TestResult {
    source::with_source(Case::ActiveReturn, |fixture, index| {
        let mut spare = enabled_spare();
        let a = source::position(index, "M-first.bnk")?;
        let b = source::position(index, "A-second.bnk")?;
        let found = run(fixture, index, &mut spare)?;
        let probe = spare.probe.as_ref().ok_or("enabled probe")?;
        assert_eq!(
            probe.reads,
            [a, b],
            "active A is not reread after B evicts the spare"
        );
        let first = probe.selection(a, 200).ok_or("original active A Event")?;
        let returned = probe
            .selection(a, 400)
            .ok_or("returned different A Event")?;
        assert!(
            Weak::ptr_eq(&first.allocation, &returned.allocation),
            "both actual selections share the first decoded owner"
        );
        assert!(
            found.breaks.is_empty(),
            "no synthetic cycle or count-fence break: {:?}",
            found.breaks
        );
        assert_variants(
            &found,
            &[
                (7003, "M-first.bnk"),
                (7002, "A-second.bnk"),
                (7001, "M-first.bnk"),
            ],
        );
        Ok(())
    })
}

#[test]
fn actual_bank_probe_does_not_retry_failed_a_after_successful_b() -> TestResult {
    source::with_source(Case::PreferenceAfterOther, |fixture, index| {
        let expected = fixture.corrupt_first(index)?;
        let mut spare = enabled_spare();
        let a = source::position(index, "M-first.bnk")?;
        let b = source::position(index, "A-second.bnk")?;
        let found = run(fixture, index, &mut spare)?;
        let probe = spare.probe.as_ref().ok_or("enabled probe")?;
        assert_eq!(
            probe.reads,
            [a, b],
            "same-walk first failure survives the real B load without an A retry"
        );
        assert_eq!(
            found.breaks.len(),
            2,
            "each original failed A reference remains represented"
        );
        for broken in &found.breaks {
            assert_eq!(
                broken.reason(),
                &BreakReason::Unreadable(expected.clone()),
                "first actual typed read error"
            );
            assert_eq!(
                broken.soundbank().map(IndexedSoundbank::file_name),
                Some("M-first.bnk"),
                "the actually selected failed bank"
            );
        }
        assert!(
            probe.selection(a, 400).is_none(),
            "a failed loader does not fabricate a selected target object"
        );
        assert_variants(&found, &[(7002, "A-second.bnk"), (7005, "Z-home.bnk")]);
        Ok(())
    })
}

#[test]
fn actual_bank_probe_releases_completed_banks_and_restores_original_preference() -> TestResult {
    source::with_source(Case::PreferenceAfterOther, |fixture, index| {
        let mut spare = enabled_spare();
        let a = source::position(index, "M-first.bnk")?;
        let b = source::position(index, "A-second.bnk")?;
        let home = source::position(index, "Z-home.bnk")?;
        let found = run(fixture, index, &mut spare)?;
        let probe = spare.probe.as_ref().ok_or("enabled probe")?;
        assert_eq!(
            probe.reads,
            [a, b, a],
            "completed successes are not kept in an all-bank resident map"
        );
        assert_completed_owners(probe, a, home, source::event(index)?.id())?;
        assert!(
            found.breaks.is_empty(),
            "complete legitimate branches: {:?}",
            found.breaks
        );
        assert_variants(
            &found,
            &[
                (7001, "M-first.bnk"),
                (7002, "A-second.bnk"),
                (7003, "M-first.bnk"),
                (7005, "Z-home.bnk"),
            ],
        );
        Ok(())
    })
}

fn assert_completed_owners(probe: &BankProbe, a: u32, home: u32, event: u32) -> TestResult {
    let first = probe.selection(a, 200).ok_or("first actual A selection")?;
    let later = probe
        .selection(a, 400)
        .ok_or("later preferred A selection")?;
    assert!(
        !Weak::ptr_eq(&first.allocation, &later.allocation),
        "completed and reloaded A have distinct actual allocations"
    );
    assert_eq!(
        first.allocation.strong_count(),
        0,
        "probe metadata does not retain the expired first A bank"
    );
    let start = probe.selection(home, event).ok_or("original home start")?;
    let restored = probe
        .selection(home, 500)
        .ok_or("actual home selection after both foreign banks")?;
    assert!(
        Weak::ptr_eq(&start.allocation, &restored.allocation),
        "restoring current does not reload home"
    );
    Ok(())
}
