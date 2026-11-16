// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Streamed sounds located as loose WEM Entries, and localised sounds resolved for a chosen
//! language with a fallback language.

use std::error::Error;

use p4kx_assembly::audio::{
    ArchiveAudio, BreakReason, LanguageChoice, SoundVariant, SourceKind, TriggerSounds, WemLocation,
};
use p4kx_extraction::Archive;
use p4kx_testkit::{ArchiveBuilder, TestAtl, TestBank, TestEntry, TestHircObject, wwise_short_id};

#[path = "audio_streamed_location_origins.rs"]
mod location_origins;

type TestResult = Result<(), Box<dyn Error>>;

const PLAY: u16 = 0x0403;

fn voice_bank(event: &str, wem: u32) -> Vec<u8> {
    TestBank::new(wwise_short_id(event))
        .object(TestHircObject::event(wwise_short_id(event), &[1]))
        .object(TestHircObject::action(1, PLAY, 10))
        .object(TestHircObject::sound(10, wem, 2, 0))
        .bytes()
}

fn common_bank() -> Vec<u8> {
    TestBank::new(wwise_short_id("Common"))
        .wem(7001, vec![0x11; 8])
        .object(TestHircObject::event(
            wwise_short_id("Play_Long"),
            &[1, 2, 3],
        ))
        .object(TestHircObject::action(1, PLAY, 200))
        .object(TestHircObject::action(2, PLAY, 201))
        .object(TestHircObject::action(3, PLAY, 202))
        .object(TestHircObject::sound(200, 7000, 2, 0))
        .object(TestHircObject::sound(201, 7001, 1, 8))
        .object(TestHircObject::sound(202, 7002, 2, 0))
        .object(TestHircObject::event(wwise_short_id("Play_Line"), &[4]))
        .object(TestHircObject::action(4, PLAY, 300))
        .object(TestHircObject::language_sound(300, 9000, 2, 0))
        .bytes()
}

fn with_audio(check: impl FnOnce(&ArchiveAudio<'_>) -> TestResult) -> TestResult {
    let definitions = TestAtl::new("SC_Audio")
        .trigger("Play_Long", &["Play_Long"])
        .trigger("Play_Line", &["Play_Line"])
        .localised_trigger("Play_Voice", "Play_DX_Hello")
        .localised_trigger("Play_Only", "Play_DX_Only");
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("Data.p4k");
    let loose = |name: &str, len: usize| TestEntry::stored(name, vec![0x7E; len]);
    ArchiveBuilder::new()
        .entry(TestEntry::zstd(
            "Data\\Libs\\GameAudio\\SC_Audio.xml",
            definitions.to_cryxml(),
        ))
        .entry(TestEntry::stored(
            "Data\\Sounds\\wwise\\Common.bnk",
            common_bank(),
        ))
        .entry(TestEntry::stored(
            "Data\\Sounds\\wwise\\English(US)\\Voice.bnk",
            voice_bank("Play_DX_Hello", 8000),
        ))
        .entry(TestEntry::stored(
            "Data\\Sounds\\wwise\\German\\Voice.bnk",
            voice_bank("Play_DX_Hello", 8000),
        ))
        .entry(TestEntry::stored(
            "Data\\Sounds\\wwise\\English(US)\\Only.bnk",
            voice_bank("Play_DX_Only", 8500),
        ))
        .entry(loose("Data\\Sounds\\wwise\\Media\\7000.wem", 33))
        .entry(loose("Data\\Sounds\\wwise\\7001.wem", 12))
        .entry(loose(
            "Data\\Sounds\\wwise\\Media\\English(US)\\8000.wem",
            21,
        ))
        .entry(loose("Data\\Sounds\\wwise\\Media\\German\\8000.wem", 22))
        .entry(loose(
            "Data\\Sounds\\wwise\\Media\\English(US)\\8500.wem",
            23,
        ))
        .entry(loose(
            "Data\\Sounds\\wwise\\Media\\English(US)\\9000.wem",
            41,
        ))
        .entry(loose("data\\sounds\\WWISE\\media\\german\\9000.WEM", 42))
        .entry(loose("Data\\Sounds\\wwise\\Media\\notanumber.wem", 1))
        .entry(loose("Data\\Sounds\\wwise\\Media\\a\\b\\7000.wem", 2))
        .entry(loose("Data\\Sounds\\other\\Media\\7002.wem", 3))
        .write_to(&path)?;
    let archive = Archive::open(&path)?;
    check(&ArchiveAudio::new(&archive))
}

fn resolved<'a>(
    audio: &'a ArchiveAudio<'_>,
    name: &str,
    choice: &LanguageChoice,
) -> Result<TriggerSounds<'a>, String> {
    audio
        .resolve_trigger_in(name, choice)
        .ok_or_else(|| format!("{name} resolves"))
}

