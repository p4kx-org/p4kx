// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The audio index of an Archive: Entity trigger references, ATL definitions and soundbank
//! events, joined by Wwise short IDs.

use std::error::Error;
use std::path::Path;

use p4kx_assembly::Entities;
use p4kx_assembly::audio::{
    ArchiveAudio, AtlDefinitions, AudioFileError, AudioIndex, AudioIssue, EntityAudio,
    EntityTrigger, Unmatched, WwiseEvent, trigger_references, wwise_id,
};
use p4kx_extraction::{Archive, CancelToken, DataForgeEntry};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestAtl, TestBank, TestEntity, TestEntry, TestHircObject,
    audio_schema, entity_schema, wwise_short_id,
};

type TestResult = Result<(), Box<dyn Error>>;

const PLAY: u16 = 0x0403;

fn database() -> DataForgeBuilder {
    let ship = TestEntity::new(0x1, "TEST_Ship", "entities/test_ship.xml")
        .audio_trigger("Play_Engine")
        .audio_trigger("Play_Missing")
        .audio_trigger("PLAY_ENGINE")
        .audio_trigger("")
        .audio_trigger("Play_Nowhere")
        .audio_trigger("Play_Silent")
        .sound_effect("Shield Power Up", "Play_Shield")
        .audio_trigger("Play_Voice");
    let quiet = TestEntity::new(0x2, "TEST_Quiet", "entities/test_quiet.xml");
    [ship, quiet].iter().fold(
        audio_schema(entity_schema(DataForgeBuilder::new())),
        |builder, entity| builder.record(entity.record()),
    )
}

fn bank(bank_id: u32, events: &[&str]) -> Vec<u8> {
    events
        .iter()
        .zip(1_u32..)
        .fold(TestBank::new(bank_id), |bank, (event, action)| {
            bank.object(TestHircObject::action(action, PLAY, 0x100 + action))
                .object(TestHircObject::event(wwise_short_id(event), &[action]))
        })
        .bytes()
}

#[expect(
    clippy::too_many_lines,
    reason = "one archive holds every kind of audio file under test"
)]
fn write_archive(dir: &Path) -> Result<std::path::PathBuf, Box<dyn Error>> {
    let ship = TestAtl::new("SC_Ship")
        .trigger("Play_Engine", &["Play_Engine_Loop"])
        .trigger("Play_Shield", &["Play_Shield_Up"])
        .trigger("Play_Nowhere", &["Play_Not_In_Any_Bank"])
        .trigger("Play_Silent", &[])
        .bank("Ship.bnk");
    let shadow = TestAtl::new("SC_Wraith").trigger("play_engine", &["Play_Shadowed"]);
    let voice = TestAtl::new("SC_Voice").localised_trigger("Play_Voice", "Play_DX_Hello");
    let path = dir.join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::zstd("Data\\Game2.dcb", database().build()))
        .entry(TestEntry::zstd(
            "Data\\Libs\\GameAudio\\SC_Ship.xml",
            ship.to_cryxml(),
        ))
        .entry(TestEntry::stored(
            "Data/Libs/GameAudio/SC_Voice.xml",
            voice.to_xml(),
        ))
        .entry(TestEntry::zstd(
            "Data\\Libs\\GameAudio\\SC_Wraith.xml",
            shadow.to_cryxml(),
        ))
        .entry(TestEntry::stored(
            "Data\\Libs\\GameAudio\\Broken.xml",
            b"not xml".to_vec(),
        ))
        .entry(TestEntry::stored(
            "Data\\Libs\\GameAudio\\Deep.xml",
            format!(
                "<ATLConfig>{}{}</ATLConfig>",
                "<a>".repeat(p4kx_cryxml::MAX_TEXT_DEPTH),
                "</a>".repeat(p4kx_cryxml::MAX_TEXT_DEPTH)
            )
            .into_bytes(),
        ))
        .entry(TestEntry::stored(
            "Data\\Libs\\Other\\Ignored.xml",
            ship.to_xml(),
        ))
        .entry(TestEntry::stored(
            "Data\\Sounds\\wwise\\Ship.bnk",
            bank(
                wwise_short_id("Ship"),
                &["Play_Engine_Loop", "Play_Shield_Up"],
            ),
        ))
        .entry(TestEntry::stored(
            "Data\\Sounds\\wwise\\Shared.bnk",
            bank(wwise_short_id("Shared"), &["Play_Shield_Up"]),
        ))
        .entry(TestEntry::stored(
            "Data\\Sounds\\wwise\\English(US)\\Voice.bnk",
            bank(wwise_short_id("Voice"), &["Play_DX_Hello"]),
        ))
        .entry(TestEntry::stored(
            "Data\\Sounds\\wwise\\Corrupt.bnk",
            b"BKHD".to_vec(),
        ))
        .write_to(&path)?;
    Ok(path)
}

