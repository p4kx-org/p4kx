// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use egui::Event;
use egui_kittest::kittest::Queryable;
use p4kx_extraction::{FieldOrigin, SourceEncryptionState, SourceLocation};
use p4kx_gui::{
    Browser, ImageMipProvenance, ImagePreview, ImageStatus, Language, MipStatus, PreviewTab,
};
use p4kx_testkit::{SplitDds, TestEntry, TestTextureFormat, test_key};

use crate::imaging::{
    TestResult, image, opened_with_key, select, settled, shown_level, wait_until,
};
use crate::waiting::RunQuiet;

const PATH: &str = "Data\\probe.dds";

fn encrypted_alpha() -> Vec<TestEntry> {
    SplitDds::new(TestTextureFormat::LegacyBgra8, 8, 8)
        .split(2)
        .alpha(TestTextureFormat::R8)
        .entries(PATH)
        .into_iter()
        .map(|entry| {
            if matches!(entry.path(), "Data\\probe.dds.1" | "Data\\probe.dds.2a") {
                entry.encrypted()
            } else {
                entry
            }
        })
        .collect()
}

fn marker(origin: &FieldOrigin, expected: Option<bool>) {
    assert_eq!(
        origin.contains_encrypted_source(),
        expected,
        "field encryption"
    );
    assert!(origin.source_set_complete(), "the actual inputs are known");
}

fn assert_selected_alpha_span(mip: &ImageMipProvenance) {
    let encrypted_ranges: Vec<&[u64]> = mip
        .inventory()
        .uses()
        .iter()
        .filter(|used| used.effective_encryption() == SourceEncryptionState::Encrypted)
        .filter(|used| mip.pixels.source_uses_refs().contains(used.id()))
        .filter_map(|used| match used.location() {
            SourceLocation::Subresource {
                namespace, indices, ..
            } if namespace == "ddsMipPayload" => Some(indices.as_slice()),
            _ => None,
        })
        .collect();
    assert_eq!(
        encrypted_ranges,
        [&[0, 0, 64, 0, 1][..]],
        "selected alpha part only"
    );
}

#[test]
fn a_mip_keeps_only_its_contributors_after_upload_and_replaces_them_when_switched() -> TestResult {
    let (_folder, mut harness) = opened_with_key(
        Language::BritishEnglish,
        encrypted_alpha(),
        Some(test_key()),
    )?;
    select(&mut harness, "probe.dds")?;
    assert_eq!(shown_level(&harness), Some(0), "largest mip after upload");
    let opened = image(harness.state()).ok_or("image preview")?;
    let header = opened.header_provenance().ok_or("header origins")?;
    let mip = opened
        .mip_provenance()
        .ok_or("rendered origins after upload")?;
    marker(&header.format, Some(false));
    marker(&header.width, Some(false));
    marker(&header.mip_label, Some(false));
    marker(&mip.width, Some(false));
    marker(&mip.pixels, Some(true));

    assert_selected_alpha_span(mip);
    assert_eq!(header.inventory().sources().len(), 1, "one header");
    assert_eq!(
        mip.inventory().sources().len(),
        4,
        "the two headers and two selected siblings"
    );

    let pending = harness
        .state_mut()
        .browser_mut()
        .and_then(Browser::image_mut)
        .ok_or("pending preview")?;
    pending.request(1, u32::MAX);
    pending.request(2, u32::MAX);
    assert!(
        pending.mip_provenance().is_none(),
        "a pending mip has no old origin"
    );
    assert!(
        pending.header_provenance().is_some(),
        "the opened header remains current"
    );
    wait_until(&mut harness, |model| {
        settled(model) && image(model).and_then(ImagePreview::level) == Some(2)
    })?;
    let opened = image(harness.state()).ok_or("image preview after mip change")?;
    let mip = opened
        .mip_provenance()
        .ok_or("embedded mip origins after upload")?;
    marker(&mip.pixels, Some(false));
    assert_eq!(
        mip.inventory().sources().len(),
        2,
        "the two embedded payloads"
    );
    Ok(())
}

