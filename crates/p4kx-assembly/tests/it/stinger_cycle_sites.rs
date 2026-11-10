// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::audio::{ArchiveAudio, BreakReason, BrokenLink, IndexedSoundbank};
use p4kx_audio::{HircValue, Soundbank};
use p4kx_extraction::Archive;
use p4kx_testkit::{ArchiveBuilder, TestAtl, TestBank, TestEntry, TestHircObject};

use super::{ACTION, EVENT, TRIGGER, TestResult};

const MUSIC_TRIGGER: u32 = 0x77;
const HOLDER: u32 = 20;
const TRACK: u32 = 31;
const FIRST_EVENT: u32 = 200;
const RETURN_ACTION: u32 = 201;
const HOME: &str = "Data\\Sounds\\wwise\\Z-cycle-home.bnk";
const FOREIGN: &str = "Data\\Sounds\\wwise\\A-cycle-target.bnk";

#[derive(Clone, Copy)]
enum Case {
    Present,
    Missing,
    Return,
}

#[test]
fn physical_cycle_at_target_uses_its_actual_foreign_bank_before_comparison() -> TestResult {
    original(Case::Present, |archive| {
        let audio = ArchiveAudio::new(archive);
        let sounds = audio
            .resolve_trigger(TRIGGER)
            .ok_or("actual source trigger")?;
        admitted(&audio);
        let actual: Vec<_> = sounds
            .variants()
            .iter()
            .map(|variant| (variant.wem_id(), variant.soundbank().file_name()))
            .collect();
        assert_eq!(
            actual,
            [(Some(7001), "A-cycle-target.bnk")],
            "actual foreign Segment10 differs from home Action10"
        );
        assert!(
            sounds.breaks().is_empty(),
            "selected foreign At target is not the ancestor"
        );
        let selected = sounds.variants().first().ok_or("actual foreign terminal")?;
        let ids: Vec<_> = selected.path().iter().map(|step| step.id).collect();
        assert_eq!(
            ids,
            [root_event(), ACTION, ACTION, TRACK],
            "unchanged public path retains both actual occurrences"
        );
        Ok(())
    })
}

#[test]
fn physical_cycle_at_target_that_is_really_missing_stays_missing() -> TestResult {
    original(Case::Missing, |archive| {
        let audio = ArchiveAudio::new(archive);
        let sounds = audio
            .resolve_trigger(TRIGGER)
            .ok_or("actual source trigger")?;
        admitted(&audio);
        assert!(
            sounds.variants().is_empty(),
            "no invented local or foreign terminal"
        );
        let reasons: Vec<_> = sounds.breaks().iter().map(BrokenLink::reason).collect();
        assert_eq!(
            reasons,
            [&BreakReason::Missing { id: ACTION }],
            "actual selected foreign absence cannot become ancestor Cycle"
        );
        Ok(())
    })
}

#[test]
fn physical_cycle_same_original_return_preserves_the_real_cycle() -> TestResult {
    original(Case::Return, |archive| {
        let audio = ArchiveAudio::new(archive);
        let sounds = audio
            .resolve_trigger(TRIGGER)
            .ok_or("actual source trigger")?;
        admitted(&audio);
        assert!(
            sounds.variants().is_empty(),
            "real cycle has no invented sound"
        );
        let reasons: Vec<_> = sounds.breaks().iter().map(BrokenLink::reason).collect();
        assert_eq!(
            reasons,
            [&BreakReason::Cycle { id: root_event() }],
            "genuine actual home Event is revisited"
        );
        let refusal = sounds.breaks().first().ok_or("actual cycle")?;
        let ids: Vec<_> = refusal.path().iter().map(|step| step.id).collect();
        assert_eq!(
            ids,
            [root_event(), ACTION, FIRST_EVENT, RETURN_ACTION],
            "exact original pre-return public path"
        );
        Ok(())
    })
}

fn root_event() -> u32 {
    p4kx_testkit::wwise_short_id(EVENT)
}

fn banks(case: Case) -> [(String, Vec<u8>); 2] {
    let (kind, target) = if matches!(case, Case::Return) {
        (0x2103, FIRST_EVENT)
    } else {
        (0x1B03, MUSIC_TRIGGER)
    };
    let home = TestBank::new(p4kx_testkit::wwise_short_id("Z-cycle-home"))
        .object(TestHircObject::event(root_event(), &[ACTION]))
        .object(TestHircObject::action(ACTION, kind, target));
    let mut foreign = TestBank::new(p4kx_testkit::wwise_short_id("A-cycle-target"));
    if matches!(case, Case::Return) {
        foreign = foreign
            .object(TestHircObject::event(FIRST_EVENT, &[RETURN_ACTION]))
            .object(TestHircObject::action(RETURN_ACTION, 0x2103, root_event()));
    } else {
        foreign = foreign.object(TestHircObject::music_segment_with_stingers(
            HOLDER,
            &[],
            &[(MUSIC_TRIGGER, ACTION)],
        ));
        if matches!(case, Case::Present) {
            foreign = foreign
                .object(TestHircObject::music_segment_with_stingers(
                    ACTION,
                    &[TRACK],
                    &[],
                ))
                .object(TestHircObject::music_track(TRACK, &[7001]));
        }
    }
    [
        (HOME.to_owned(), home.bytes()),
        (FOREIGN.to_owned(), foreign.bytes()),
    ]
}

