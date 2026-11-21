// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::io::Read as _;

use p4kx_assembly::audio::{
    ArchiveAudio, AudioIndex, LanguageChoice, SoundVariant, WemLocation, WemSelection,
};
use p4kx_audio::Soundbank;
use p4kx_extraction::Archive;
use p4kx_testkit::{ArchiveBuilder, TestAtl, TestBank, TestEntry, TestHircObject, wwise_short_id};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

const ATL: &str = "Data/Libs/GameAudio/Selection.xml";
const VOICE: &str = "Data/Sounds/wwise/German/Voice.bnk";
const OTHER_GERMAN: &str = "Data/Sounds/wwise/german/Other.bnk";
const CZECH: &str = "Data/Sounds/wwise/Czech/Other.bnk";
const NEUTRAL: &str = "Data/Sounds/wwise/Neutral.bnk";
const COMMON: &str = "Data/Sounds/wwise/Common.bnk";
const GERMAN_LOOSE: &str = "Data/Sounds/wwise/Media/German/100.wem";
const ENGLISH_LOOSE: &str = "Data/Sounds/wwise/Media/English(US)/200.wem";
const NEUTRAL_LOOSE: &str = "Data/Sounds/wwise/Media/300.wem";
const CHOICE_GERMAN: &str = "Data/Sounds/wwise/Media/German/900.wem";
const CHOICE_ENGLISH: &str = "Data/Sounds/wwise/Media/English(US)/900.wem";
const PLAY: u16 = 0x0403;

#[test]
fn localisation_receipt_distinguishes_the_actual_loose_and_embedded_selection_rules() -> TestResult
{
    let (_folder, archive) = open_fixture()?;
    let audio = ArchiveAudio::new(&archive);
    let choice = LanguageChoice::new(Some("German"), Some("English(US)"));
    let sounds = audio
        .resolve_trigger_in("Play_Selection", &choice)
        .ok_or("original selection trigger")?;
    assert_eq!(
        sounds.language_choice(),
        &choice,
        "the actual supplied policy"
    );
    assert_original_definition(&archive, &audio, &sounds)?;
    for (variant, expected) in sounds.variants().iter().zip([
        (
            100,
            WemSelection::LooseBankLanguage,
            Some(GERMAN_LOOSE),
            false,
        ),
        (
            200,
            WemSelection::LooseFallbackLanguage,
            Some(ENGLISH_LOOSE),
            true,
        ),
        (300, WemSelection::LooseNeutral, Some(NEUTRAL_LOOSE), false),
        (400, WemSelection::NotFound, None, false),
        (500, WemSelection::EmbeddedFirstAvailable, Some(CZECH), true),
        (600, WemSelection::EmbeddedSameBank, Some(VOICE), false),
        (
            700,
            WemSelection::EmbeddedBankLanguage,
            Some(OTHER_GERMAN),
            false,
        ),
        (800, WemSelection::EmbeddedNeutral, Some(NEUTRAL), false),
    ]) {
        let (id, rule, path, legacy_fallback) = expected;
        assert_eq!(variant.wem_id(), Some(id), "original event action order");
        assert_eq!(
            variant.wem_selection(),
            rule,
            "actual admission for media {id}"
        );
        assert_eq!(
            variant.used_fallback(),
            legacy_fallback,
            "the established fallback flag remains stable for media {id}"
        );
        assert_eq!(
            variant.soundbank().entry_position(),
            physical_position(&archive, VOICE)?,
            "the Sound comes from its own exact physical bank"
        );
        assert_location(&archive, variant, path)?;
    }
    assert_eq!(sounds.variants().len(), 8, "every authored action survives");
    Ok(())
}

#[test]
fn a_neutral_bank_records_explicit_preference_or_first_localised_choice() -> TestResult {
    let (_folder, archive) = open_fixture()?;
    let audio = ArchiveAudio::new(&archive);
    for (choice, rule, path) in [
        (
            LanguageChoice::new(Some("German"), Some("English(US)")),
            WemSelection::LoosePreferredLanguage,
            CHOICE_GERMAN,
        ),
        (
            LanguageChoice::any(),
            WemSelection::LooseFirstLocalised,
            CHOICE_ENGLISH,
        ),
    ] {
        let sounds = audio
            .resolve_trigger_in("Play_Common", &choice)
            .ok_or("original neutral-bank trigger")?;
        let [variant] = sounds.variants() else {
            return Err("one original neutral-bank Sound".into());
        };
        assert_eq!(
            variant.wem_selection(),
            rule,
            "actual explicit or archive-order choice"
        );
        assert_location(&archive, variant, Some(path))?;
    }
    Ok(())
}

fn assert_original_definition(
    archive: &Archive,
    audio: &ArchiveAudio<'_>,
    sounds: &p4kx_assembly::audio::TriggerSounds<'_>,
) -> TestResult {
    let definition = sounds
        .definition_entry_position()
        .ok_or("ATL Entry position")?;
    assert_eq!(
        physical_position(archive, ATL)?,
        definition,
        "the chosen ATL points to the first original occurrence"
    );
    let index = audio.index();
    let foreign = AudioIndex::build(archive);
    let foreign_trigger = foreign.trigger("Play_Selection").ok_or("foreign trigger")?;
    assert!(
        index.definition_entry_position(foreign_trigger).is_none(),
        "an equal trigger from another index has no physical receipt here"
    );
    Ok(())
}