fn entry_of<'a>(variant: &SoundVariant<'a>) -> Option<(&'a str, Option<&'a str>, u64)> {
    match variant.location()? {
        WemLocation::Entry {
            path,
            language,
            len,
            ..
        } => Some((path, *language, *len)),
        _ => None,
    }
}

fn german() -> LanguageChoice {
    LanguageChoice::new(Some("german"), Some("English(US)"))
}

#[test]
fn streamed_sources_are_located_as_loose_wem_entries() -> TestResult {
    with_audio(|audio| {
        let sounds = resolved(audio, "Play_Long", &LanguageChoice::any())?;
        assert!(sounds.breaks().is_empty(), "{:?}", sounds.breaks());
        let found: Vec<_> = sounds
            .variants()
            .iter()
            .map(|variant| (variant.wem_id(), variant.source_kind(), entry_of(variant)))
            .collect();
        assert_eq!(
            found,
            [
                (
                    Some(7000),
                    SourceKind::Streamed,
                    Some(("Data\\Sounds\\wwise\\Media\\7000.wem", None, 33))
                ),
                (
                    Some(7001),
                    SourceKind::PrefetchStreamed,
                    Some(("Data\\Sounds\\wwise\\7001.wem", None, 12))
                ),
                (Some(7002), SourceKind::Streamed, None),
            ]
        );
        assert!(
            sounds
                .variants()
                .iter()
                .all(|variant| !variant.is_localised())
        );
        Ok(())
    })
}

#[test]
fn a_prefetched_source_without_a_loose_entry_keeps_its_embedded_start() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::zstd(
            "Data\\Libs\\GameAudio\\SC_Audio.xml",
            TestAtl::new("SC")
                .trigger("Play_Long", &["Play_Long"])
                .to_cryxml(),
        ))
        .entry(TestEntry::stored(
            "Data\\Sounds\\wwise\\Common.bnk",
            common_bank(),
        ))
        .write_to(&path)?;
    let archive = Archive::open(&path)?;
    let audio = ArchiveAudio::new(&archive);
    let sounds = resolved(&audio, "Play_Long", &LanguageChoice::any())?;
    let prefetch = sounds
        .variants()
        .iter()
        .find(|variant| variant.wem_id() == Some(7001))
        .ok_or("the prefetched sound")?;
    assert!(matches!(
        prefetch.location(),
        Some(WemLocation::Soundbank { len: 8, .. })
    ));
    Ok(())
}

#[test]
fn the_languages_are_enumerated_from_the_archive_and_stray_files_are_ignored() -> TestResult {
    with_audio(|audio| {
        let index = audio.index();
        let languages: Vec<&str> = index.languages().iter().map(|name| &**name).collect();
        assert_eq!(languages, ["English(US)", "German"]);
        assert_eq!(index.default_language(), Some("English(US)"));
        assert_eq!(index.language("GERMAN"), Some("German"));
        assert_eq!(index.language("French"), None);
        let found: Vec<_> = index
            .loose_wem(7000)
            .map(|location| match location {
                WemLocation::Entry { path, .. } => path.to_owned(),
                _ => String::new(),
            })
            .collect();
        assert_eq!(found, ["Data\\Sounds\\wwise\\Media\\7000.wem"]);
        assert!(
            index.loose_wem(7002).next().is_none(),
            "other folders are not WEMs of the game"
        );
        Ok(())
    })
}

#[test]
fn a_localised_event_is_followed_in_the_chosen_language_only() -> TestResult {
    with_audio(|audio| {
        let sounds = resolved(audio, "Play_Voice", &german())?;
        assert!(sounds.breaks().is_empty(), "{:?}", sounds.breaks());
        let [variant] = sounds.variants() else {
            return Err("one variant".into());
        };
        assert!(variant.is_localised());
        assert_eq!(variant.language(), Some("German"));
        assert!(!variant.used_fallback());
        assert_eq!(
            entry_of(variant),
            Some((
                "Data\\Sounds\\wwise\\Media\\German\\8000.wem",
                Some("German"),
                22
            ))
        );
        assert_eq!(variant.soundbank().language(), Some("German"));
        Ok(())
    })
}

