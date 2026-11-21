// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Explicit resolver budgets count authored visits and stop incomplete export publication.

use std::error::Error;
use std::fs;
use std::path::Path;

use p4kx_assembly::audio::{
    ArchiveAudio, BreakReason, BrokenLink, ExportLoopOrigins, ExportOptions, ExportReport,
    ExportState, ExportWatch, IndexedSoundbank, LanguageChoice, ResolveOptions, SoundFormat,
    SoundVariant, TriggerSounds, write_txtp, write_txtp_with_loop_origins,
};
use p4kx_extraction::{CancelToken, ConflictPolicy, ConversionError};
use p4kx_testkit::{TestBank, TestHircObject, wwise_short_id};

use super::audio_resolve_limits::{TRIGGER, random, with_archive};
use super::audio_txtp_support::{PLAY, TestResult, pcm};

const EXPORT_VISITS: usize = 4;

fn with_budget<'a>(
    audio: &'a ArchiveAudio<'_>,
    limit: usize,
) -> Result<TriggerSounds<'a>, Box<dyn Error>> {
    audio
        .resolve_trigger_with_options(
            TRIGGER,
            &LanguageChoice::any(),
            ResolveOptions::default().with_max_visits(limit),
        )?
        .ok_or_else(|| "trigger resolves".into())
}

fn with_export_cut(
    check: impl FnOnce(&ArchiveAudio<'_>, &TriggerSounds<'_>, &Path) -> TestResult,
) -> TestResult {
    let wem = pcm().bytes();
    let size = u32::try_from(wem.len())?;
    let bank = TestBank::new(1)
        .wem(7, wem)
        .object(TestHircObject::event(wwise_short_id(TRIGGER), &[1]))
        .object(TestHircObject::action(1, PLAY, 10))
        .object(random(10, &[20, 21]))
        .object(TestHircObject::sound(20, 7, 0, size))
        .object(TestHircObject::sound(21, 8, 2, 0))
        .bytes();
    with_archive(&[TRIGGER], &[("Audio", &bank)], |archive, root| {
        let audio = ArchiveAudio::new(archive);
        let sounds = with_budget(&audio, EXPORT_VISITS)?;
        let [variant] = sounds.variants() else {
            return Err("the visit budget reaches exactly one route".into());
        };
        assert!(
            variant.location().is_some(),
            "the reached WEM can be exported"
        );
        assert!(
            sounds.truncated_by_limits(),
            "a second route was left unvisited"
        );
        check(&audio, &sounds, root)
    })
}

fn assert_export_cut(error: &ConversionError) -> TestResult {
    let ConversionError::IncompleteAudioResolution {
        trigger,
        event,
        source,
    } = error
    else {
        return Err(format!("expected incomplete resolution, got {error:?}").into());
    };
    assert_eq!(
        (trigger.as_str(), event.as_str()),
        (TRIGGER, TRIGGER),
        "identify the trigger and event that stopped publication"
    );
    assert_eq!(
        source.downcast_ref::<BreakReason>(),
        Some(&BreakReason::TooManyObjects {
            limit: EXPORT_VISITS
        }),
        "retain the caller budget as the typed error source"
    );
    Ok(())
}

#[test]
fn a_caller_budget_stops_audio_before_planning_or_creating_outputs() -> TestResult {
    with_export_cut(|audio, sounds, root| {
        let options = ExportOptions {
            format: SoundFormat::Raw,
            policy: ConflictPolicy::Overwrite,
        };
        let missing = root.join("not-created");
        let triggers = [sounds];
        let watch = ExportWatch::default();
        let error = audio
            .export_sounds_watched_with_loop_origins(&triggers, &missing, options, &watch)
            .expect_err("the exhausted budget stops planning");
        assert_export_cut(&error)?;
        assert_eq!((watch.total(), watch.done()), (0, 0), "no work planned");
        assert!(!missing.exists(), "no destination created");

        let existing = root.join("Play_It.wem");
        fs::write(&existing, b"previous WEM")?;
        let error = audio
            .export_sounds(&triggers, root, options)
            .expect_err("the exhausted budget stops replacement");
        assert_export_cut(&error)?;
        assert_eq!(fs::read(existing)?, b"previous WEM", "the old WEM remains");
        Ok(())
    })
}

