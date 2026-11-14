// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Trigger resolution across soundbanks: Play and `PlayEvent` actions into other soundbanks and
//! Trigger actions into the stingers of music.

use std::error::Error;

use p4kx_assembly::audio::{
    ArchiveAudio, BreakReason, BrokenLink, LanguageChoice, MAX_ACTION_CHAIN, TriggerSounds,
};
use p4kx_extraction::Archive;
use p4kx_testkit::{ArchiveBuilder, TestAtl, TestBank, TestEntry, TestHircObject, wwise_short_id};

type TestResult = Result<(), Box<dyn Error>>;

pub(crate) const PLAY: u16 = 0x0403;
const PLAY_EVENT: u16 = 0x2103;
pub(crate) const TRIGGER: u16 = 0x1B03;

pub(crate) fn resolve(
    banks: &[(&str, TestBank)],
    choice: &LanguageChoice,
    check: impl FnOnce(TriggerSounds<'_>) -> TestResult,
) -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("Data.p4k");
    let mut builder = ArchiveBuilder::new().entry(TestEntry::zstd(
        "Data\\Libs\\GameAudio\\SC_Music.xml",
        TestAtl::new("SC_Music")
            .trigger("Play_Music", &["Play_Music"])
            .to_cryxml(),
    ));
    for (name, bank) in banks {
        builder = builder.entry(TestEntry::zstd(
            format!("Data\\Sounds\\wwise\\{name}.bnk"),
            bank.bytes(),
        ));
    }
    builder.write_to(&path)?;
    let archive = Archive::open_with_key(&path, Some(p4kx_testkit::test_key()))?;
    let audio = ArchiveAudio::new(&archive);
    let sounds = audio
        .resolve_trigger_in("Play_Music", choice)
        .ok_or("Play_Music resolves")?;
    check(sounds)
}

pub(crate) fn music(actions: &[(u16, u32)]) -> TestBank {
    let ids: Vec<u32> = (1..).take(actions.len()).collect();
    let mut bank = TestBank::new(wwise_short_id("Music"))
        .object(TestHircObject::event(wwise_short_id("Play_Music"), &ids));
    for (id, &(action, target)) in ids.iter().zip(actions) {
        bank = bank.object(TestHircObject::action(*id, action, target));
    }
    bank
}

fn names(sounds: &TriggerSounds<'_>) -> Vec<(Option<u32>, String)> {
    sounds
        .variants()
        .iter()
        .map(|variant| (variant.wem_id(), variant.soundbank().file_name().to_owned()))
        .collect()
}

#[test]
fn a_play_action_follows_the_soundbank_it_names() -> TestResult {
    let elsewhere =
        TestBank::new(wwise_short_id("Elsewhere")).object(TestHircObject::sound(10, 7000, 2, 0));
    let music = TestBank::new(wwise_short_id("Music"))
        .object(TestHircObject::event(wwise_short_id("Play_Music"), &[1]))
        .object(TestHircObject::action_in_bank(
            1,
            PLAY,
            10,
            wwise_short_id("Elsewhere"),
        ));
    resolve(
        &[("Music", music), ("Elsewhere", elsewhere)],
        &LanguageChoice::any(),
        |sounds| {
            assert_eq!(names(&sounds), [(Some(7000), "Elsewhere.bnk".to_owned())]);
            assert!(sounds.breaks().is_empty(), "{:?}", sounds.breaks());
            Ok(())
        },
    )
}

#[test]
fn a_play_action_naming_a_soundbank_the_archive_lacks_breaks() -> TestResult {
    let other = wwise_short_id("Absent");
    let music = TestBank::new(wwise_short_id("Music"))
        .object(TestHircObject::event(wwise_short_id("Play_Music"), &[1]))
        .object(TestHircObject::action_in_bank(1, PLAY, 10, other));
    resolve(&[("Music", music)], &LanguageChoice::any(), |sounds| {
        let reasons: Vec<_> = sounds.breaks().iter().map(BrokenLink::reason).collect();
        assert_eq!(
            reasons,
            [&BreakReason::OtherSoundbank {
                id: 10,
                bank_id: other
            }]
        );
        Ok(())
    })
}

