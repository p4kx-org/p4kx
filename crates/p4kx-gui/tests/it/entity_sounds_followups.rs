// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The language choice, the soundbank kept for playing and the note about a missing player of the
//! Sounds tab.

use std::error::Error;

use egui::accesskit::{Role, Toggled};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use p4kx_extraction::{SourceLocation, SourceUseRole};
use p4kx_gui::{
    AudioPreview, Language, ListStatus, Model, SoundFormat, SoundsExport, SoundsExportStatus,
    SoundsListStatus,
};

use crate::entity_sounds::{TestResult, opened, wait_until};
use crate::sounding::FakeOutput;

const SIZE: [f32; 2] = [1400.0, 800.0];
const BANK: &str = "Data\\Sounds\\wwise\\Test.bnk";

fn resolved(harness: &mut Harness<'_, Model>, query: &str) -> TestResult {
    harness.state_mut().sounds_mut().set_query(query);
    wait_until(harness, |model| {
        matches!(model.sounds().list_status(), ListStatus::Ready(_))
    })?;
    harness.state_mut().sounds_mut().select(0);
    wait_until(harness, |model| {
        matches!(model.sounds().sounds_status(), SoundsListStatus::Ready(_))
    })
}

fn languages(harness: &Harness<'_, Model>) -> Vec<(Option<String>, bool)> {
    match harness.state().sounds().sounds_status() {
        SoundsListStatus::Ready(list) => list
            .variants
            .iter()
            .map(|variant| (variant.language.clone(), variant.fallback))
            .collect(),
        _ => Vec::new(),
    }
}

fn choose(harness: &mut Harness<'_, Model>, label: &str) -> TestResult {
    let before = harness.state().sounds().sounds_serial();
    harness.get_by_role_and_label(Role::Button, label).click();
    harness.run_steps(2);
    wait_until(harness, |model| {
        model.sounds().sounds_serial() > before
            && matches!(model.sounds().sounds_status(), SoundsListStatus::Ready(_))
    })
}

#[test]
fn the_language_buttons_resolve_the_selected_trigger_in_the_chosen_language() -> TestResult {
    let (_dir, mut harness) = opened(Language::BritishEnglish, SIZE, &FakeOutput::default())?;
    resolved(&mut harness, "play_voice")?;
    harness.state_mut().select_sound_variant(0, || {});

    assert_eq!(
        languages(&harness),
        [(Some("English(US)".to_owned()), false)],
        "the default language"
    );
    assert_eq!(harness.state().sounds().language(), None, "none chosen");

    choose(&mut harness, "German")?;

    assert_eq!(harness.state().sounds().language(), Some("German"));
    assert_eq!(
        languages(&harness),
        [(Some("German".to_owned()), false)],
        "resolved in German"
    );
    assert_eq!(
        harness.state().sounds().selected_variant(),
        None,
        "the playing sound of the other language is dropped"
    );

    choose(&mut harness, "English(US)")?;

    assert_eq!(harness.state().sounds().language(), None, "default again");
    assert_eq!(
        languages(&harness),
        [(Some("English(US)".to_owned()), false)],
        "back in the default language"
    );
    Ok(())
}

fn exported_sizes(harness: &mut Harness<'_, Model>, folder: &std::path::Path) -> Vec<u64> {
    assert!(harness.state_mut().begin_sounds_export(), "setup opens");
    let Some(setup) = harness.state_mut().sounds_mut().export_setup_mut() else {
        return Vec::new();
    };
    setup.set_folder(folder.to_path_buf());
    setup.set_format(SoundFormat::Wav);
    assert!(harness.state_mut().submit_sounds_export(), "starts");
    let _ = wait_until(harness, |model| {
        model
            .sounds()
            .export()
            .is_some_and(|export| !export.is_running())
    });
    let sizes = match harness.state().sounds().export().map(SoundsExport::status) {
        Some(SoundsExportStatus::Finished { report, .. }) => {
            report.sounds.iter().map(|sound| sound.size).collect()
        }
        _ => Vec::new(),
    };
    harness.state_mut().sounds_mut().close_export();
    sizes
}

