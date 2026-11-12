// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Completion in the Query editor over the DataForge of a real Archive; runs with `--ignored`
//! when `P4KX_DATA_P4K` names one.
//!
//! The time limits assume a release build.

use std::error::Error;
use std::time::{Duration, Instant};

use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use p4kx_gui::{Language, Model, WorkTab, ui};
use p4kx_testkit::real_archive_path;

type TestResult = Result<(), Box<dyn Error>>;

const WAIT: Duration = Duration::from_mins(2);
const FRAME_LIMIT: Duration = Duration::from_millis(50);

fn wait_until(harness: &mut Harness<'_, Model>, done: impl Fn(&Model) -> bool) -> TestResult {
    let deadline = Instant::now() + WAIT;
    harness.step();
    while !done(harness.state()) {
        if Instant::now() > deadline {
            return Err("the window did not settle in time".into());
        }
        std::thread::sleep(Duration::from_millis(10));
        harness.step();
    }
    harness.run_steps(3);
    Ok(())
}

fn shown(model: &Model) -> (Vec<String>, usize) {
    model
        .query()
        .completion_shown()
        .map_or_default(|(completion, _)| {
            let names = completion
                .candidates()
                .iter()
                .map(|candidate| candidate.name().to_owned())
                .collect();
            (names, completion.more())
        })
}

fn names(model: &Model) -> Vec<String> {
    shown(model).0
}

/// Opens the real Archive on the Query tab with the focus in the editor and its schema read.
fn opened_on_query() -> Result<Harness<'static, Model>, Box<dyn Error>> {
    let path = real_archive_path()?;
    let mut harness = Harness::builder()
        .with_context_setup(p4kx_gui::theme::apply)
        .with_size([1400.0, 900.0])
        .build_ui_state(
            |ui, model: &mut Model| {
                let _requests = ui::show(ui, model);
            },
            Model::new(Language::BritishEnglish),
        );
    harness
        .state_mut()
        .set_archive_key(Some(p4kx_testkit::real_archive_key()?));
    harness.state_mut().open(path, || {});
    wait_until(&mut harness, |model| model.browser().is_some())?;
    harness.state_mut().set_work_tab(WorkTab::Query);
    harness.run_steps(2);
    harness.get_by_role(Role::MultilineTextInput).click();
    harness.run_steps(2);
    harness
        .get_by_role(Role::MultilineTextInput)
        .type_text("from ");
    wait_until(&mut harness, |model| !names(model).is_empty())?;
    harness.state_mut().query_mut().set_text("");
    harness.run_steps(2);
    Ok(harness)
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn components_of_a_ship_are_completed_from_the_real_schema() -> TestResult {
    let mut harness = opened_on_query()?;
    harness
        .get_by_role(Role::MultilineTextInput)
        .type_text("from EntityClassDefinition select Components[VehicleComp");
    wait_until(&mut harness, |model| !names(model).is_empty())?;

    assert!(
        names(harness.state()).contains(&"VehicleComponentParams".to_owned()),
        "component selector: {:?}",
        names(harness.state())
    );

    harness
        .get_by_role(Role::MultilineTextInput)
        .type_text("onentParams].");
    let typed = Instant::now();
    harness.run_steps(2);
    let keystroke = typed.elapsed() / 2;
    assert!(
        names(harness.state()).contains(&"vehicleName".to_owned()),
        "fields behind it: {:?}",
        names(harness.state())
    );
    assert!(keystroke < FRAME_LIMIT, "a frame took {keystroke:?}");
    Ok(())
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn completion_without_from_keeps_up_with_typing() -> TestResult {
    let mut harness = opened_on_query()?;
    for (text, expected) in [
        ("at \"*\" select ", "Components"),
        ("at \"*\" select Components[", "VehicleComponentParams"),
    ] {
        harness.state_mut().query_mut().set_text("");
        harness.run_steps(2);
        harness
            .get_by_role(Role::MultilineTextInput)
            .type_text(text);
        let typed = Instant::now();
        harness.step();
        let frame = typed.elapsed();
        harness.run_steps(2);
        let (names, more) = shown(harness.state());

        assert!(!names.is_empty(), "{text}: names offered");
        assert!(
            names.contains(&expected.to_owned()) || more > 0,
            "{text}: {expected} listed or more to type"
        );
        assert!(frame < FRAME_LIMIT, "{text}: a frame took {frame:?}");
    }
    Ok(())
}
