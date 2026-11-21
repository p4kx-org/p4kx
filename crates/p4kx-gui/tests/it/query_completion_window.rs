// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Completion in the Query editor, driven headless over a synthetic Archive with a DataForge.

use crate::querying;
use crate::waiting::wait_for_model;

use std::error::Error;
use std::time::Duration;

use egui::accesskit::Role;
use egui::{Key, Modifiers};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use p4kx_gui::{Language, Model, WorkTab, ui};
use p4kx_testkit::DataForgeBuilder;
use querying::{TestResult, archive, database, referencing};
use tempfile::TempDir;

fn opened(forge: &DataForgeBuilder) -> Result<(TempDir, Harness<'static, Model>), Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let path = archive(dir.path(), forge)?;
    let mut harness = Harness::builder()
        .with_context_setup(p4kx_gui::theme::apply)
        .with_size([1400.0, 900.0])
        .build_ui_state(
            |ui, model: &mut Model| {
                let _requests = ui::show(ui, model);
            },
            Model::new(Language::BritishEnglish),
        );
    harness.state_mut().open(path, || {});
    wait_until(&mut harness, |model| model.browser().is_some())?;
    harness.state_mut().set_work_tab(WorkTab::Query);
    harness.run_steps(2);
    Ok((dir, harness))
}

fn wait_until(harness: &mut Harness<'_, Model>, done: impl Fn(&Model) -> bool) -> TestResult {
    wait_for_model(harness, "the window to settle", done)?;
    harness.run_steps(3);
    Ok(())
}

fn shown(model: &Model) -> Vec<String> {
    model
        .query()
        .completion_shown()
        .map_or_default(|(completion, _)| {
            completion
                .candidates()
                .iter()
                .map(|candidate| candidate.name().to_owned())
                .collect()
        })
}

fn type_text(harness: &mut Harness<'_, Model>, text: &str) {
    harness.get_by_role(Role::MultilineTextInput).click();
    harness.run_steps(2);
    harness
        .get_by_role(Role::MultilineTextInput)
        .type_text(text);
    harness.run_steps(2);
}

#[test]
fn typing_after_select_lists_the_fields_of_the_from_type() -> TestResult {
    let (_dir, mut harness) = opened(&database())?;

    type_text(&mut harness, "from Ship select ");
    wait_until(&mut harness, |model| !shown(model).is_empty())?;

    assert_eq!(
        shown(harness.state()),
        ["crew", "mass", "name"],
        "inherited too"
    );
    for name in ["crew", "mass", "name"] {
        assert!(
            harness
                .query_by_role_and_label(Role::ListBoxOption, name)
                .is_some(),
            "option {name}"
        );
    }
    Ok(())
}

#[test]
fn the_arrow_keys_choose_and_enter_writes_the_name() -> TestResult {
    let (_dir, mut harness) = opened(&database())?;
    type_text(&mut harness, "from Ship select ");
    wait_until(&mut harness, |model| !shown(model).is_empty())?;

    harness.key_press(Key::ArrowDown);
    harness.run_steps(2);
    harness.key_press(Key::Enter);
    harness.run_steps(3);

    assert_eq!(
        harness.state().query().text(),
        "from Ship select mass",
        "written, no line break"
    );
    assert!(
        harness.state().query().completion_shown().is_none(),
        "closed"
    );
    Ok(())
}

#[test]
fn a_type_name_is_completed_by_a_click_and_typing_goes_on() -> TestResult {
    let (_dir, mut harness) = opened(&database())?;
    type_text(&mut harness, "from Sh");
    wait_until(&mut harness, |model| !shown(model).is_empty())?;

    assert_eq!(
        shown(harness.state()),
        ["Ship"],
        "filtered by the typed word"
    );
    harness
        .get_by_role_and_label(Role::ListBoxOption, "Ship")
        .click();
    harness.run_steps(3);
    harness
        .get_by_role(Role::MultilineTextInput)
        .type_text(" select");
    harness.run_steps(2);

    assert_eq!(
        harness.state().query().text(),
        "from Ship select",
        "replaced the word and kept the focus"
    );
    Ok(())
}

#[test]
fn escape_closes_the_list_and_ctrl_space_opens_it_again() -> TestResult {
    let (_dir, mut harness) = opened(&database())?;
    type_text(&mut harness, "from ");
    wait_until(&mut harness, |model| !shown(model).is_empty())?;

    harness.key_press(Key::Escape);
    harness.run_steps(2);
    assert!(shown(harness.state()).is_empty(), "closed");
    assert_eq!(harness.state().query().text(), "from ", "text kept");

    harness.key_press_modifiers(Modifiers::COMMAND, Key::Space);
    harness.run_steps(3);
    assert_eq!(shown(harness.state()), ["Item", "Ship"], "open again");
    Ok(())
}

#[test]
fn an_arrow_lists_the_fields_of_the_linked_struct() -> TestResult {
    let (_dir, mut harness) = opened(&referencing())?;

    type_text(&mut harness, "from Ship select maker->");
    wait_until(&mut harness, |model| !shown(model).is_empty())?;

    assert_eq!(shown(harness.state()), ["name"], "Maker fields");
    Ok(())
}

#[test]
fn brackets_list_the_element_struct_and_its_subtypes() -> TestResult {
    let forge = DataForgeBuilder::new()
        .structure(
            p4kx_testkit::TestStruct::new("Part").field("mass", p4kx_testkit::TestType::Single),
        )
        .structure(
            p4kx_testkit::TestStruct::new("Engine")
                .parent("Part")
                .field("thrust", p4kx_testkit::TestType::Single),
        )
        .structure(p4kx_testkit::TestStruct::new("Ship").array(
            "Components",
            p4kx_testkit::TestType::StrongPointer("Part".to_owned()),
        ));
    let (_dir, mut harness) = opened(&forge)?;

    type_text(&mut harness, "from Ship select Components[");
    wait_until(&mut harness, |model| !shown(model).is_empty())?;

    assert_eq!(shown(harness.state()), ["Engine", "Part"], "subtypes");
    Ok(())
}