fn with_archive(check: impl FnOnce(&Archive) -> TestResult) -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive =
        Archive::open_with_key(write_archive(dir.path())?, Some(p4kx_testkit::test_key()))?;
    check(&archive)
}

fn with_ship(check: impl FnOnce(&AudioIndex, &EntityAudio<'_>) -> TestResult) -> TestResult {
    with_archive(|archive| {
        let entry = DataForgeEntry::read(archive)?;
        let forge = entry.open()?;
        let entities = Entities::new(&forge);
        let index = AudioIndex::build(archive);
        let audio = index.entity_audio(&entities.find("TEST_Ship")?)?;
        check(&index, &audio)
    })
}

#[test]
fn wwise_ids_are_the_fnv1_hash_of_the_lower_case_name() {
    assert_eq!(wwise_id(""), 0x811C_9DC5, "the FNV-1 offset basis");
    assert_eq!(wwise_id("Init"), 1_355_168_291, "the ID of Init.bnk");
    assert_eq!(wwise_id("INIT"), wwise_id("init"), "case does not matter");
}

#[test]
fn trigger_references_are_read_from_the_entity_record() -> TestResult {
    let bytes = database().build();
    let forge = p4kx_dataforge::DataForge::open(&bytes)?;
    let ship = Entities::new(&forge).find("TEST_Ship")?;
    let references = trigger_references(&ship.tree()?);
    let found: Vec<(&str, &str, u32)> = references
        .iter()
        .map(|reference| (reference.name(), reference.location(), reference.count()))
        .collect();
    assert_eq!(
        found,
        [
            ("Play_Engine", "Components[0].triggers[0].audioTrigger", 2),
            ("Play_Missing", "Components[0].triggers[1].audioTrigger", 1),
            ("Play_Nowhere", "Components[0].triggers[4].audioTrigger", 1),
            ("Play_Silent", "Components[0].triggers[5].audioTrigger", 1),
            ("Play_Voice", "Components[0].triggers[6].audioTrigger", 1),
            (
                "Play_Shield",
                "Components[0].triggerEffects[0].audio.audioTrigger",
                1
            ),
        ],
        "each trigger once, ignoring case, with its first location"
    );
    let quiet = Entities::new(&forge).find("TEST_Quiet")?;
    assert!(trigger_references(&quiet.tree()?).is_empty(), "no audio");
    Ok(())
}

#[test]
fn atl_definitions_in_cryxml_and_text_map_triggers_to_wwise_events() -> TestResult {
    with_archive(|archive| {
        let index = AudioIndex::build(archive);
        let engine = index.trigger("PLAY_engine").ok_or("Play_Engine")?;
        assert_eq!(engine.name(), "Play_Engine", "the CryXml definition");
        let events: Vec<(&str, u32)> = engine
            .events()
            .iter()
            .map(|event| (event.name(), event.id()))
            .collect();
        assert_eq!(
            events,
            [("Play_Engine_Loop", wwise_short_id("play_engine_loop"))],
            "event name and ID"
        );
        assert_eq!(
            index.definition_path(engine),
            Some("Data\\Libs\\GameAudio\\SC_Ship.xml"),
            "the first definition by path wins over SC_Wraith"
        );
        let voice = index.trigger("Play_Voice").ok_or("Play_Voice")?;
        assert!(
            voice.events().first().is_some_and(WwiseEvent::is_localised),
            "the text definition marks the event as localised"
        );
        assert!(index.trigger("Play_Missing").is_none(), "undefined");
        assert_eq!(
            index.triggers().len(),
            5,
            "triggers outside GameAudio are not read"
        );
        Ok(())
    })
}

#[test]
fn atl_definitions_share_lookup_and_issue_prefix_without_opening_soundbanks() -> TestResult {
    with_archive(|archive| {
        let definitions = AtlDefinitions::build(archive);
        let audio = AudioIndex::build(archive);
        assert_eq!(
            definitions.triggers(),
            audio.triggers(),
            "same authored order, event names, IDs and localisation",
        );
        for trigger in definitions.triggers() {
            let from_audio = audio.trigger(trigger.name()).ok_or("full index trigger")?;
            assert_eq!(
                definitions.definition_path(trigger),
                audio.definition_path(from_audio),
                "same winning path for {trigger:?}",
            );
            assert_eq!(
                definitions.definition_entry_position(trigger),
                audio.definition_entry_position(from_audio),
                "same original Entry position for {trigger:?}",
            );
        }

        let (bank_issue, atl_issues) = audio
            .issues()
            .split_last()
            .ok_or("full index reports the malformed BNK")?;
        assert_eq!(
            definitions.issues(),
            atl_issues,
            "same original ATL failures"
        );
        assert!(
            matches!(bank_issue.error(), AudioFileError::Soundbank(_)),
            "only the full index opens the malformed BNK",
        );

        let cancel = CancelToken::new();
        cancel.cancel();
        assert!(
            AtlDefinitions::build_until(archive, &cancel).is_none(),
            "a cancelled read returns no trigger index",
        );
        Ok(())
    })
}

#[test]
fn events_are_matched_to_the_soundbanks_that_hold_them() -> TestResult {
    with_ship(|index, audio| {
        let banks = |name: &str| -> Vec<String> {
            audio
                .triggers()
                .iter()
                .filter(|trigger| trigger.name() == name)
                .flat_map(EntityTrigger::events)
                .flat_map(|event| event.soundbanks())
                .map(|bank| format!("{}:{:?}", bank.file_name(), bank.language()))
                .collect()
        };
        assert_eq!(banks("Play_Engine"), ["Ship.bnk:None"], "one bank");
        assert_eq!(
            banks("Play_Shield"),
            ["Shared.bnk:None", "Ship.bnk:None"],
            "every bank holding the event, in path order"
        );
        assert_eq!(
            banks("Play_Voice"),
            ["Voice.bnk:Some(\"English(US)\")"],
            "a localised bank"
        );
        let ship = index
            .soundbanks()
            .iter()
            .find(|bank| bank.file_name() == "Ship.bnk")
            .ok_or("Ship.bnk")?;
        assert_eq!(ship.id(), wwise_id("ship"), "the bank ID");
        assert_eq!(ship.event_count(), 2, "its events");
        assert_eq!(audio.matched_count(), 3, "three triggers reach a bank");
        Ok(())
    })
}

#[test]
fn unmatched_triggers_say_where_the_chain_breaks() -> TestResult {
    with_ship(|_, audio| {
        let reasons: Vec<(&str, Option<&str>)> = audio
            .triggers()
            .iter()
            .map(|trigger| (trigger.name(), trigger.unmatched().map(Unmatched::kind)))
            .collect();
        assert_eq!(
            reasons,
            [
                ("Play_Engine", None),
                ("Play_Missing", Some("no_definition")),
                ("Play_Nowhere", Some("no_soundbank")),
                ("Play_Silent", Some("no_event")),
                ("Play_Voice", None),
                ("Play_Shield", None),
            ],
            "reasons"
        );
        Ok(())
    })
}

#[test]
fn unreadable_definitions_and_soundbanks_are_reported_and_the_rest_indexed() -> TestResult {
    with_archive(|archive| {
        let index = AudioIndex::build(archive);
        let issues: Vec<(&str, &str)> = index
            .issues()
            .iter()
            .map(|issue| (issue.path(), issue.error().kind()))
            .collect();
        assert_eq!(
            issues,
            [
                ("Data\\Libs\\GameAudio\\Broken.xml", "atl"),
                ("Data\\Libs\\GameAudio\\Deep.xml", "atl"),
                ("Data\\Sounds\\wwise\\Corrupt.bnk", "soundbank"),
            ],
            "issues"
        );
        assert!(
            matches!(
                index.issues().get(2).map(AudioIssue::error),
                Some(AudioFileError::Soundbank(_))
            ),
            "the parse error is kept"
        );
        assert_eq!(index.soundbanks().len(), 3, "the other banks");
        Ok(())
    })
}

#[test]
fn the_index_is_built_once_per_open_archive() -> TestResult {
    with_archive(|archive| {
        let audio = ArchiveAudio::new(archive);
        let first = audio.index();
        let second = std::thread::scope(|scope| scope.spawn(|| audio.index()).join())
            .map_err(|_| "the thread panicked")?;
        assert!(std::ptr::eq(first, second), "the same index");
        assert!(first.trigger("Play_Shield").is_some(), "built");
        Ok(())
    })
}
