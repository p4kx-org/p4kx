// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Resolution bounds the depth, the fan-out and the size of an untrusted soundbank hierarchy.

use std::error::Error;
use std::fs;
use std::path::Path;

use p4kx_assembly::audio::{
    ArchiveAudio, BreakReason, ExportLoopOrigins, ExportOptions, ExportReport, ExportState,
    ExportWatch, ExportedSound, MAX_CONTAINER_CHILDREN, MAX_EVENT_OBJECTS, MAX_PATH_DEPTH,
    SoundFormat, TriggerSounds, write_txtp, write_txtp_with_loop_origins,
};
use p4kx_extraction::{Archive, CancelToken, ConflictPolicy, ConversionError};
use p4kx_testkit::{
    ArchiveBuilder, TestAtl, TestBank, TestContainerKind, TestEntry, TestHircObject, TestNodeBase,
    wwise_short_id,
};

use crate::audio_txtp_support::pcm;

type TestResult = Result<(), Box<dyn Error>>;

const PLAY: u16 = 0x0403;

fn resolve(bank: TestBank, check: impl FnOnce(&TriggerSounds<'_>) -> TestResult) -> TestResult {
    let bytes = bank
        .object(TestHircObject::event(wwise_short_id("Play_It"), &[1]))
        .object(TestHircObject::action(1, PLAY, 10))
        .bytes();
    resolve_bytes(bytes, check)
}

fn resolve_bytes(
    bytes: Vec<u8>,
    check: impl FnOnce(&TriggerSounds<'_>) -> TestResult,
) -> TestResult {
    with_archive(bytes, |archive, _| {
        let audio = ArchiveAudio::new(archive);
        let sounds = audio.resolve_trigger("Play_It").ok_or("Play_It resolves")?;
        check(&sounds)
    })
}

fn with_archive(bytes: Vec<u8>, check: impl FnOnce(&Archive, &Path) -> TestResult) -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(
            "Data\\Libs\\GameAudio\\SC.xml",
            TestAtl::new("SC").trigger("Play_It", &["Play_It"]).to_xml(),
        ))
        .entry(TestEntry::stored("Data\\Sounds\\wwise\\Deep.bnk", bytes))
        .write_to(&path)?;
    let archive = Archive::open_with_key(&path, Some(p4kx_testkit::test_key()))?;
    check(&archive, dir.path())
}

fn random(id: u32, children: &[u32]) -> TestHircObject {
    TestHircObject::container(TestContainerKind::Random, id, children, TestNodeBase::Full)
}

#[test]
fn a_path_deeper_than_the_limit_is_cut_at_the_limit() -> TestResult {
    let bank = (10..60).fold(TestBank::new(1), |bank, id| {
        bank.object(random(id, &[id + 1]))
    });
    resolve(bank, |sounds| {
        assert!(sounds.variants().is_empty(), "no variants");
        let [deep] = sounds.breaks() else {
            return Err(format!("one break: {:?}", sounds.breaks()).into());
        };
        assert_eq!(
            deep.reason(),
            &BreakReason::TooDeep {
                limit: MAX_PATH_DEPTH
            },
            "reason"
        );
        assert_eq!(deep.path().len(), MAX_PATH_DEPTH, "path at the limit");
        Ok(())
    })
}