#[test]
fn a_play_action_whose_target_the_named_soundbank_lacks_breaks_as_missing() -> TestResult {
    let elsewhere = TestBank::new(wwise_short_id("Elsewhere"));
    let music = TestBank::new(wwise_short_id("Music"))
        .object(TestHircObject::event(wwise_short_id("Play_Music"), &[1]))
        .object(TestHircObject::action_in_bank(
            1,
            PLAY,
            10,
            wwise_short_id("Elsewhere"),
        ));
    resolve(
        &[("Music", music), ("Elsewhere", elsewhere)],
        &LanguageChoice::any(),
        |sounds| {
            let reasons: Vec<_> = sounds.breaks().iter().map(BrokenLink::reason).collect();
            assert_eq!(reasons, [&BreakReason::Missing { id: 10 }]);
            Ok(())
        },
    )
}

#[test]
fn a_play_event_action_follows_an_event_another_soundbank_holds() -> TestResult {
    let elsewhere = TestBank::new(wwise_short_id("Elsewhere"))
        .object(TestHircObject::event(500, &[2]))
        .object(TestHircObject::action(2, PLAY, 10))
        .object(TestHircObject::sound(10, 7000, 2, 0));
    resolve(
        &[
            ("Music", music(&[(PLAY_EVENT, 500)])),
            ("Elsewhere", elsewhere),
        ],
        &LanguageChoice::any(),
        |sounds| {
            let [variant] = sounds.variants() else {
                return Err(format!("one variant: {:?}", sounds.variants()).into());
            };
            assert_eq!(variant.soundbank().file_name(), "Elsewhere.bnk");
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
        },
    )
}

#[test]
fn a_walk_follows_each_referenced_indexed_soundbank() -> TestResult {
    let bank = |name: &str, event: u32, sound: u32| {
        TestBank::new(wwise_short_id(name))
            .object(TestHircObject::event(event, &[2]))
            .object(TestHircObject::action(2, PLAY, 10))
            .object(TestHircObject::sound(10, sound, 2, 0))
    };
    resolve(
        &[
            ("Music", music(&[(PLAY_EVENT, 500), (PLAY_EVENT, 600)])),
            ("First", bank("First", 500, 7000)),
            ("Second", bank("Second", 600, 7001)),
        ],
        &LanguageChoice::any(),
        |sounds| {
            assert_eq!(
                names(&sounds),
                [
                    (Some(7000), "First.bnk".to_owned()),
                    (Some(7001), "Second.bnk".to_owned())
                ],
                "both original foreign terminals remain in authored branch order"
            );
            assert!(
                sounds.breaks().is_empty(),
                "all original indexed references resolve: {:?}",
                sounds.breaks()
            );
            Ok(())
        },
    )
}

#[test]
fn the_other_soundbank_is_followed_again_for_a_second_reference() -> TestResult {
    let elsewhere = TestBank::new(wwise_short_id("Elsewhere"))
        .object(TestHircObject::event(500, &[2]))
        .object(TestHircObject::action(2, PLAY, 10))
        .object(TestHircObject::sound(10, 7000, 2, 0))
        .object(TestHircObject::event(600, &[3]))
        .object(TestHircObject::action(3, PLAY, 11))
        .object(TestHircObject::sound(11, 7001, 2, 0));
    resolve(
        &[
            ("Music", music(&[(PLAY_EVENT, 500), (PLAY_EVENT, 600)])),
            ("Elsewhere", elsewhere),
        ],
        &LanguageChoice::any(),
        |sounds| {
            assert_eq!(
                names(&sounds),
                [
                    (Some(7000), "Elsewhere.bnk".to_owned()),
                    (Some(7001), "Elsewhere.bnk".to_owned())
                ]
            );
            assert!(sounds.breaks().is_empty(), "{:?}", sounds.breaks());
            Ok(())
        },
    )
}

