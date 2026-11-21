// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Helpers of the Characters tab tests: a synthetic Archive with the bodies, pieces, Loadouts and
//! a head rig of the testkit and a CHF file, opened in a headless window.

use std::error::Error;
use std::path::{Path, PathBuf};

use egui_kittest::Harness;
use p4kx_gui::ui::{self, Dialog};
use p4kx_gui::{ChooserKind, Language, Model, WorkTab};
use p4kx_testkit::{
    HEAD_REGIONS, HEAD_VERTICES, MALE_BODY_TYPE, MALE_RIG_HASH, TestChf, TestEntry, head_entries,
    head_rig,
};
use tempfile::TempDir;

use crate::tagging::archive;
use crate::waiting::wait_for;

pub(crate) type TestResult = Result<(), Box<dyn Error>>;

pub(crate) const ARCHIVE_CHF: &str = "Data/Libs/CharacterCustomizer/PU/Test/Runner.chf";
pub(crate) const GRUNT: &str = "Scripts/Loadouts/Grunt.xml";

pub(crate) struct State {
    pub(crate) model: Model,
    pub(crate) dialog: Option<Dialog>,
}

pub(crate) type Window = (TempDir, Harness<'static, State>);

pub(crate) fn chf_bytes() -> Vec<u8> {
    let blend = |part: usize| match part {
        3 => vec![(65535, 2), (0, 0)],
        _ => vec![(0, 0), (0, 0)],
    };
    TestChf {
        body_type: MALE_BODY_TYPE,
        dna_hashes: Some((MALE_RIG_HASH, 0)),
        face_parts: (0..HEAD_REGIONS).map(blend).collect(),
        ..TestChf::v8()
    }
    .to_bytes()
}

pub(crate) fn entries() -> Vec<TestEntry> {
    let mut entries = head_entries(Some(head_rig(HEAD_VERTICES, MALE_RIG_HASH).build()));
    entries.push(TestEntry::stored(
        ARCHIVE_CHF.replace('/', "\\"),
        chf_bytes(),
    ));
    entries
}

pub(crate) fn harness_sized(model: Model, size: [f32; 2]) -> Harness<'static, State> {
    Harness::builder()
        .with_context_setup(p4kx_gui::theme::apply)
        .with_size(size)
        .build_ui_state(
            |ui, state: &mut State| {
                let requests = ui::show(ui, &mut state.model);
                if requests.dialog.is_some() {
                    state.dialog = requests.dialog;
                }
            },
            State {
                model,
                dialog: None,
            },
        )
}

pub(crate) fn wait_until(
    harness: &mut Harness<'_, State>,
    done: impl Fn(&Model) -> bool,
) -> TestResult {
    wait_for(harness, "the window to settle", |harness| {
        done(&harness.state().model)
    })?;
    harness.run_steps(3);
    Ok(())
}

pub(crate) fn built(model: &Model) -> bool {
    let tab = model.characters();
    tab.scene().is_some() && !tab.is_building()
}

pub(crate) fn listed(model: &Model) -> bool {
    built(model) && model.characters().sources().is_some() && model.entities().catalogue().is_some()
}

/// Opens the Archive of `entries` in a window of `size` on the Characters tab and waits until the
/// bare body is built and the Archive is listed.
pub(crate) fn opened_sized(
    entries: Vec<TestEntry>,
    language: Language,
    size: [f32; 2],
) -> Result<Window, Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let path = archive(dir.path(), entries)?;
    opened_from(dir, path, language, size)
}

/// Opens the Archive at `path` like [`opened_sized`].
pub(crate) fn opened_from(
    dir: TempDir,
    path: PathBuf,
    language: Language,
    size: [f32; 2],
) -> Result<Window, Box<dyn Error>> {
    let mut harness = harness_sized(Model::new(language), size);
    harness.state_mut().model.open(path, || {});
    wait_until(&mut harness, |model| model.browser().is_some())?;
    harness.state_mut().model.set_work_tab(WorkTab::Characters);
    harness.run_steps(2);
    wait_until(&mut harness, listed)?;
    Ok((dir, harness))
}

pub(crate) fn opened(language: Language) -> Result<Window, Box<dyn Error>> {
    opened_sized(entries(), language, [1400.0, 900.0])
}

/// Opens the Archive in the channel folder `StarCitizen/LIVE` of a Star Citizen installation and
/// returns that folder too.
pub(crate) fn opened_in_installation(
    language: Language,
) -> Result<(Window, PathBuf), Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let channel = dir.path().join("StarCitizen").join("LIVE");
    std::fs::create_dir_all(channel.join("Bin64"))?;
    let path = archive(&channel, entries())?;
    let window = opened_from(dir, path, language, [1400.0, 900.0])?;
    Ok((window, channel))
}

/// Waits until the Character is built again after a change.
pub(crate) fn settle(harness: &mut Harness<'_, State>) -> TestResult {
    harness.run_steps(2);
    wait_until(harness, built)
}

/// Chooses the shown candidate called `name` of the open choice of `kind`.
pub(crate) fn pick(harness: &mut Harness<'_, State>, kind: ChooserKind, name: &str) -> TestResult {
    let model = &mut harness.state_mut().model;
    if !model.begin_character_choice(kind) {
        return Err("the choice did not open".into());
    }
    let at = model
        .characters()
        .chooser()
        .and_then(|chooser| {
            (0..chooser.shown_len())
                .find(|at| chooser.get(*at).is_some_and(|item| item.name() == name))
        })
        .ok_or("the candidate is not listed")?;
    model.characters_mut().choose(at);
    Ok(())
}

pub(crate) fn part_names(model: &Model) -> Vec<String> {
    model.characters().armour().map_or_default(|armour| {
        armour
            .parts()
            .iter()
            .map(|part| part.name().to_owned())
            .collect()
    })
}

/// Writes the CHF file of the test head to `dir`.
pub(crate) fn chf_file(dir: &Path, name: &str) -> Result<PathBuf, Box<dyn Error>> {
    let path = dir.join(name);
    std::fs::write(&path, chf_bytes())?;
    Ok(path)
}