#[test]
fn the_export_uses_the_chosen_language() -> TestResult {
    let (dir, mut harness) = opened(Language::BritishEnglish, SIZE, &FakeOutput::default())?;
    resolved(&mut harness, "play_voice")?;

    let english = exported_sizes(&mut harness, &dir.path().join("english"));
    choose(&mut harness, "German")?;
    let german = exported_sizes(&mut harness, &dir.path().join("german"));

    assert_eq!(english.len(), 1, "one sound in English: {english:?}");
    assert_eq!(german.len(), 1, "one sound in German: {german:?}");
    assert!(
        german[0] > english[0],
        "the longer German line is written: {german:?} against {english:?}"
    );
    Ok(())
}

#[test]
fn the_chosen_language_is_exposed_as_selected() -> TestResult {
    let (_dir, mut harness) = opened(Language::BritishEnglish, SIZE, &FakeOutput::default())?;
    harness.run_steps(2);

    let selected = |harness: &Harness<'_, Model>, label: &str| {
        harness
            .get_by_role_and_label(Role::Button, label)
            .accesskit_node()
            .toggled()
    };

    assert_eq!(
        selected(&harness, "English(US)"),
        Some(Toggled::True),
        "the default"
    );
    assert_eq!(
        selected(&harness, "German"),
        Some(Toggled::False),
        "the other"
    );
    Ok(())
}

#[test]
fn a_language_that_has_no_sound_falls_back_and_says_so() -> TestResult {
    let (_dir, mut harness) = opened(Language::BritishEnglish, SIZE, &FakeOutput::default())?;
    resolved(&mut harness, "play_voice")?;

    harness
        .state_mut()
        .sounds_mut()
        .set_language(Some("Klingon"));
    wait_until(&mut harness, |model| {
        matches!(model.sounds().sounds_status(), SoundsListStatus::Ready(_))
    })?;

    assert_eq!(
        languages(&harness),
        [(Some("English(US)".to_owned()), true)],
        "the default language stands in"
    );
    Ok(())
}

#[test]
fn the_soundbank_of_a_played_sound_is_kept_for_the_next_one() -> TestResult {
    let (_dir, mut harness) = opened(Language::BritishEnglish, SIZE, &FakeOutput::default())?;
    resolved(&mut harness, "play_multi")?;
    let id = harness
        .state()
        .browser()
        .and_then(|browser| browser.index().id_of_path(BANK))
        .ok_or("no soundbank Entry")?;
    assert_eq!(
        harness.state().sounds().kept_soundbank(),
        None,
        "nothing yet"
    );

    let first = play_first_embedded(&mut harness)?;
    assert_eq!(
        harness.state().sounds().kept_soundbank(),
        Some(id),
        "kept after the first embedded sound"
    );
    resolved(&mut harness, "play_single")?;
    let second = play_first_embedded(&mut harness)?;
    assert_eq!(
        harness.state().sounds().kept_soundbank(),
        Some(id),
        "same Bank"
    );
    let triplet = u64::try_from(3 * size_of::<u32>())?;
    assert_eq!(
        second.checked_sub(first),
        Some(triplet),
        "the next WEM's original DIDX triplet"
    );
    Ok(())
}

fn play_first_embedded(harness: &mut Harness<'_, Model>) -> Result<u64, Box<dyn Error>> {
    harness.state_mut().select_sound_variant(0, || {});
    wait_until(harness, |model| {
        model
            .sounds()
            .audio()
            .is_some_and(|audio| audio.player().is_some())
    })?;
    let proof = harness
        .state()
        .sounds()
        .audio()
        .and_then(AudioPreview::header_provenance)
        .ok_or("resolved embedded WEM header proof")?
        .map_err(ToString::to_string)?;
    for field in [
        &proof.codec,
        &proof.channels,
        &proof.sample_rate,
        &proof.duration,
    ] {
        assert_eq!(
            field.contains_encrypted_source(),
            Some(false),
            "ordinary plain Bank origin"
        );
        assert!(
            field.source_set_complete(),
            "the selected metadata inputs are bound"
        );
    }
    proof
        .inventory()
        .uses()
        .iter()
        .filter(|used| {
            proof.codec.source_uses_refs().contains(used.id())
                && used.role() == SourceUseRole::Selector
        })
        .find_map(|used| match used.location() {
            SourceLocation::Subresource {
                namespace,
                field,
                indices,
            } if namespace == "bnkDidx" && field == "mediaId" => indices.first().copied(),
            _ => None,
        })
        .ok_or_else(|| "the playback header is missing its original DIDX selection".into())
}
