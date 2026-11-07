// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Streamed and localised sounds of a real Archive, resolved to loose WEM Entries.

mod canary;

use std::error::Error;

use p4kx_assembly::audio::{
    ArchiveAudio, DEFAULT_LANGUAGE, LanguageChoice, SoundVariant, SourceKind, TriggerSounds,
};
use p4kx_extraction::Archive;
use p4kx_testkit::real_archive_path;

type TestResult = Result<(), Box<dyn Error>>;

#[derive(Debug, Default)]
struct Tally {
    streamed: usize,
    streamed_located: usize,
    localised: usize,
    localised_located: usize,
    flagged: usize,
    flagged_in_language: usize,
    missing_localisation: usize,
}

impl Tally {
    fn add(&mut self, sounds: &TriggerSounds<'_>, language: &str) {
        self.missing_localisation += sounds
            .breaks()
            .iter()
            .filter(|link| link.reason().kind() == "no_localisation")
            .count();
        for variant in sounds.variants() {
            self.variant(variant, language);
        }
    }

    fn variant(&mut self, variant: &SoundVariant<'_>, language: &str) {
        if matches!(
            variant.source_kind(),
            SourceKind::Streamed | SourceKind::PrefetchStreamed
        ) {
            self.streamed += 1;
            self.streamed_located += usize::from(variant.location().is_some());
        }
        if variant.is_localised() {
            self.localised += 1;
            self.localised_located += usize::from(variant.location().is_some());
        }
        if variant.source().is_language_specific() {
            self.flagged += 1;
            self.flagged_in_language += usize::from(variant.language() == Some(language));
        }
    }
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn the_streamed_sounds_of_documented_ships_are_located_as_loose_entries() {
    crate::media_fixtures::run(canary::verify);
}
#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn the_localised_sounds_are_located_in_the_chosen_language() -> TestResult {
    let archive = Archive::open_with_key(
        real_archive_path()?,
        Some(p4kx_testkit::real_archive_key()?),
    )?;
    let audio = ArchiveAudio::new(&archive);
    let index = audio.index();
    let language = index
        .language(DEFAULT_LANGUAGE)
        .ok_or("the Archive holds the default language folder")?;
    let choice = LanguageChoice::new(Some(language), None);
    let mut tally = Tally::default();
    for trigger in index.triggers().iter().step_by(5) {
        let sounds = audio
            .resolve_trigger_in(trigger.name(), &choice)
            .ok_or("a trigger has a definition")?;
        tally.add(&sounds, language);
    }
    assert!(tally.localised > 1000, "{tally:?}");
    assert!(
        tally.localised_located * 100 >= tally.localised * 99,
        "{tally:?}"
    );
    assert_eq!(tally.streamed_located, tally.streamed, "{tally:?}");
    assert_eq!(
        tally.flagged_in_language, tally.flagged,
        "a language-specific source lies in the language folder: {tally:?}"
    );
    assert_eq!(tally.missing_localisation, 0, "{tally:?}");
    Ok(())
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn the_languages_are_enumerated_from_the_archive() -> TestResult {
    let archive = Archive::open_with_key(
        real_archive_path()?,
        Some(p4kx_testkit::real_archive_key()?),
    )?;
    let audio = ArchiveAudio::new(&archive);
    let index = audio.index();
    assert_ne!(index.languages().len(), 0);
    assert!(index.language(DEFAULT_LANGUAGE).is_some());
    Ok(())
}
