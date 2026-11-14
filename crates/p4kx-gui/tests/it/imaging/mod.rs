// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Helpers of the image preview tests: a synthetic Archive with split textures opened in a
//! headless window.

use crate::waiting::{RunQuiet, wait_for_model};
use std::error::Error;
use std::path::PathBuf;

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use p4kx_gui::{
    ArchiveKey, Browser, ImagePreview, ImageStatus, Language, MipStatus, Model, Preview, ui,
};
use p4kx_testkit::{ArchiveBuilder, SplitDds, TestEntry, TestTextureFormat};
use tempfile::TempDir;

pub(crate) type TestResult = Result<(), Box<dyn Error>>;

const PANEL: &str = "Data\\Textures\\panel_diff.dds";
const PROBE: &str = "Data\\Textures\\probe_cm.dds";
const ODD: &str = "Data\\Textures\\odd_spec.dds";
const LOST: &str = "Data\\Textures\\lost_ddna.dds";
const R8G8B8A8_SINT: TestTextureFormat = TestTextureFormat::Dx10Pixel {
    dxgi_format: 32,
    pixel_bytes: 4,
};

pub(crate) fn textures() -> Vec<TestEntry> {
    let mut entries = SplitDds::new(TestTextureFormat::LegacyBgra8, 1024, 1024)
        .split(4)
        .entries(PANEL);
    entries.extend(
        SplitDds::new(TestTextureFormat::LegacyBgra8, 16, 16)
            .cubemap()
            .split(1)
            .entries(PROBE),
    );
    entries.extend(SplitDds::new(R8G8B8A8_SINT, 8, 8).entries(ODD));
    let mut lost = SplitDds::new(TestTextureFormat::LegacyBgra8, 64, 64)
        .split(7)
        .entries(LOST);
    lost.truncate(1);
    entries.extend(lost);
    entries.push(TestEntry::stored("Data\\readme.txt", "plain text"));
    entries
}

pub(crate) fn archive(entries: Vec<TestEntry>) -> Result<(TempDir, PathBuf), Box<dyn Error>> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("Data.p4k");
    entries
        .into_iter()
        .fold(ArchiveBuilder::new(), ArchiveBuilder::entry)
        .write_to(&path)?;
    Ok((folder, path))
}

pub(crate) fn opened(
    language: Language,
) -> Result<(TempDir, Harness<'static, Model>), Box<dyn Error>> {
    opened_with(language, textures())
}

pub(crate) fn opened_with(
    language: Language,
    entries: Vec<TestEntry>,
) -> Result<(TempDir, Harness<'static, Model>), Box<dyn Error>> {
    opened_with_key(language, entries, None)
}

pub(crate) fn opened_with_key(
    language: Language,
    entries: Vec<TestEntry>,
    key: Option<ArchiveKey>,
) -> Result<(TempDir, Harness<'static, Model>), Box<dyn Error>> {
    let (folder, path) = archive(entries)?;
    let mut model = Model::new(language);
    model.set_archive_key(key);
    let mut harness = Harness::builder()
        .with_context_setup(p4kx_gui::theme::apply)
        .with_size([1600.0, 900.0])
        .build_ui_state(
            |ui, model: &mut Model| {
                ui::show(ui, model);
            },
            model,
        );
    harness.state_mut().open(path, || {});
    wait_until(&mut harness, |model| model.browser().is_some())?;
    Ok((folder, harness))
}

pub(crate) fn wait_until(
    harness: &mut Harness<'_, Model>,
    done: impl Fn(&Model) -> bool,
) -> TestResult {
    wait_for_model(harness, "the window to settle", done)?;
    harness.run_quiet();
    Ok(())
}

pub(crate) fn image(model: &Model) -> Option<&ImagePreview> {
    model
        .browser()
        .and_then(Browser::preview)
        .and_then(Preview::image)
}

pub(crate) fn settled(model: &Model) -> bool {
    image(model).is_some_and(|image| match image.status() {
        ImageStatus::Opened(info, MipStatus::Unchosen) => {
            !info.is_decodable() || info.first_showable_level(u32::MAX).is_none()
        }
        ImageStatus::Opening
        | ImageStatus::Opened(_, MipStatus::Decoding(_) | MipStatus::Decoded(_, None)) => false,
        _ => true,
    })
}

pub(crate) fn select(harness: &mut Harness<'_, Model>, name: &str) -> TestResult {
    harness.get_by_label(name).click();
    wait_until(harness, settled)
}

pub(crate) fn shown_level(harness: &Harness<'_, Model>) -> Option<u32> {
    match image(harness.state())?.status() {
        ImageStatus::Opened(_, MipStatus::Decoded(level, Some(_))) => Some(level),
        _ => None,
    }
}
