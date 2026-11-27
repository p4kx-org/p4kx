// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use egui::{Event, Pos2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use p4kx_extraction::SourceLocation;
use p4kx_gui::{Language, Model, PreviewTab, SoundbankPreview, SoundbankStatus};
use p4kx_testkit::{TestBank, TestCodec, TestEntry, TestWem, test_key};

use crate::banking::{bank, select_sound};
use crate::sounding::{FakeOutput, TestResult, opened_with_key, wait_until};

const PATH: &str = "Data\\Sounds\\duplicate.bnk";
const WEM: u32 = 101;
const PREFETCH: u32 = 102;
const PLUGIN: u32 = 103;
const MALFORMED: u32 = 104;

fn banks() -> Result<Vec<TestEntry>, &'static str> {
    let plain = TestWem::pcm(TestCodec::Pcm, 1, 8_000, &[1; 8]).bytes();
    let encrypted = TestWem::pcm(TestCodec::Pcm, 1, 16_000, &[2; 16]).bytes();
    let prefetch = encrypted.get(..16).ok_or("RIFF prefix")?.to_vec();
    let first = TestBank::new(1).wem(WEM, plain).bytes();
    let second = TestBank::new(2)
        .wem(WEM, encrypted)
        .wem(PREFETCH, prefetch)
        .wem(PLUGIN, b"PLUG1234".to_vec())
        .wem(MALFORMED, b"bad".to_vec())
        .bytes();
    Ok(vec![
        TestEntry::stored(PATH, first),
        TestEntry::deflated(PATH, second).encrypted(),
    ])
}

fn select_bank(harness: &mut Harness<'_, Model>, encrypted: bool) -> TestResult {
    let browser = harness.state_mut().browser_mut().ok_or("browser")?;
    let id = (0..browser.index().len())
        .filter_map(|id| u32::try_from(id).ok())
        .find(|&id| {
            browser
                .index()
                .entry(id)
                .is_some_and(|entry| entry.path() == PATH && entry.is_encrypted() == encrypted)
        })
        .ok_or("original bank slot")?;
    browser.select_entry(id, || {});
    harness.state_mut().set_preview_tab(PreviewTab::Audio);
    wait_until(harness, |model| {
        bank(model).is_some_and(|bank| matches!(bank.status(), SoundbankStatus::Opened(_)))
    })
}

fn assert_summary(bank: &SoundbankPreview, encrypted: bool) -> TestResult {
    let summary = bank
        .summary_provenance()
        .ok_or("bank summary provenance")?
        .map_err(ToString::to_string)?;
    for metadata in [&summary.path, &summary.size] {
        assert_eq!(
            metadata.contains_encrypted_source(),
            Some(false),
            "directory fact"
        );
    }
    assert_eq!(
        summary.wem_count.contains_encrypted_source(),
        Some(encrypted),
        "count uses DIDX"
    );
    assert_eq!(summary.inventory().source_count(), 1, "one original bank");
    Ok(())
}

fn assert_rows(bank: &SoundbankPreview) -> TestResult {
    let row = bank
        .row_provenance(WEM)
        .ok_or("playable row")?
        .map_err(ToString::to_string)?;
    let duration_uses_riff_data = row.inventory().iter_uses().any(|used| {
        row.duration.source_uses_refs().contains(used.id())
            && matches!(
                used.location(),
                SourceLocation::Subresource { namespace, field, .. }
                    if namespace == "bnkWem.wemChunk" && field == "bodyLength"
            )
    });
    assert!(
        duration_uses_riff_data,
        "PCM duration comes from the selected RIFF"
    );
    for (id, is_complete) in [(PREFETCH, true), (PLUGIN, true), (MALFORMED, false)] {
        let row = bank
            .row_provenance(id)
            .ok_or("classified row")?
            .map_err(ToString::to_string)?;
        assert_eq!(
            row.codec.source_set_complete(),
            is_complete,
            "classification of WEM {id}"
        );
        assert!(
            row.identity.id.source_set_complete(),
            "the DIDX identity is always known"
        );
    }
    Ok(())
}

