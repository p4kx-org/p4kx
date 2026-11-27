// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use egui::Key;
use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};

use super::test_fixture::Window;
use crate::dialog::Dialog;
use crate::{Language, OutputTargetText as T};

fn harness(dialog: Dialog, invalid: bool) -> Harness<'static, Window> {
    let mut harness = Harness::builder()
        .with_context_setup(crate::theme::apply)
        .with_size([1280.0, 800.0])
        .build_ui_state(
            |ui, window: &mut Window| window.show(ui),
            Window::new(Language::BritishEnglish, dialog, invalid),
        );
    harness.run_steps(3);
    harness
}

#[test]
fn the_parent_button_requests_the_same_purpose_without_confirming_output() {
    let mut harness = harness(Dialog::AssemblyPackage, false);
    let button = harness.state().texts.get(T::ChooseParent);
    harness.get_by_role_and_label(Role::Button, &button).click();
    harness.run_steps(3);
    assert_eq!(
        harness.state().parent_request,
        Some(Dialog::AssemblyPackage)
    );
    assert_eq!(harness.state().parent_requests, 1);
    assert!(harness.state().target.pending().is_some());
    let scope = harness.state().scope();
    assert!(
        harness
            .state_mut()
            .target
            .take_confirmed(Some(&scope))
            .is_none()
    );
}

#[test]
fn escape_cancels_a_selection_without_dispatching() {
    let mut harness = harness(Dialog::DiagnosticCatalogFile, false);
    harness.key_press(Key::Escape);
    harness.run_steps(3);
    assert!(harness.state().target.pending().is_none());
    let scope = harness.state().scope();
    assert!(
        harness
            .state_mut()
            .target
            .take_confirmed(Some(&scope))
            .is_none()
    );
}

#[test]
fn enter_in_the_name_field_confirms_one_exact_route() {
    let mut harness = harness(Dialog::ModelFile, false);
    let name = harness.state().name_label();
    harness
        .get_by_role_and_label(Role::TextInput, &name)
        .focus();
    harness.run_steps(2);
    harness.key_press(Key::Enter);
    harness.run_steps(3);
    assert!(harness.state().target.pending().is_none());
    let scope = harness.state().scope();
    let choice = harness
        .state_mut()
        .target
        .take_confirmed(Some(&scope))
        .expect("one output choice");
    assert_eq!(choice.dialog, Dialog::ModelFile);
    assert_eq!(
        choice.path.file_name().and_then(|name| name.to_str()),
        Some("Prüfung Ä.glb")
    );
    assert!(
        harness
            .state_mut()
            .target
            .take_confirmed(Some(&scope))
            .is_none()
    );
}

#[test]
fn an_invalid_component_disables_confirmation_and_enter_cannot_dispatch() {
    let mut harness = harness(Dialog::ModelFile, true);
    let button = harness.state().texts.get(T::Confirm);
    assert!(
        harness
            .get_by_role_and_label(Role::Button, &button)
            .accesskit_node()
            .is_disabled()
    );
    let name = harness.state().name_label();
    harness
        .get_by_role_and_label(Role::TextInput, &name)
        .focus();
    harness.run_steps(2);
    harness.key_press(Key::Enter);
    harness.run_steps(3);
    assert!(harness.state().target.pending().is_some());
    let scope = harness.state().scope();
    assert!(
        harness
            .state_mut()
            .target
            .take_confirmed(Some(&scope))
            .is_none()
    );
}

#[test]
fn cancel_keeps_the_pending_path_uncommitted() {
    let mut harness = harness(Dialog::CharacterPackage, false);
    let expected = harness.state().target.pending().expect("open").path();
    assert!(expected.is_some(), "ready selection before cancellation");
    let cancel = harness.state().texts.get(T::Cancel);
    harness.get_by_role_and_label(Role::Button, &cancel).click();
    harness.run_steps(3);
    assert!(harness.state().target.pending().is_none());
    let scope = harness.state().scope();
    assert!(
        harness
            .state_mut()
            .target
            .take_confirmed(Some(&scope))
            .is_none()
    );
}

#[test]
fn later_frames_do_not_steal_focus_from_the_parent_button() {
    let mut harness = harness(Dialog::ModelFile, false);
    let parent = harness.state().texts.get(T::ChooseParent);
    harness.get_by_role_and_label(Role::Button, &parent).focus();
    harness.run_steps(3);
    assert!(
        harness
            .get_by_role_and_label(Role::Button, &parent)
            .accesskit_node()
            .is_focused()
    );
}