#[test]
fn nothing_is_listed_where_no_name_fits() -> TestResult {
    let (_dir, mut harness) = opened(&database())?;
    type_text(&mut harness, "from ");
    wait_until(&mut harness, |model| !shown(model).is_empty())?;

    harness
        .get_by_role(Role::MultilineTextInput)
        .type_text("Ship at \"lib");
    harness.run_steps(3);

    assert!(shown(harness.state()).is_empty(), "inside a string");
    Ok(())
}

#[test]
fn enter_breaks_the_line_when_the_chosen_name_is_written_already() -> TestResult {
    let (_dir, mut harness) = opened(&database())?;
    type_text(&mut harness, "from Ship select ");
    wait_until(&mut harness, |model| !shown(model).is_empty())?;
    harness
        .get_by_role(Role::MultilineTextInput)
        .type_text("mass");
    harness.run_steps(3);
    assert_eq!(shown(harness.state()), ["mass"], "still offered");

    harness.key_press(Key::Enter);
    harness.run_steps(3);

    assert_eq!(
        harness.state().query().text(),
        "from Ship select mass\n",
        "a line break, not the name again"
    );
    Ok(())
}

fn editor_has_focus(harness: &Harness<'_, Model>) -> bool {
    harness
        .get_by_role(Role::MultilineTextInput)
        .accesskit_node()
        .is_focused()
}

fn announced(harness: &Harness<'_, Model>) -> (Option<bool>, Option<egui::accesskit::NodeId>) {
    let editor = harness
        .get_by_role(Role::MultilineTextInput)
        .accesskit_node();
    let data = editor.data();
    (data.is_expanded(), data.active_descendant())
}

fn option_id(harness: &Harness<'_, Model>, name: &str) -> egui::accesskit::NodeId {
    let full = u128::from(
        harness
            .get_by_role_and_label(Role::ListBoxOption, name)
            .accesskit_node()
            .id(),
    );
    egui::accesskit::NodeId(u64::try_from(full >> 64).unwrap_or_default())
}

#[test]
fn the_editor_points_screen_readers_at_the_active_option() -> TestResult {
    let (_dir, mut harness) = opened(&database())?;
    type_text(&mut harness, "from Ship select ");
    wait_until(&mut harness, |model| !shown(model).is_empty())?;
    assert_eq!(
        announced(&harness),
        (Some(true), Some(option_id(&harness, "crew"))),
        "open, first option"
    );

    harness.key_press(Key::ArrowDown);
    harness.run_steps(3);
    assert_eq!(
        announced(&harness),
        (Some(true), Some(option_id(&harness, "mass"))),
        "moved"
    );

    harness.key_press(Key::Escape);
    harness.run_steps(3);
    assert_eq!(announced(&harness), (Some(false), None), "closed");
    Ok(())
}

#[test]
fn tab_writes_the_chosen_name() -> TestResult {
    let (_dir, mut harness) = opened(&database())?;
    type_text(&mut harness, "from Ship select ");
    wait_until(&mut harness, |model| !shown(model).is_empty())?;

    harness.key_press(Key::Tab);
    harness.run_steps(3);

    assert_eq!(
        harness.state().query().text(),
        "from Ship select crew",
        "the first name, no tab written"
    );
    assert!(shown(harness.state()).is_empty(), "closed");
    assert!(editor_has_focus(&harness), "the focus stays");
    Ok(())
}

#[test]
fn tab_leaves_the_editor_while_the_list_is_closed() -> TestResult {
    let (_dir, mut harness) = opened(&database())?;
    type_text(&mut harness, "from Ship select ");
    wait_until(&mut harness, |model| !shown(model).is_empty())?;
    harness.key_press(Key::Escape);
    harness.run_steps(2);
    assert!(shown(harness.state()).is_empty(), "closed");

    harness.key_press(Key::Tab);
    harness.run_steps(3);

    assert_eq!(
        harness.state().query().text(),
        "from Ship select ",
        "nothing written"
    );
    assert!(!editor_has_focus(&harness), "the focus moved on");
    Ok(())
}

#[test]
fn the_list_does_not_open_by_itself_when_the_names_arrive() -> TestResult {
    let (_dir, mut harness) = opened(&database())?;
    harness
        .state_mut()
        .query_mut()
        .set_text("from Ship select ");
    harness.run_steps(2);

    harness.get_by_role(Role::MultilineTextInput).focus();
    wait_until(&mut harness, |model| {
        model
            .browser()
            .is_some_and(|browser| browser.query_schema_builds() == 1)
    })?;
    std::thread::sleep(Duration::from_millis(300));
    harness.run_steps(4);
    assert!(editor_has_focus(&harness), "focused");
    assert!(shown(harness.state()).is_empty(), "no list without typing");

    harness.key_press(Key::Tab);
    harness.run_steps(3);

    assert_eq!(
        harness.state().query().text(),
        "from Ship select ",
        "no name written"
    );
    assert!(!editor_has_focus(&harness), "the focus moved on");
    Ok(())
}

#[test]
fn typing_before_the_names_arrive_opens_the_list_when_they_do() -> TestResult {
    let (_dir, mut harness) = opened(&database())?;

    type_text(&mut harness, "from Ship select ");
    wait_until(&mut harness, |model| !shown(model).is_empty())?;

    assert_eq!(shown(harness.state()), ["crew", "mass", "name"], "opened");
    Ok(())
}