fn move_pointer(harness: &mut Harness<'_, Model>, point: Pos2) {
    harness.event(Event::PointerMoved(point));
    harness.run_steps(4);
}

fn assert_tooltips(harness: &mut Harness<'_, Model>) -> TestResult {
    harness.ctx.style_mut_of(harness.ctx.theme(), |style| {
        style.interaction.tooltip_delay = 0.0;
        style.interaction.show_tooltips_only_when_still = false;
    });
    let preview = p4kx_gui::ui::regions(&harness.ctx)
        .preview
        .ok_or("preview region")?;
    let path = harness
        .query_all_by_label(PATH)
        .find(|node| preview.contains(node.rect().center()))
        .ok_or("bank path line")?
        .rect();
    move_pointer(harness, path.center());
    assert_eq!(
        harness
            .query_all_by_label("Unencrypted source data")
            .count(),
        2,
        "path and size"
    );
    assert!(
        harness.query_by_label("Embedded WEM count").is_some(),
        "count is a separate fact"
    );

    let row = harness.get_by_label_contains("WEM 101, PCM").rect();
    move_pointer(harness, row.center());
    assert_eq!(
        harness
            .query_all_by_label("Includes encrypted source data")
            .count(),
        4,
        "row fields"
    );
    Ok(())
}

#[test]
fn direct_bank_and_embedded_wem_keep_the_selected_original_source() -> TestResult {
    let (folder, mut harness) = opened_with_key(
        Language::BritishEnglish,
        &FakeOutput::default(),
        banks()?,
        Some(test_key()),
    )?;
    for encrypted in [false, true] {
        select_bank(&mut harness, encrypted)?;
        assert_summary(bank(harness.state()).ok_or("bank preview")?, encrypted)?;
    }

    assert_rows(bank(harness.state()).ok_or("encrypted bank")?)?;
    assert_tooltips(&mut harness)?;
    assert_selected_wem(&mut harness)?;

    std::fs::OpenOptions::new()
        .write(true)
        .open(folder.path().join("Data.p4k"))?
        .set_len(0)?;
    assert!(
        harness.state_mut().check_archive(),
        "archive change observed"
    );
    let changed = bank(harness.state()).ok_or("invalidated bank")?;
    assert!(
        matches!(changed.status(), SoundbankStatus::ArchiveChanged),
        "the stale bank is replaced by the archive-change state"
    );
    assert!(
        changed.summary_provenance().is_none(),
        "stale summary hidden"
    );
    assert!(changed.row_provenance(WEM).is_none(), "stale row hidden");
    assert!(changed.audio().is_none(), "stale audio hidden");
    Ok(())
}

fn assert_selected_wem(harness: &mut Harness<'_, Model>) -> TestResult {
    select_sound(harness, WEM)?;
    let opened = bank(harness.state()).ok_or("selected bank")?;
    let audio = opened.audio().ok_or("selected audio")?;
    let facts = audio
        .header_provenance()
        .ok_or("selected WEM facts")?
        .map_err(ToString::to_string)?;
    for field in [
        &facts.codec,
        &facts.channels,
        &facts.sample_rate,
        &facts.duration,
    ] {
        assert_eq!(
            field.contains_encrypted_source(),
            Some(true),
            "the original encrypted bank"
        );
    }
    assert_eq!(
        audio.player().ok_or("PCM")?.pcm().frames(),
        16,
        "the second physical WEM"
    );
    let fact_row = harness
        .get_by_label_contains("Codec PCM Channels 1 Sample rate 16,000 Hz")
        .rect();
    move_pointer(harness, fact_row.center());
    assert_eq!(
        harness
            .query_all_by_label("Includes encrypted source data")
            .count(),
        4,
        "selected facts"
    );
    Ok(())
}