#[test]
fn a_caller_budget_stops_txtp_even_with_a_whole_audio_file() -> TestResult {
    with_export_cut(|audio, sounds, root| {
        let report = kept_sound(audio, root)?;
        let txtp = root.join("Play_It.txtp");
        let receipt = root.join("Play_It.txtp.p4kx-origin.jsonl");
        fs::write(&txtp, b"previous TXTP")?;
        fs::write(&receipt, b"previous receipt")?;
        let triggers = [sounds];
        let policy = ConflictPolicy::Overwrite;
        let direct = write_txtp(&triggers, &report, root, policy);
        assert_eq!(direct.files, [], "the legacy writer publishes no files");
        let [skip] = direct.skipped.as_slice() else {
            return Err("the blocked event is reported once".into());
        };
        assert_eq!(
            (skip.trigger.as_str(), skip.event.as_str()),
            (TRIGGER, TRIGGER)
        );
        assert!(
            skip.reason.contains(&EXPORT_VISITS.to_string()),
            "report the supplied limit: {}",
            skip.reason
        );

        let error = write_txtp_with_loop_origins(
            audio.archive(),
            &triggers,
            &report,
            root,
            policy,
            &CancelToken::new(),
            &ExportLoopOrigins::default(),
        )
        .expect_err("the exhausted budget stops text and receipt publication");
        let cause = error
            .get_ref()
            .and_then(|source| source.downcast_ref::<ConversionError>())
            .ok_or("typed traversal error")?;
        assert_export_cut(cause)?;
        assert_eq!(fs::read(txtp)?, b"previous TXTP", "the old TXTP remains");
        assert_eq!(
            fs::read(receipt)?,
            b"previous receipt",
            "the old receipt remains"
        );
        Ok(())
    })
}

fn kept_sound(audio: &ArchiveAudio<'_>, root: &Path) -> Result<ExportReport, Box<dyn Error>> {
    let sounds = audio
        .resolve_trigger(TRIGGER)
        .ok_or("the trigger resolves")?;
    let options = ExportOptions {
        format: SoundFormat::Raw,
        policy: ConflictPolicy::Skip,
    };
    let first = audio.export_sounds(&[&sounds], root, options)?;
    let [sound] = first.sounds.as_slice() else {
        return Err("one whole WEM is available".into());
    };
    assert_eq!(
        sound.state,
        ExportState::Written,
        "the fixture file is written"
    );
    let report = audio.export_sounds(&[&sounds], root, options)?;
    assert_eq!(report.kept_count(), 1, "the complete WEM was kept");
    Ok(report)
}

#[test]
fn the_visit_budget_is_shared_in_authored_event_and_holder_order() -> TestResult {
    const FIRST: &str = "E_First";
    const LAST: &str = "E_Last";
    let a = bank_for_event("A", LAST, 71);
    let b = bank_for_event("B", FIRST, 72);
    let c = bank_for_event("C", FIRST, 73);
    with_archive(
        &[FIRST, LAST],
        &[("A", &a), ("B", &b), ("C", &c)],
        |archive, _| {
            let audio = ArchiveAudio::new(archive);
            let intended = [Some(72), Some(73), Some(71)];
            let exact = with_budget(&audio, 9)?;
            assert_eq!(
                exact
                    .variants()
                    .iter()
                    .map(SoundVariant::wem_id)
                    .collect::<Vec<_>>(),
                intended,
                "all three visits per Event bank fit the exact budget"
            );
            assert!(
                !exact.truncated_by_limits(),
                "the exact budget is not a cutoff"
            );
            assert!(exact.breaks().is_empty(), "the three routes are valid");
            let exact_zero = with_budget(&audio, 0)?;
            assert_budget_cut(&exact_zero, 0, FIRST, "B.bnk", &[])?;
            let next_holder = with_budget(&audio, 4)?;
            assert_budget_cut(&next_holder, 4, FIRST, "C.bnk", &[Some(72)])?;
            let next_event = with_budget(&audio, 8)?;
            assert_budget_cut(&next_event, 8, LAST, "A.bnk", &[Some(72), Some(73)])?;
            Ok(())
        },
    )
}

