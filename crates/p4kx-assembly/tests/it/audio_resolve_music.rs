// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Trigger resolution through music objects, events that post events, and Play actions that name
//! another soundbank.

use std::error::Error;

use p4kx_assembly::audio::{ArchiveAudio, BreakReason, BrokenLink, SoundVariant, TriggerSounds};
use p4kx_extraction::Archive;
use p4kx_testkit::{ArchiveBuilder, TestAtl, TestBank, TestEntry, TestHircObject, wwise_short_id};

type TestResult = Result<(), Box<dyn Error>>;

const PLAY: u16 = 0x0403;
const PLAY_EVENT: u16 = 0x2103;

fn with_bank(bank: &TestBank, check: impl FnOnce(TriggerSounds<'_>) -> TestResult) -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::zstd(
            "Data\\Libs\\GameAudio\\SC_Music.xml",
            TestAtl::new("SC_Music")
                .trigger("Play_Music", &["Play_Music"])
                .to_cryxml(),
        ))
        .entry(TestEntry::zstd(
            "Data\\Sounds\\wwise\\Music.bnk",
            bank.bytes(),
        ))
        .write_to(&path)?;
    let archive = Archive::open_with_key(&path, Some(p4kx_testkit::test_key()))?;
    let audio = ArchiveAudio::new(&archive);
    let sounds = audio
        .resolve_trigger("Play_Music")
        .ok_or("Play_Music resolves")?;
    check(sounds)
}

fn music_bank() -> TestBank {
    TestBank::new(wwise_short_id("Music"))
        .object(TestHircObject::event(wwise_short_id("Play_Music"), &[1]))
        .object(TestHircObject::action(1, PLAY, 10))
}

#[test]
fn a_play_event_action_to_an_event_no_soundbank_holds_breaks_as_missing() -> TestResult {
    let bank = TestBank::new(wwise_short_id("Music"))
        .object(TestHircObject::event(wwise_short_id("Play_Music"), &[1]))
        .object(TestHircObject::action(1, PLAY_EVENT, 501));
    with_bank(&bank, |sounds| {
        let reasons: Vec<_> = sounds.breaks().iter().map(BrokenLink::reason).collect();
        assert_eq!(reasons, [&BreakReason::Missing { id: 501 }]);
        Ok(())
    })
}

#[test]
fn a_playlist_leads_through_segments_and_tracks_to_their_sources() -> TestResult {
    let bank = music_bank()
        .object(TestHircObject::music_playlist(10, &[20, 21]))
        .object(TestHircObject::music_segment(20, &[30], &["Entry"]))
        .object(TestHircObject::music_segment(21, &[31], &[]))
        .object(TestHircObject::music_track(30, &[7000, 7001]))
        .object(TestHircObject::music_track(31, &[7002]));
    with_bank(&bank, |sounds| {
        let found: Vec<_> = sounds
            .variants()
            .iter()
            .map(|variant| {
                (
                    variant.wem_id(),
                    variant.sound_id(),
                    variant
                        .path()
                        .iter()
                        .map(|step| step.kind.name())
                        .collect::<Vec<_>>(),
                )
            })
            .collect();
        let path = |segment| vec!["event", "action", "music_playlist", segment, "music_track"];
        assert_eq!(
            found,
            [
                (Some(7000), 30, path("music_segment")),
                (Some(7001), 30, path("music_segment")),
                (Some(7002), 31, path("music_segment")),
            ]
        );
        assert!(sounds.breaks().is_empty(), "{:?}", sounds.breaks());
        Ok(())
    })
}

#[test]
fn a_music_switch_container_leads_to_its_children() -> TestResult {
    let bank = music_bank()
        .object(TestHircObject::music_switch(10, &[20], 0x4000, &[20]))
        .object(TestHircObject::music_segment(20, &[30], &[]))
        .object(TestHircObject::music_track(30, &[7000]));
    with_bank(&bank, |sounds| {
        assert_eq!(
            sounds
                .variants()
                .iter()
                .map(SoundVariant::wem_id)
                .collect::<Vec<_>>(),
            [Some(7000)]
        );
        Ok(())
    })
}

