// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use egui::Event;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use p4kx_extraction::{FieldOrigin, SourceLocation};
use p4kx_gui::{AudioStatus, Language, Model, PreviewTab, SoundStatus, WemHeaderProvenance};
use p4kx_testkit::{TestCodec, TestEntry, TestWem, test_key};

use crate::sounding::{FakeOutput, TestResult, audio, opened_with_key, wait_until};

const PATH: &str = "Data\\Sounds\\duplicate.wem";

fn select(harness: &mut Harness<'_, Model>, encrypted: bool) -> TestResult {
    let browser = harness.state_mut().browser_mut().ok_or("browser")?;
    let physical = (0..browser.index().len())
        .filter_map(|id| u32::try_from(id).ok())
        .find(|&id| {
            browser
                .index()
                .entry(id)
                .is_some_and(|entry| entry.path() == PATH && entry.is_encrypted() == encrypted)
        })
        .ok_or("original WEM slot")?;
    browser.select_entry(physical, || {});
    harness.state_mut().set_preview_tab(PreviewTab::Audio);
    wait_until(harness, |model| {
        audio(model).is_some_and(|audio| !audio.is_busy())
    })
}

const fn fields(origins: &WemHeaderProvenance) -> [&FieldOrigin; 4] {
    [
        &origins.codec,
        &origins.channels,
        &origins.sample_rate,
        &origins.duration,
    ]
}

fn assert_duration_size_input(origins: &WemHeaderProvenance) {
    let used_length = |origin: &FieldOrigin| {
        origins.inventory().iter_uses().any(|used| {
            origin.source_uses_refs().contains(used.id())
                && matches!(
                    used.location(),
                    SourceLocation::Subresource { namespace, field, .. }
                        if namespace == "wemChunk" && field == "bodyLength"
                )
        })
    };
    assert!(
        used_length(&origins.duration),
        "PCM duration uses the data chunk size"
    );
    assert!(!used_length(&origins.channels), "channels come from fmt");
}

fn assert_facts_tooltip(harness: &mut Harness<'_, Model>) {
    let rect = harness
        .get_by_label_contains("Codec PCM Channels 1 Sample rate 16,000 Hz")
        .rect();
    harness.ctx.style_mut_of(harness.ctx.theme(), |style| {
        style.interaction.tooltip_delay = 0.0;
        style.interaction.show_tooltips_only_when_still = false;
    });
    harness.event(Event::PointerMoved(rect.center()));
    harness.run_steps(4);
    assert_eq!(
        harness
            .query_all_by_label("Includes encrypted source data")
            .count(),
        4,
        "all four facts show their provenance"
    );
}

#[test]
fn direct_wem_facts_follow_the_selected_physical_duplicate() -> TestResult {
    let first = TestWem::pcm(TestCodec::Pcm, 1, 8_000, &[1; 8]).bytes();
    let second = TestWem::pcm(TestCodec::Pcm, 1, 16_000, &[2; 16]).bytes();
    let (folder, mut harness) = opened_with_key(
        Language::BritishEnglish,
        &FakeOutput::default(),
        vec![
            TestEntry::stored(PATH, first),
            TestEntry::deflated(PATH, second).encrypted(),
        ],
        Some(test_key()),
    )?;

    for (encrypted, rate, frames) in [(false, 8_000, 8), (true, 16_000, 16)] {
        select(&mut harness, encrypted)?;
        let preview = audio(harness.state()).ok_or("audio preview")?;
        let AudioStatus::Opened(info, SoundStatus::Ready(player)) = preview.status() else {
            return Err("the selected original WEM did not decode".into());
        };
        assert_eq!(info.sample_rate, rate, "rate of the selected Entry");
        assert_eq!(player.pcm().frames(), frames, "audio of the selected Entry");
        let origins = preview
            .header_provenance()
            .ok_or("WEM origins")?
            .map_err(ToString::to_string)?;
        assert_eq!(origins.inventory().source_count(), 1, "one physical WEM");
        for field in fields(origins) {
            assert!(field.source_set_complete(), "known physical inputs");
            assert_eq!(
                field.contains_encrypted_source(),
                Some(encrypted),
                "field origin"
            );
        }
        assert_duration_size_input(origins);
    }

    assert_facts_tooltip(&mut harness);
    std::fs::OpenOptions::new()
        .write(true)
        .open(folder.path().join("Data.p4k"))?
        .set_len(0)?;
    assert!(harness.state_mut().check_archive(), "change observed");
    let preview = audio(harness.state()).ok_or("invalidated preview")?;
    assert!(
        matches!(preview.status(), AudioStatus::ArchiveChanged),
        "stale preview hidden"
    );
    assert!(
        preview.header_provenance().is_none(),
        "stale origins hidden"
    );
    Ok(())
}