fn original(case: Case, check: impl FnOnce(&Archive) -> TestResult) -> TestResult {
    assert!(
        ![ACTION, HOLDER, TRACK, FIRST_EVENT, RETURN_ACTION].contains(&root_event()),
        "fixture hash has no accidental object collision"
    );
    let banks = banks(case);
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("physical-cycle-sites.p4k");
    let atl = TestAtl::new("PhysicalCycle")
        .trigger(TRIGGER, &[EVENT])
        .to_xml();
    let atl_path = "Data\\Libs\\GameAudio\\physical-cycle.xml";
    let mut builder = ArchiveBuilder::new().entry(TestEntry::stored(atl_path, atl.as_bytes()));
    for (path, bytes) in &banks {
        builder = builder.entry(TestEntry::stored(path.as_str(), bytes.as_slice()));
    }
    builder.write_to(&path)?;
    let archive = Archive::open(path)?;
    super::assert_bytes(&archive, atl_path, atl.as_bytes())?;
    for (path, bytes) in &banks {
        super::assert_bytes(&archive, path, bytes)?;
        let source = Soundbank::parse(bytes.as_slice())?;
        assert_eq!(source.header().version, 154, "actual original layout");
        assert!(
            source.warnings().is_empty(),
            "no source-layout error stands in for semantic RED"
        );
        if path == HOME {
            assert_home(&source, case)?;
        } else {
            assert_foreign(&source, case)?;
        }
    }
    check(&archive)
}

fn assert_home(source: &Soundbank<&[u8]>, case: Case) -> TestResult {
    assert_eq!(
        source.header().id,
        p4kx_testkit::wwise_short_id("Z-cycle-home"),
        "actual home owner"
    );
    let ids: Vec<_> = source.objects().iter().map(|object| object.id).collect();
    assert_eq!(ids, [root_event(), ACTION], "all original home objects");
    let event = source.object(root_event()).ok_or("original home Event")?;
    let HircValue::Event(actions) = &event.value else {
        return Err("actual home Event type".into());
    };
    assert_eq!(actions, &[ACTION], "actual home Event-to-action edge");
    let action = source.object(ACTION).ok_or("original ancestor action")?;
    let HircValue::Action(action) = &action.value else {
        return Err("actual ancestor Action type".into());
    };
    let expected = if matches!(case, Case::Return) {
        (0x2103, FIRST_EVENT)
    } else {
        (0x1B03, MUSIC_TRIGGER)
    };
    assert_eq!(
        (action.action_type, action.target_id),
        expected,
        "exact original home selector"
    );
    Ok(())
}

fn assert_foreign(source: &Soundbank<&[u8]>, case: Case) -> TestResult {
    assert_eq!(
        source.header().id,
        p4kx_testkit::wwise_short_id("A-cycle-target"),
        "actual distinct foreign owner"
    );
    if matches!(case, Case::Return) {
        return assert_return(source);
    }
    let ids: Vec<_> = source.objects().iter().map(|object| object.id).collect();
    assert_eq!(
        ids,
        if matches!(case, Case::Present) {
            vec![HOLDER, ACTION, TRACK]
        } else {
            vec![HOLDER]
        },
        "all original foreign objects"
    );
    let holder = source.object(HOLDER).ok_or("original stinger holder")?;
    let HircValue::MusicSegment(holder) = &holder.value else {
        return Err("actual holder type".into());
    };
    let stingers: Vec<_> = holder
        .node
        .stingers
        .iter()
        .map(|item| (item.trigger_id, item.segment_id))
        .collect();
    assert_eq!(
        stingers,
        [(MUSIC_TRIGGER, ACTION)],
        "exact original indexed foreign target"
    );
    if matches!(case, Case::Missing) {
        assert!(
            source.object(ACTION).is_none(),
            "foreign target is genuinely absent despite equal home ancestor ID"
        );
        return Ok(());
    }
    assert_terminal(source)
}

fn assert_terminal(source: &Soundbank<&[u8]>) -> TestResult {
    let segment = source.object(ACTION).ok_or("original foreign target")?;
    let HircValue::MusicSegment(segment) = &segment.value else {
        return Err("actual foreign Segment type".into());
    };
    assert_eq!(
        segment.node.children,
        [TRACK],
        "original foreign target-to-track edge"
    );
    assert!(
        segment.node.stingers.is_empty(),
        "target introduces no hidden stinger"
    );
    let track = source.object(TRACK).ok_or("original terminal track")?;
    let HircValue::MusicTrack(track) = &track.value else {
        return Err("actual track type".into());
    };
    let media: Vec<_> = track.sources.iter().map(|item| item.source_id).collect();
    assert_eq!(media, [7001], "actual original terminal media ID");
    Ok(())
}

fn assert_return(source: &Soundbank<&[u8]>) -> TestResult {
    let ids: Vec<_> = source.objects().iter().map(|object| object.id).collect();
    assert_eq!(
        ids,
        [FIRST_EVENT, RETURN_ACTION],
        "all genuine original return objects"
    );
    let event = source.object(FIRST_EVENT).ok_or("original foreign Event")?;
    let HircValue::Event(actions) = &event.value else {
        return Err("actual return Event type".into());
    };
    assert_eq!(
        actions,
        &[RETURN_ACTION],
        "original return action selection"
    );
    let action = source
        .object(RETURN_ACTION)
        .ok_or("original return action")?;
    let HircValue::Action(action) = &action.value else {
        return Err("actual return Action type".into());
    };
    assert_eq!(
        (action.action_type, action.target_id),
        (0x2103, root_event()),
        "actual source returns to the unique original home Event"
    );
    Ok(())
}

fn admitted(audio: &ArchiveAudio<'_>) {
    assert!(
        audio.index().issues().is_empty(),
        "all genuine source banks admitted without parser or index errors"
    );
    let names: Vec<_> = audio
        .index()
        .soundbanks()
        .iter()
        .map(IndexedSoundbank::file_name)
        .collect();
    assert_eq!(
        names,
        ["A-cycle-target.bnk", "Z-cycle-home.bnk"],
        "both actual physical bank inputs in canonical index order"
    );
}
