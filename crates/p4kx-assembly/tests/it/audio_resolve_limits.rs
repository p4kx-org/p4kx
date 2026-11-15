// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Resolution of long and wide authored soundbank hierarchies.

use std::error::Error;
use std::path::Path;

use p4kx_assembly::audio::{
    ArchiveAudio, BreakReason, LanguageChoice, ResolveOptions, SoundVariant, TriggerSounds,
};
use p4kx_extraction::Archive;
use p4kx_testkit::{
    ArchiveBuilder, TestAtl, TestBank, TestContainerKind, TestEntry, TestHircObject, TestNodeBase,
    wwise_short_id,
};

use super::audio_txtp_support::PLAY;

type TestResult = Result<(), Box<dyn Error>>;

pub(crate) const TRIGGER: &str = "Play_It";
const STOP: u16 = 0x0102;
const PLAY_EVENT: u16 = 0x2103;

fn resolve(bank: TestBank, check: impl FnOnce(&TriggerSounds<'_>) -> TestResult) -> TestResult {
    let bytes = bank
        .object(TestHircObject::event(wwise_short_id(TRIGGER), &[1]))
        .object(TestHircObject::action(1, PLAY, 10))
        .bytes();
    resolve_bytes(&bytes, check)
}

fn resolve_bytes(bytes: &[u8], check: impl FnOnce(&TriggerSounds<'_>) -> TestResult) -> TestResult {
    with_archive(&[TRIGGER], &[("Deep", bytes)], |archive, _| {
        let audio = ArchiveAudio::new(archive);
        let sounds = audio.resolve_trigger(TRIGGER).ok_or("trigger resolves")?;
        check(&sounds)
    })
}

pub(crate) fn with_archive(
    events: &[&str],
    banks: &[(&str, &[u8])],
    check: impl FnOnce(&Archive, &Path) -> TestResult,
) -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("Data.p4k");
    let mut builder = ArchiveBuilder::new().entry(TestEntry::stored(
        "Data\\Libs\\GameAudio\\SC.xml",
        TestAtl::new("SC").trigger(TRIGGER, events).to_xml(),
    ));
    for &(name, bytes) in banks {
        builder = builder.entry(TestEntry::stored(
            format!("Data\\Sounds\\wwise\\{name}.bnk"),
            bytes,
        ));
    }
    builder.write_to(&path)?;
    let archive = Archive::open_with_key(&path, Some(p4kx_testkit::test_key()))?;
    check(&archive, dir.path())
}

pub(crate) fn random(id: u32, children: &[u32]) -> TestHircObject {
    TestHircObject::container(TestContainerKind::Random, id, children, TestNodeBase::Full)
}

#[test]
fn a_deep_path_reaches_its_sound_with_every_original_object_site() -> TestResult {
    const CONTAINERS: u32 = 2_048;
    const FIRST: u32 = 10;
    const TERMINAL: u32 = FIRST + CONTAINERS;
    let mut bank = TestBank::new(1)
        .object(TestHircObject::event(wwise_short_id(TRIGGER), &[1]))
        .object(TestHircObject::action(1, PLAY, FIRST))
        .object(TestHircObject::sound(TERMINAL, 7, 2, 0));
    for id in FIRST..TERMINAL {
        bank = bank.object(random(id, &[id + 1]));
    }
    let bytes = bank.bytes();
    with_archive(&[TRIGGER], &[("Deep", &bytes)], |archive, _| {
        let audio = ArchiveAudio::new(archive);
        let sounds = audio
            .resolve_trigger_with_options(
                TRIGGER,
                &LanguageChoice::any(),
                ResolveOptions::default().with_path_sources(),
            )?
            .ok_or("trigger resolves")?;
        let [variant] = sounds.variants() else {
            return Err(format!("one terminal variant: {:?}", sounds.breaks()).into());
        };
        assert_eq!(variant.wem_id(), Some(7), "the terminal sound is reached");
        assert_eq!(
            variant.path().len(),
            usize::try_from(CONTAINERS)? + 3,
            "every authored container, Event, Action and Sound is retained"
        );
        let original_ids = [wwise_short_id(TRIGGER), 1]
            .into_iter()
            .chain(FIRST..=TERMINAL);
        assert!(
            variant.path().iter().map(|step| step.id).eq(original_ids),
            "the path is in authored order"
        );
        assert_original_path_sites(variant, &bytes);
        assert!(
            !sounds.truncated_by_limits(),
            "the default walk reaches the end"
        );
        assert!(
            sounds.breaks().is_empty(),
            "no broken links: {:?}",
            sounds.breaks()
        );
        Ok(())
    })
}

fn assert_original_path_sites(variant: &SoundVariant<'_>, bytes: &[u8]) {
    assert_eq!(
        variant.path_sources().len(),
        variant.path().len(),
        "every step has its original source site"
    );
    for (step, source) in variant.path().iter().zip(variant.path_sources()) {
        assert_eq!(
            source.entry_position(),
            variant.soundbank().entry_position(),
            "every source belongs to this soundbank Entry"
        );
        assert_eq!(
            bytes.get(source.id_bytes()),
            Some(step.id.to_le_bytes().as_slice()),
            "the original bank stores this object ID at its reported site"
        );
    }
}

#[test]
fn a_wide_container_preserves_all_repeated_child_occurrences() -> TestResult {
    let children: Vec<_> = [20, 21].into_iter().cycle().take(1_029).collect();
    let bank = TestBank::new(1)
        .object(random(10, &children))
        .object(TestHircObject::sound(20, 7, 2, 0))
        .object(TestHircObject::sound(21, 8, 2, 0));
    resolve(bank, |sounds| {
        assert!(
            sounds
                .variants()
                .iter()
                .map(SoundVariant::sound_id)
                .eq(children.iter().copied()),
            "one variant per child occurrence, in order"
        );
        assert!(
            sounds.breaks().is_empty(),
            "no broken links: {:?}",
            sounds.breaks()
        );
        Ok(())
    })
}

#[test]
fn a_wide_event_follows_its_last_play_action() -> TestResult {
    let mut actions = vec![2; 1_024];
    actions.insert(0, 1);
    actions.push(3);
    let bank = TestBank::new(1)
        .object(TestHircObject::event(wwise_short_id(TRIGGER), &actions))
        .object(TestHircObject::action(1, PLAY, 10))
        .object(TestHircObject::action(2, STOP, 10))
        .object(TestHircObject::action(3, PLAY, 11))
        .object(TestHircObject::sound(10, 7, 2, 0))
        .object(TestHircObject::sound(11, 8, 2, 0))
        .bytes();
    resolve_bytes(&bank, |sounds| {
        assert!(
            sounds
                .variants()
                .iter()
                .map(SoundVariant::wem_id)
                .eq([Some(7), Some(8)]),
            "the first and last authored Play actions are both reached, in order"
        );
        assert!(
            sounds.breaks().is_empty(),
            "no broken links: {:?}",
            sounds.breaks()
        );
        Ok(())
    })
}

#[test]
fn a_terminal_play_after_more_than_sixteen_thousand_visits_is_reached() -> TestResult {
    let mut actions = vec![1; 128];
    actions.push(3);
    let bank = TestBank::new(1)
        .object(TestHircObject::event(wwise_short_id(TRIGGER), &actions))
        .object(TestHircObject::action(1, PLAY_EVENT, 100))
        .object(TestHircObject::event(100, &[2; 128]))
        .object(TestHircObject::action(2, STOP, 10))
        .object(TestHircObject::action(3, PLAY, 10))
        .object(TestHircObject::sound(10, 7, 2, 0))
        .bytes();
    resolve_bytes(&bank, |sounds| {
        let [variant] = sounds.variants() else {
            return Err(format!("one terminal variant: {:?}", sounds.breaks()).into());
        };
        assert_eq!(
            variant.wem_id(),
            Some(7),
            "all earlier actions were visited"
        );
        assert!(
            sounds.breaks().is_empty(),
            "no broken links: {:?}",
            sounds.breaks()
        );
        Ok(())
    })
}

#[test]
fn an_event_listing_itself_reports_each_cycle_without_following_it() -> TestResult {
    const REFERENCES: usize = 1_029;
    let event = wwise_short_id(TRIGGER);
    let bytes = TestBank::new(1)
        .object(TestHircObject::event(event, &[event; REFERENCES]))
        .bytes();
    resolve_bytes(&bytes, |sounds| {
        assert!(
            sounds.variants().is_empty(),
            "cyclic actions never yield variants"
        );
        assert_eq!(sounds.breaks().len(), REFERENCES, "one break per cycle");
        assert!(
            sounds
                .breaks()
                .iter()
                .all(|link| *link.reason() == BreakReason::Cycle { id: event }),
            "cycles do not become traversal cutoffs"
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
