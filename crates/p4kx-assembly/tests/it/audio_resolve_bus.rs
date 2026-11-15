// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::audio::{ArchiveAudio, BreakReason};
use p4kx_extraction::Archive;
use p4kx_testkit::{ArchiveBuilder, TestAtl, TestBank, TestEntry, TestHircObject, wwise_short_id};

#[test]
fn malformed_bus_targets_keep_the_unsupported_playback_reason()
-> Result<(), Box<dyn std::error::Error>> {
    const PLAY: u16 = 0x0403;
    const TRIGGER: &str = "Play_Bus_Targets";
    let original = TestBank::new(wwise_short_id("Ship"))
        .version(154)
        .object(TestHircObject::event(wwise_short_id(TRIGGER), &[1, 2]))
        .object(TestHircObject::action(1, PLAY, 10))
        .object(TestHircObject::action(2, PLAY, 20))
        .object(TestHircObject::raw(8, 10, []))
        .object(TestHircObject::raw(18, 20, []))
        .bytes();
    let definitions = TestAtl::new("Ship").trigger(TRIGGER, &[TRIGGER]);
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::zstd(
            "Data\\Libs\\GameAudio\\Ship.xml",
            definitions.to_cryxml(),
        ))
        .entry(TestEntry::zstd("Data\\Sounds\\wwise\\Ship.bnk", original))
        .write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?;
    let audio = ArchiveAudio::new(&archive);
    let report = audio.resolve_trigger(TRIGGER).ok_or("trigger resolves")?;
    assert!(report.variants().is_empty());
    let [primary, auxiliary] = report.breaks() else {
        return Err("both bus targets retain their playback disposition".into());
    };
    for (link, id) in [(primary, 10), (auxiliary, 20)] {
        assert_eq!(link.object().map(|step| step.id), Some(id));
        assert_eq!(link.reason(), &BreakReason::Unsupported);
    }
    Ok(())
}