fn assert_budget_cut(
    sounds: &TriggerSounds<'_>,
    limit: usize,
    event: &str,
    bank: &str,
    expected: &[Option<u32>],
) -> TestResult {
    let [cut] = sounds.breaks() else {
        return Err(format!("one cutoff: {:?}", sounds.breaks()).into());
    };
    assert_eq!(
        cut.reason(),
        &BreakReason::TooManyObjects { limit },
        "budget"
    );
    assert_eq!(
        cut.event().name(),
        event,
        "the first unfinished authored event"
    );
    assert_eq!(
        cut.soundbank().map(IndexedSoundbank::file_name),
        Some(bank),
        "the first unfinished physical bank"
    );
    assert!(
        sounds
            .variants()
            .iter()
            .map(SoundVariant::wem_id)
            .eq(expected.iter().copied()),
        "only variants reached before exhausting the shared budget"
    );
    assert!(sounds.truncated_by_limits(), "the cutoff is visible");
    Ok(())
}

fn bank_for_event(name: &str, event: &str, wem: u32) -> Vec<u8> {
    TestBank::new(wwise_short_id(name))
        .object(TestHircObject::event(wwise_short_id(event), &[1]))
        .object(TestHircObject::action(1, PLAY, 10))
        .object(TestHircObject::sound(10, wem, 2, 0))
        .bytes()
}

#[test]
fn even_back_references_consume_visits() -> TestResult {
    let event = wwise_short_id(TRIGGER);
    let bank = TestBank::new(1)
        .object(TestHircObject::event(event, &[event; 5]))
        .bytes();
    with_archive(&[TRIGGER], &[("Cycle", &bank)], |archive, _| {
        let audio = ArchiveAudio::new(archive);
        let sounds = with_budget(&audio, 4)?;
        let cycle = BreakReason::Cycle { id: event };
        let cut = BreakReason::TooManyObjects { limit: 4 };
        assert_eq!(
            sounds
                .breaks()
                .iter()
                .map(BrokenLink::reason)
                .collect::<Vec<_>>(),
            [&cycle, &cycle, &cycle, &cut],
            "the Event and first three back references use all four visits"
        );
        Ok(())
    })
}

#[test]
fn each_music_track_source_consumes_a_visit() -> TestResult {
    let bank = TestBank::new(1)
        .object(TestHircObject::event(wwise_short_id(TRIGGER), &[1]))
        .object(TestHircObject::action(1, PLAY, 10))
        .object(TestHircObject::music_track(10, &[7, 8]))
        .bytes();
    with_archive(&[TRIGGER], &[("Track", &bank)], |archive, _| {
        let audio = ArchiveAudio::new(archive);
        let partial = with_budget(&audio, 4)?;
        assert_budget_cut(&partial, 4, TRIGGER, "Track.bnk", &[Some(7)])?;
        let exact = with_budget(&audio, 5)?;
        assert!(
            exact
                .variants()
                .iter()
                .map(SoundVariant::wem_id)
                .eq([Some(7), Some(8)]),
            "with a fifth visit the second source is also reached"
        );
        assert!(
            exact.breaks().is_empty(),
            "reaching the exact budget is complete"
        );
        Ok(())
    })
}

#[test]
fn cancelling_before_resolution_returns_cancellation() -> TestResult {
    let bank = bank_for_event("Audio", TRIGGER, 71);
    with_archive(&[TRIGGER], &[("Audio", &bank)], |archive, _| {
        let cancel = CancelToken::new();
        cancel.cancel();
        let audio = ArchiveAudio::new(archive);
        let resolved = audio.resolve_trigger_with_options(
            TRIGGER,
            &LanguageChoice::any(),
            ResolveOptions::default().with_cancel(&cancel),
        );
        assert!(
            matches!(resolved, Err(ConversionError::Cancelled)),
            "no partial result"
        );
        Ok(())
    })
}