fn assert_location(
    archive: &Archive,
    variant: &SoundVariant<'_>,
    expected: Option<&str>,
) -> TestResult {
    let Some(expected) = expected else {
        assert!(
            variant.location().is_none(),
            "there is no original location"
        );
        return Ok(());
    };
    let position = physical_position(archive, expected)?;
    match variant.location().ok_or("actual original WEM location")? {
        WemLocation::Entry { entry, len, .. } => {
            let actual = archive
                .entries()
                .get(position)
                .ok_or("original loose WEM")?;
            assert_eq!(usize::try_from(*entry)?, position, "exact loose WEM Entry");
            assert_eq!(
                *len,
                actual.uncompressed_size(),
                "original loose WEM length"
            );
        }
        WemLocation::Soundbank {
            soundbank,
            offset,
            len,
        } => {
            assert_eq!(
                soundbank.entry_position(),
                position,
                "exact embedded WEM Entry"
            );
            let bytes = original_bytes(archive, position)?;
            let raw = Soundbank::parse(bytes.as_slice())?;
            let wem = raw
                .wem(variant.wem_id().ok_or("original media ID")?)
                .ok_or("original embedded WEM")?;
            assert_eq!(
                *offset,
                wem.offset(),
                "actual original DATA-relative offset"
            );
            assert_eq!(
                *len,
                u64::try_from(wem.bytes().len())?,
                "actual original WEM length"
            );
        }
        _ => return Err("unexpected physical media location".into()),
    }
    Ok(())
}

fn physical_position(archive: &Archive, path: &str) -> TestResult<usize> {
    archive
        .entries()
        .iter()
        .position(|entry| entry.path().replace('\\', "/") == path)
        .ok_or_else(|| format!("the original Entry {path}").into())
}

fn original_bytes(archive: &Archive, position: usize) -> TestResult<Vec<u8>> {
    let entry = archive
        .entries()
        .get(position)
        .ok_or("original Entry position")?;
    let mut bytes = Vec::new();
    archive.open_entry(entry)?.read_to_end(&mut bytes)?;
    Ok(bytes)
}

fn voice() -> Vec<u8> {
    let action_ids: Vec<u32> = (1..=8).collect();
    let mut bank = TestBank::new(10)
        .wem(600, vec![0x60; 6])
        .object(TestHircObject::event(
            wwise_short_id("Play_DX_Selection"),
            &action_ids,
        ));
    for (action, media) in action_ids.into_iter().zip((100..=800).step_by(100)) {
        let sound = 1000 + action;
        let storage = u8::from(action <= 4) * 2;
        bank = bank
            .object(TestHircObject::action(action, PLAY, sound))
            .object(TestHircObject::sound(sound, media, storage, 0));
    }
    bank.bytes()
}

fn open_fixture() -> TestResult<(tempfile::TempDir, Archive)> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("Data.p4k");
    let atl = TestAtl::new("SC")
        .localised_trigger("Play_Selection", "Play_DX_Selection")
        .trigger("Play_Common", &["Play_Common"])
        .to_cryxml();
    let common = TestBank::new(20)
        .object(TestHircObject::event(wwise_short_id("Play_Common"), &[1]))
        .object(TestHircObject::action(1, PLAY, 2))
        .object(TestHircObject::language_sound(2, 900, 2, 0))
        .bytes();
    let bank = |id, wem, len| TestBank::new(id).wem(wem, vec![0x31; len]).bytes();
    ArchiveBuilder::new()
        .entry(TestEntry::stored(ATL, atl))
        .entry(TestEntry::stored(
            ATL,
            TestAtl::new("Later")
                .trigger("Play_Selection", &["Ignored"])
                .to_cryxml(),
        ))
        .entry(TestEntry::stored(VOICE, voice()))
        .entry(TestEntry::stored(VOICE, bank(99, 600, 14)))
        .entry(TestEntry::stored(OTHER_GERMAN, bank(11, 700, 7)))
        .entry(TestEntry::stored(CZECH, bank(12, 500, 5)))
        .entry(TestEntry::stored(NEUTRAL, bank(13, 800, 8)))
        .entry(TestEntry::stored(COMMON, common))
        .entry(TestEntry::stored(GERMAN_LOOSE, vec![1; 5]))
        .entry(TestEntry::stored(GERMAN_LOOSE, vec![2; 15]))
        .entry(TestEntry::stored(ENGLISH_LOOSE, vec![3; 10]))
        .entry(TestEntry::stored(NEUTRAL_LOOSE, vec![4; 11]))
        .entry(TestEntry::stored(CHOICE_GERMAN, vec![5; 13]))
        .entry(TestEntry::stored(CHOICE_ENGLISH, vec![6; 14]))
        .write_to(&path)?;
    Ok((folder, Archive::open(&path)?))
}