#[test]
fn music_objects_that_refer_back_are_cut_as_cycles() -> TestResult {
    let bank = music_bank()
        .object(TestHircObject::music_playlist(10, &[20]))
        .object(TestHircObject::music_segment(20, &[10], &[]));
    with_bank(&bank, |sounds| {
        assert_eq!(sounds.variants().len(), 0, "no variants");
        let [link] = sounds.breaks() else {
            return Err(format!("one break: {:?}", sounds.breaks()).into());
        };
        assert_eq!(link.reason(), &BreakReason::Cycle { id: 10 });
        assert_eq!(
            link.object().map(|step| step.kind.name()),
            Some("music_segment")
        );
        Ok(())
    })
}

#[test]
fn a_wide_music_track_resolves_every_authored_source_in_order() -> TestResult {
    let sources: Vec<u32> = (9000..).take(1_029).collect();
    let bank = music_bank().object(TestHircObject::music_track(10, &sources));
    with_bank(&bank, |sounds| {
        assert!(
            sounds
                .variants()
                .iter()
                .map(SoundVariant::wem_id)
                .eq(sources.iter().copied().map(Some)),
            "each MusicTrack source appears in the authored order"
        );
        assert!(sounds.breaks().is_empty(), "{:?}", sounds.breaks());
        Ok(())
    })
}

#[test]
fn a_play_event_action_follows_the_event_it_posts() -> TestResult {
    let bank = TestBank::new(wwise_short_id("Music"))
        .object(TestHircObject::event(wwise_short_id("Play_Music"), &[1]))
        .object(TestHircObject::action(1, PLAY_EVENT, 500))
        .object(TestHircObject::event(500, &[2]))
        .object(TestHircObject::action(2, PLAY, 10))
        .object(TestHircObject::sound(10, 7000, 2, 0));
    with_bank(&bank, |sounds| {
        let [variant] = sounds.variants() else {
            return Err(format!("one variant: {:?}", sounds.variants()).into());
        };
        assert_eq!(variant.wem_id(), Some(7000));
        assert_eq!(
            variant
                .path()
                .iter()
                .map(|step| step.kind.name())
                .collect::<Vec<_>>(),
            ["event", "action", "event", "action", "sound"]
        );
        assert!(sounds.breaks().is_empty(), "{:?}", sounds.breaks());
        Ok(())
    })
}

#[test]
fn an_event_that_posts_itself_is_cut_as_a_cycle() -> TestResult {
    let id = wwise_short_id("Play_Music");
    let bank = TestBank::new(wwise_short_id("Music"))
        .object(TestHircObject::event(id, &[1]))
        .object(TestHircObject::action(1, PLAY_EVENT, id));
    with_bank(&bank, |sounds| {
        assert_eq!(sounds.variants().len(), 0, "no variants");
        let [link] = sounds.breaks() else {
            return Err(format!("one break: {:?}", sounds.breaks()).into());
        };
        assert_eq!(link.reason(), &BreakReason::Cycle { id });
        Ok(())
    })
}

#[test]
fn a_play_action_naming_another_soundbank_breaks_with_that_soundbank() -> TestResult {
    let other = wwise_short_id("Elsewhere");
    let bank = TestBank::new(wwise_short_id("Music"))
        .object(TestHircObject::event(wwise_short_id("Play_Music"), &[1, 2]))
        .object(TestHircObject::action_in_bank(1, PLAY, 10, other))
        .object(TestHircObject::action_in_bank(
            2,
            PLAY,
            11,
            wwise_short_id("Music"),
        ));
    with_bank(&bank, |sounds| {
        let reasons: Vec<_> = sounds.breaks().iter().map(BrokenLink::reason).collect();
        assert_eq!(
            reasons,
            [
                &BreakReason::OtherSoundbank {
                    id: 10,
                    bank_id: other
                },
                &BreakReason::Missing { id: 11 }
            ]
        );
        assert_eq!(sounds.breaks()[0].reason().kind(), "other_soundbank");
        Ok(())
    })
}