#[test]
fn without_a_language_every_language_is_followed() -> TestResult {
    with_audio(|audio| {
        let sounds = resolved(audio, "Play_Voice", &LanguageChoice::any())?;
        let languages: Vec<_> = sounds
            .variants()
            .iter()
            .map(SoundVariant::language)
            .collect();
        assert_eq!(languages, [Some("English(US)"), Some("German")]);
        Ok(())
    })
}

#[test]
fn the_fallback_language_is_used_where_the_chosen_one_has_no_localisation() -> TestResult {
    with_audio(|audio| {
        let sounds = resolved(audio, "Play_Only", &german())?;
        assert!(sounds.breaks().is_empty(), "{:?}", sounds.breaks());
        assert_ne!(sounds.variants().len(), 0);
        for variant in sounds.variants() {
            assert_eq!(variant.language(), Some("English(US)"));
            assert!(variant.used_fallback());
            assert_eq!(
                entry_of(variant).map(|(path, ..)| path),
                Some("Data\\Sounds\\wwise\\Media\\English(US)\\8500.wem")
            );
        }
        Ok(())
    })
}

#[test]
fn a_missing_localisation_is_reported() -> TestResult {
    with_audio(|audio| {
        let choice = LanguageChoice::new(Some("German"), None);
        let sounds = resolved(audio, "Play_Only", &choice)?;
        assert_eq!(sounds.variants().len(), 0);
        let [link] = sounds.breaks() else {
            return Err(format!("one break: {:?}", sounds.breaks()).into());
        };
        assert_eq!(
            link.reason(),
            &BreakReason::NoLocalisation {
                language: "German".into(),
                fallback: "".into()
            }
        );
        assert_eq!(
            link.reason().to_string(),
            "no localisation in the language German"
        );
        assert_eq!(link.reason().kind(), "no_localisation");
        assert!(link.soundbank().is_none());
        Ok(())
    })
}

#[test]
fn a_language_specific_streamed_sound_is_located_in_the_language_folder() -> TestResult {
    with_audio(|audio| {
        let sounds = resolved(audio, "Play_Line", &german())?;
        assert!(sounds.breaks().is_empty(), "{:?}", sounds.breaks());
        let [variant] = sounds.variants() else {
            return Err("one variant".into());
        };
        assert!(variant.is_localised());
        assert_eq!(variant.source_kind(), SourceKind::Streamed);
        assert_eq!(variant.language(), Some("German"));
        assert_eq!(
            entry_of(variant),
            Some((
                "data\\sounds\\WWISE\\media\\german\\9000.WEM",
                Some("German"),
                42
            ))
        );
        Ok(())
    })
}

#[test]
fn a_language_specific_sound_falls_back_and_reports_what_stays_missing() -> TestResult {
    with_audio(|audio| {
        let choice = LanguageChoice::new(Some("French"), Some("English(US)"));
        let sounds = resolved(audio, "Play_Line", &choice)?;
        let [variant] = sounds.variants() else {
            return Err("one variant".into());
        };
        assert_eq!(variant.language(), Some("English(US)"));
        assert!(variant.used_fallback());
        assert_eq!(sounds.breaks().len(), 0);

        let choice = LanguageChoice::new(Some("French"), Some("Spanish"));
        let sounds = resolved(audio, "Play_Line", &choice)?;
        let [variant] = sounds.variants() else {
            return Err("one variant".into());
        };
        assert!(variant.location().is_none());
        assert!(variant.is_localised());
        let [link] = sounds.breaks() else {
            return Err("one break".into());
        };
        assert_eq!(
            link.reason(),
            &BreakReason::NoLocalisation {
                language: "French".into(),
                fallback: "Spanish".into()
            }
        );
        assert_eq!(
            link.reason().to_string(),
            "no localisation in the language French, nor in the fallback Spanish"
        );
        assert_eq!(link.object().map(|step| step.kind.name()), Some("sound"));
        Ok(())
    })
}
