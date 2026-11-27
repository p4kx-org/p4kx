// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Resolution and export of Sound sources with and without a located media file.

use std::error::Error;
use std::path::Path;

use p4kx_assembly::audio::{
    ArchiveAudio, ExportFailureKind, ExportOptions, ExportReportCapture, ExportWatch,
    LanguageChoice, SoundFormat, SoundVariant, SourceKind, TriggerSounds,
};
use p4kx_extraction::{
    Archive, ConflictPolicy, FieldOrigin, SourceLocation, SourceUseRole, SourceUses,
};
use p4kx_testkit::{ArchiveBuilder, TestAtl, TestBank, TestEntry, TestHircObject, wwise_short_id};

type TestResult = Result<(), Box<dyn Error>>;

const PLAY: u16 = 0x0403;
const MISSING_WEM: u32 = 8000;

fn bank() -> Vec<u8> {
    TestBank::new(wwise_short_id("Media"))
        .wem(7000, b"MThd\0\0\0\x06\0\x01\0\x01\x01\xE0".to_vec())
        .object(TestHircObject::event(
            wwise_short_id("Play_Media"),
            &[1, 2, 3, 4, 4],
        ))
        .object(TestHircObject::action(1, PLAY, 10))
        .object(TestHircObject::action(2, PLAY, 20))
        .object(TestHircObject::action(3, PLAY, 30))
        .object(TestHircObject::action(4, PLAY, 40))
        .object(TestHircObject::plugin_sound(10, 6000))
        .object(TestHircObject::external_sound(20, 6001))
        .object(TestHircObject::midi_sound(30, 7000))
        .object(TestHircObject::sound(40, MISSING_WEM, 0, 0))
        .bytes()
}

#[test]
fn external_sources_have_no_file_and_midi_is_located_like_a_wem() -> TestResult {
    let definitions = TestAtl::new("SC_Media").trigger("Play_Media", &["Play_Media"]);
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::zstd(
            "Data\\Libs\\GameAudio\\SC_Media.xml",
            definitions.to_cryxml(),
        ))
        .entry(TestEntry::zstd("Data\\Sounds\\wwise\\Media.bnk", bank()).encrypted())
        .write_to(&path)?;
    let archive = Archive::open_with_key(&path, Some(p4kx_testkit::test_key()))?;
    let audio = ArchiveAudio::new(&archive);
    let sounds = audio
        .resolve_trigger_with_path_sources("Play_Media", &LanguageChoice::any())
        .ok_or("the trigger resolves")?;
    let seen: Vec<_> = sounds
        .variants()
        .iter()
        .map(|variant| {
            (
                variant.source_kind(),
                variant.source_kind().name(),
                variant.wem_id(),
                variant.location().is_some(),
            )
        })
        .collect();
    assert_eq!(
        seen,
        [
            (SourceKind::Plugin, "plugin", None, false),
            (SourceKind::External, "external", None, false),
            (SourceKind::Midi, "midi", Some(7000), true),
            (SourceKind::Embedded, "embedded", Some(MISSING_WEM), false),
            (SourceKind::Embedded, "embedded", Some(MISSING_WEM), false),
        ],
        "the two unresolved paths refer to the same unavailable WEM"
    );
    assert!(sounds.breaks().is_empty(), "{:?}", sounds.breaks());
    let fileless = sounds.variants().first().ok_or("plug-in variant")?;
    check_declared_source_id(&archive, fileless)?;
    check_failure_order(&audio, &sounds, dir.path())
}

fn check_failure_order(
    audio: &ArchiveAudio<'_>,
    sounds: &TriggerSounds<'_>,
    root: &Path,
) -> TestResult {
    for cancelled in [false, true] {
        let watch = ExportWatch::default();
        if cancelled {
            watch.cancel();
        }
        let folder = root.join(if cancelled { "cancelled" } else { "out" });
        let (report, origins) = audio.export_sounds_watched_with_report_origins(
            &[sounds],
            &folder,
            ExportOptions {
                format: SoundFormat::Raw,
                policy: ConflictPolicy::Overwrite,
            },
            &watch,
            ExportReportCapture::FileSizes,
        )?;
        let ids = report
            .failures
            .iter()
            .map(|failure| failure.wem_id)
            .collect::<Vec<_>>();
        let sites = origins
            .failures()
            .iter()
            .map(|origin| (origin.trigger_ordinal, origin.variant_ordinal, origin.kind))
            .collect::<Vec<_>>();
        assert_eq!(
            ids,
            [None, None, Some(MISSING_WEM)],
            "the planned report keeps its order"
        );
        assert_eq!(
            sites,
            [
                (0, 0, ExportFailureKind::FilelessSource),
                (0, 1, ExportFailureKind::FilelessSource),
                (0, 3, ExportFailureKind::UnresolvedMedia),
            ],
            "the deduplicated failure keeps its first actual variant"
        );
        assert_eq!(
            report.cancelled, cancelled,
            "only the second export is cancelled"
        );
        assert_eq!(
            folder.exists(),
            !cancelled,
            "a cancelled export creates no folder"
        );
    }
    Ok(())
}

fn check_declared_source_id(archive: &Archive, fileless: &SoundVariant<'_>) -> TestResult {
    let uses = SourceUses::new();
    let terminal = fileless.terminal_media_origins(archive, &uses)?;
    let inventory = uses.freeze()?;
    let is_declared_id = |field: &FieldOrigin| -> Result<bool, Box<dyn Error>> {
        let selected = inventory.select_uses(field.source_uses_refs())?;
        let found = selected.iter_uses().any(|used| {
            used.role() == SourceUseRole::Payload
                && matches!(used.location(), SourceLocation::Subresource { field, .. } if field == "sourceId")
        });
        Ok(found)
    };
    assert!(
        is_declared_id(&terminal.declared_source_id)?,
        "the diagnostic ID uses the authored word"
    );
    assert_eq!(
        terminal.declared_source_id.contains_encrypted_source(),
        Some(true),
        "the word is in the protected bank"
    );
    assert!(
        !is_declared_id(&terminal.wem_id)?,
        "the absent WEM ID uses no numeric source ID"
    );
    Ok(())
}