fn with_truncated_resolution(
    check: impl FnOnce(&ArchiveAudio<'_>, &TriggerSounds<'_>, &Path) -> TestResult,
) -> TestResult {
    let terminal = 100 + u32::try_from(MAX_PATH_DEPTH)?;
    let bank = TestBank::new(1)
        .wem(7, pcm().bytes())
        .object(TestHircObject::event(wwise_short_id("Play_It"), &[1]))
        .object(TestHircObject::action(1, PLAY, 10))
        .object(random(10, &[20, 100]))
        .object(TestHircObject::sound(20, 7, 0, 0))
        .object(TestHircObject::sound(terminal, 7, 0, 0));
    let bank = (100..terminal).fold(bank, |bank, id| bank.object(random(id, &[id + 1])));
    with_archive(bank.bytes(), |archive, root| {
        let audio = ArchiveAudio::new(archive);
        let sounds = audio.resolve_trigger("Play_It").ok_or("Play_It resolves")?;
        let [good] = sounds.variants() else {
            return Err("exactly one route resolves before the limit".into());
        };
        assert!(good.location().is_some(), "the good WEM can be written");
        assert!(sounds.truncated_by_limits(), "the other route is truncated");
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
        ("Play_It", "Play_It"),
        "identify the trigger and event that stopped publication"
    );
    assert_eq!(
        source.downcast_ref::<BreakReason>(),
        Some(&BreakReason::TooDeep {
            limit: MAX_PATH_DEPTH,
        }),
        "retain the original limit as the error source"
    );
    Ok(())
}

#[test]
fn a_truncated_resolution_stops_audio_before_planning_or_creating_outputs() -> TestResult {
    with_truncated_resolution(|audio, sounds, root| {
        let options = ExportOptions {
            format: SoundFormat::Raw,
            policy: ConflictPolicy::Overwrite,
        };
        let missing = root.join("not-created");
        let triggers = [sounds];
        let watch = ExportWatch::default();
        let error = audio
            .export_sounds_watched_with_loop_origins(&triggers, &missing, options, &watch)
            .expect_err("the incomplete walk stops planning");
        assert_export_cut(&error)?;
        assert_eq!((watch.total(), watch.done()), (0, 0), "no work planned");
        assert!(!missing.exists(), "no destination created");

        let existing = root.join("Play_It.wem");
        fs::write(&existing, b"previous WEM")?;
        let error = audio
            .export_sounds(&triggers, root, options)
            .expect_err("the incomplete walk stops replacement");
        assert_export_cut(&error)?;
        assert_eq!(fs::read(existing)?, b"previous WEM");
        Ok(())
    })
}

#[test]
fn a_truncated_resolution_stops_txtp_even_with_a_whole_audio_file() -> TestResult {
    with_truncated_resolution(|audio, sounds, root| {
        let wem = root.join("Play_It.wem");
        let previous = pcm().bytes();
        fs::write(&wem, &previous)?;
        let report = ExportReport {
            sounds: vec![ExportedSound {
                trigger: "Play_It".to_owned(),
                wem_id: 7,
                path: wem,
                format: SoundFormat::Raw,
                size: u64::try_from(previous.len())?,
                state: ExportState::Kept,
                unconverted: None,
                loop_points: None,
                prefetch_only: false,
            }],
            ..ExportReport::default()
        };
        let txtp = root.join("Play_It.txtp");
        fs::write(&txtp, b"previous TXTP")?;
        let triggers = [sounds];
        let policy = ConflictPolicy::Overwrite;
        let direct = write_txtp(&triggers, &report, root, policy);
        assert_eq!(direct.files, [], "the legacy writer publishes no files");
        let [skip] = direct.skipped.as_slice() else {
            return Err("the blocked event is reported once".into());
        };
        assert_eq!(
            (skip.trigger.as_str(), skip.event.as_str()),
            ("Play_It", "Play_It")
        );
        assert!(skip.reason.contains("path is deeper"), "{}", skip.reason);

        let error = write_txtp_with_loop_origins(
            audio.archive(),
            &triggers,
            &report,
            root,
            policy,
            &CancelToken::new(),
            &ExportLoopOrigins::default(),
        )
        .expect_err("the incomplete walk stops text and receipt publication");
        let cause = error
            .get_ref()
            .and_then(|source| source.downcast_ref::<ConversionError>())
            .ok_or("typed traversal error")?;
        assert_export_cut(cause)?;
        assert_eq!(fs::read(txtp)?, b"previous TXTP");
        Ok(())
    })
}

#[test]
fn children_beyond_the_fan_out_limit_are_left_out_and_reported() -> TestResult {
    let children = vec![20; MAX_CONTAINER_CHILDREN + 5];
    let bank = TestBank::new(1)
        .object(random(10, &children))
        .object(TestHircObject::sound(20, 7, 2, 0));
    resolve(bank, |sounds| {
        assert_eq!(
            sounds.variants().len(),
            MAX_CONTAINER_CHILDREN,
            "the first children"
        );
        let [wide] = sounds.breaks() else {
            return Err(format!("one break: {:?}", sounds.breaks()).into());
        };
        assert_eq!(
            (wide.reason(), wide.object().map(|step| step.id)),
            (
                &BreakReason::TooManyChildren {
                    count: MAX_CONTAINER_CHILDREN + 5,
                    limit: MAX_CONTAINER_CHILDREN,
                },
                Some(10)
            ),
            "reported at the container"
        );
        Ok(())
    })
}

#[test]
fn a_hierarchy_leading_to_more_objects_than_the_budget_is_cut_once() -> TestResult {
    let bank = TestBank::new(1)
        .object(random(10, &[11; 40]))
        .object(random(11, &[12; 40]))
        .object(random(12, &[20; 40]))
        .object(TestHircObject::sound(20, 7, 2, 0));
    resolve(bank, |sounds| {
        let [budget] = sounds.breaks() else {
            return Err(format!("one break: {:?}", sounds.breaks()).into());
        };
        assert_eq!(
            budget.reason(),
            &BreakReason::TooManyObjects {
                limit: MAX_EVENT_OBJECTS
            },
            "reason"
        );
        assert!(
            sounds.variants().len() < MAX_EVENT_OBJECTS,
            "{} variants",
            sounds.variants().len()
        );
        Ok(())
    })
}

#[test]
fn back_references_are_charged_to_the_budget() -> TestResult {
    let mut bank = TestBank::new(1);
    for id in 10..40 {
        let mut children = vec![10; 900];
        children.extend([id + 1, id + 1]);
        bank = bank.object(random(id, &children));
    }
    resolve(bank.object(random(40, &[10; 900])), |sounds| {
        let budget = sounds
            .breaks()
            .iter()
            .filter(|link| matches!(link.reason(), BreakReason::TooManyObjects { .. }))
            .count();
        assert_eq!(budget, 1, "the budget cut is reported once");
        assert!(
            sounds.breaks().len() <= MAX_EVENT_OBJECTS + 1,
            "{} breaks",
            sounds.breaks().len()
        );
        Ok(())
    })
}

#[test]
fn an_event_listing_itself_many_times_is_cut_at_the_fan_out_limit() -> TestResult {
    let event = wwise_short_id("Play_It");
    let bytes = TestBank::new(1)
        .object(TestHircObject::event(event, &vec![event; 5000]))
        .bytes();
    resolve_bytes(bytes, |sounds| {
        assert!(sounds.variants().is_empty(), "no variants");
        assert_eq!(
            sounds.breaks().len(),
            MAX_CONTAINER_CHILDREN + 1,
            "one break per followed action and one for the rest"
        );
        assert!(
            sounds.breaks().iter().any(|link| matches!(
                link.reason(),
                BreakReason::TooManyChildren { count: 5000, .. }
            )),
            "the list is reported as too long"
        );
        Ok(())
    })
}

#[test]
fn authored_effect_modulator_and_device_objects_keep_unsupported_playback_disposition() -> TestResult
{
    for (type_code, complete_prefix_length) in
        [(16, 15), (17, 15), (19, 4), (20, 4), (21, 16), (22, 4)]
    {
        for body_length in [complete_prefix_length, 1] {
            let body = vec![0; body_length];
            let bank = TestBank::new(1)
                .version(154)
                .object(TestHircObject::raw(type_code, 10, body));
            resolve(bank, |sounds| {
                assert!(
                    sounds.variants().is_empty(),
                    "metadata targets do not yield audio variants"
                );
                let [unresolved] = sounds.breaks() else {
                    return Err("the authored metadata remains a single unresolved target".into());
                };
                assert_eq!(
                    unresolved.reason(),
                    &BreakReason::Unsupported,
                    "source metadata has the existing playback disposition"
                );
                Ok(())
            })?;
        }
    }
    Ok(())
}