#[test]
fn a_known_archive_change_discards_already_rendered_origins_and_pixels() -> TestResult {
    let (folder, mut harness) = opened_with_key(
        Language::BritishEnglish,
        encrypted_alpha(),
        Some(test_key()),
    )?;
    select(&mut harness, "probe.dds")?;
    assert!(
        image(harness.state())
            .and_then(ImagePreview::mip_provenance)
            .is_some(),
        "the mip has origins"
    );
    std::fs::OpenOptions::new()
        .write(true)
        .open(folder.path().join("Data.p4k"))?
        .set_len(0)?;
    assert!(
        harness.state_mut().check_archive(),
        "source change observed"
    );
    let changed = image(harness.state()).ok_or("invalidated preview")?;
    assert!(
        matches!(changed.status(), ImageStatus::ArchiveChanged),
        "stale pixels are hidden"
    );
    assert!(
        changed.header_provenance().is_none(),
        "stale header origins are hidden"
    );
    assert!(
        changed.mip_provenance().is_none(),
        "stale mip origins are hidden"
    );
    harness.run_quiet();
    assert_eq!(
        shown_level(&harness),
        None,
        "an old worker result did not revive the mip"
    );
    Ok(())
}

#[test]
fn the_selected_duplicate_owns_the_header_and_the_pixels() -> TestResult {
    let first = SplitDds::new(TestTextureFormat::R8, 4, 4).header_bytes();
    let second = SplitDds::new(TestTextureFormat::R8, 8, 8).header_bytes();
    let (_folder, mut harness) = opened_with_key(
        Language::BritishEnglish,
        vec![
            TestEntry::stored(PATH, first),
            TestEntry::stored(PATH, second).encrypted(),
        ],
        Some(test_key()),
    )?;
    let browser = harness.state_mut().browser_mut().ok_or("browser")?;
    let physical = (0..browser.index().len())
        .filter_map(|id| u32::try_from(id).ok())
        .find(|&id| {
            browser
                .index()
                .entry(id)
                .is_some_and(|entry| entry.index() == 1)
        })
        .ok_or("second original slot")?;
    browser.select_entry(physical, || {});
    harness.state_mut().set_preview_tab(PreviewTab::Image);
    wait_until(&mut harness, settled)?;
    let selected = image(harness.state()).ok_or("duplicate preview")?;
    let ImageStatus::Opened(info, MipStatus::Decoded(_, Some(_))) = selected.status() else {
        return Err("the selected original did not decode".into());
    };
    assert_eq!(info.width(), 8, "the second physical header");
    let header = selected.header_provenance().ok_or("header")?;
    marker(&header.path, Some(false));
    marker(&header.width, Some(true));
    marker(&selected.mip_provenance().ok_or("mip")?.pixels, Some(true));
    Ok(())
}

#[test]
fn an_unsupported_texture_retains_header_provenance_without_inventing_pixels() -> TestResult {
    let (_folder, mut harness) = opened_with_key(
        Language::BritishEnglish,
        SplitDds::new(
            TestTextureFormat::Dx10Pixel {
                dxgi_format: 32,
                pixel_bytes: 4,
            },
            4,
            4,
        )
        .entries(PATH),
        None,
    )?;
    select(&mut harness, "probe.dds")?;
    let opened = image(harness.state()).ok_or("unsupported preview")?;
    let ImageStatus::Opened(info, MipStatus::Unchosen) = opened.status() else {
        return Err("the header is expected to open without a supported mip".into());
    };
    assert!(!info.is_decodable(), "unsupported decoder");
    let header = opened
        .header_provenance()
        .ok_or("unsupported header origin")?;
    marker(&header.decodable, Some(false));
    assert!(opened.mip_provenance().is_none(), "no rendered pixels");
    Ok(())
}

#[test]
fn the_image_inspector_shows_the_rendered_origin_without_relabelling_header_facts() -> TestResult {
    let (_folder, mut harness) = opened_with_key(
        Language::BritishEnglish,
        encrypted_alpha(),
        Some(test_key()),
    )?;
    select(&mut harness, "probe.dds")?;
    let picture = harness.get_by_label_contains("Mip 0: 8").rect();
    harness.ctx.style_mut_of(harness.ctx.theme(), |style| {
        style.interaction.tooltip_delay = 0.0;
        style.interaction.show_tooltips_only_when_still = false;
    });
    harness.event(Event::PointerMoved(picture.center()));
    harness.run_steps(4);
    assert!(
        harness
            .query_by_label("Includes encrypted source data")
            .is_some(),
        "the rendered pixels' tooltip"
    );
    assert!(
        harness
            .query_by_label_contains("Format B8G8R8A8_UNORM Size")
            .is_some(),
        "the cached header facts keep their original label"
    );
    Ok(())
}