#[test]
fn an_event_posted_back_across_soundbanks_is_cut_as_a_cycle() -> TestResult {
    let id = wwise_short_id("Play_Music");
    let elsewhere = TestBank::new(wwise_short_id("Elsewhere"))
        .object(TestHircObject::event(500, &[2]))
        .object(TestHircObject::action(2, PLAY_EVENT, id));
    resolve(
        &[
            ("Music", music(&[(PLAY_EVENT, 500)])),
            ("Elsewhere", elsewhere),
        ],
        &LanguageChoice::any(),
        |sounds| {
            assert_eq!(sounds.variants().len(), 0, "no variants");
            let reasons: Vec<_> = sounds.breaks().iter().map(BrokenLink::reason).collect();
            assert_eq!(reasons, [&BreakReason::Cycle { id }]);
            Ok(())
        },
    )
}

#[test]
fn a_chain_of_play_event_actions_stops_at_the_limit() -> TestResult {
    let limit = u32::try_from(MAX_ACTION_CHAIN)?;
    resolve(
        &[("Music", chained(limit))],
        &LanguageChoice::any(),
        |sounds| {
            assert_eq!(sounds.variants().len(), 1);
            assert!(sounds.breaks().is_empty(), "{:?}", sounds.breaks());
            Ok(())
        },
    )?;
    resolve(
        &[("Music", chained(limit + 1))],
        &LanguageChoice::any(),
        |sounds| {
            assert_eq!(sounds.variants().len(), 0, "no variants");
            let reasons: Vec<_> = sounds.breaks().iter().map(BrokenLink::reason).collect();
            assert_eq!(
                reasons,
                [&BreakReason::ChainTooLong {
                    limit: MAX_ACTION_CHAIN
                }]
            );
            assert_eq!(sounds.breaks()[0].reason().kind(), "chain_too_long");
            Ok(())
        },
    )
}

fn chained(hops: u32) -> TestBank {
    let mut bank = TestBank::new(wwise_short_id("Music"))
        .object(TestHircObject::event(wwise_short_id("Play_Music"), &[100]));
    for hop in 0..hops {
        bank = bank
            .object(TestHircObject::action(100 + hop, PLAY_EVENT, 200 + hop))
            .object(TestHircObject::event(200 + hop, &[100 + hop + 1]));
    }
    bank.object(TestHircObject::action(100 + hops, PLAY, 300))
        .object(TestHircObject::sound(300, 7000, 2, 0))
}

pub(crate) fn stingers(trigger: u32) -> TestBank {
    TestBank::new(wwise_short_id("Stingers"))
        .object(TestHircObject::music_segment_stinging(20, &[], trigger, 21))
        .object(TestHircObject::music_segment(21, &[31], &[]))
        .object(TestHircObject::music_track(31, &[7000]))
}

#[test]
fn a_trigger_action_follows_the_stinger_to_its_segment_and_sources() -> TestResult {
    resolve(
        &[
            ("Music", music(&[(TRIGGER, 0x77)])),
            ("Stingers", stingers(0x77)),
        ],
        &LanguageChoice::any(),
        |sounds| {
            let [variant] = sounds.variants() else {
                return Err(format!("one variant: {:?}", sounds.variants()).into());
            };
            assert_eq!(variant.wem_id(), Some(7000));
            assert_eq!(variant.soundbank().file_name(), "Stingers.bnk");
            assert_eq!(
                variant
                    .path()
                    .iter()
                    .map(|step| step.kind.name())
                    .collect::<Vec<_>>(),
                ["event", "action", "music_segment", "music_track"]
            );
            assert!(sounds.breaks().is_empty(), "{:?}", sounds.breaks());
            Ok(())
        },
    )
}

#[test]
fn a_trigger_action_without_a_stinger_breaks_as_missing() -> TestResult {
    resolve(
        &[
            ("Music", music(&[(TRIGGER, 0x78)])),
            ("Stingers", stingers(0x77)),
        ],
        &LanguageChoice::any(),
        |sounds| {
            assert_eq!(sounds.variants().len(), 0, "no variants");
            let reasons: Vec<_> = sounds.breaks().iter().map(BrokenLink::reason).collect();
            assert_eq!(reasons, [&BreakReason::Missing { id: 0x78 }]);
            Ok(())
        },
    )
}
