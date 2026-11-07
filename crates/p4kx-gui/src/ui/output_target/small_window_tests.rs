// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};

use super::test_fixture::Window;
use crate::dialog::Dialog;
use crate::theme::desktop::BUTTON_HEIGHT;
use crate::widgets::placement;
use crate::{Language, OutputTargetText as T};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn the_target_dialog_scrolls_at_640_by_400_and_keeps_its_actions_visible() -> TestResult {
    for language in [Language::BritishEnglish, Language::German] {
        for dialog in [
            Dialog::ModelFile,
            Dialog::AssemblyPackage,
            Dialog::DiagnosticCatalogFile,
        ] {
            let mut harness = Harness::builder()
                .with_context_setup(crate::theme::apply)
                .with_size([640.0, 400.0])
                .build_ui_state(
                    |ui, window: &mut Window| window.show(ui),
                    Window::new(language, dialog, true),
                );
            harness.run_steps(3);
            assert!(
                crate::ui::regions(&harness.ctx).scrolls,
                "long body has bounded scrolling"
            );
            let confirm = if dialog.is_setup_output() {
                T::Confirm
            } else {
                T::StartExport
            };
            for text in [confirm, T::Cancel] {
                let label = harness.state().texts.get(text);
                let node = harness.get_by_role_and_label(Role::Button, &label);
                let rect = node
                    .accesskit_node()
                    .bounding_box()
                    .ok_or("footer bounds missing")?;
                let viewport = placement::dialog_area(&harness.ctx);
                assert!(
                    rect.x1 > rect.x0 && rect.y1 - rect.y0 >= f64::from(BUTTON_HEIGHT),
                    "{label}: a full button remains visible"
                );
                assert!(
                    rect.x0 >= f64::from(viewport.left())
                        && rect.x1 <= f64::from(viewport.right())
                        && rect.y0 >= f64::from(viewport.top())
                        && rect.y1 <= f64::from(viewport.bottom()),
                    "{label}: footer stays inside the 640x400 viewport"
                );
                if text == confirm {
                    assert!(node.accesskit_node().is_disabled(), "invalid name");
                }
            }
        }
    }
    Ok(())
}

#[test]
fn scrolling_reveals_the_complete_destination_without_moving_the_actions() -> TestResult {
    let mut harness = Harness::builder()
        .with_context_setup(crate::theme::apply)
        .with_size([640.0, 400.0])
        .build_ui_state(
            |ui, window: &mut Window| window.show(ui),
            Window::new(Language::German, Dialog::AssemblyPackage, false),
        );
    harness.run_steps(3);
    let cancel = harness.state().texts.get(T::Cancel);
    let before = harness
        .get_by_role_and_label(Role::Button, &cancel)
        .accesskit_node()
        .bounding_box()
        .ok_or("action bounds missing")?;
    harness.hover_at(egui::pos2(300.0, 200.0));
    harness.event(egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: egui::vec2(0.0, -600.0),
        phase: egui::TouchPhase::Move,
        modifiers: egui::Modifiers::NONE,
        source: egui::MouseWheelSource::Wheel,
    });
    harness.run_steps(3);
    let path = harness
        .state()
        .target
        .pending()
        .and_then(crate::output_target::Selection::path)
        .ok_or("selected output missing")?;
    let destination = harness
        .get_by_label(&path.display().to_string())
        .accesskit_node()
        .bounding_box()
        .ok_or("destination bounds missing")?;
    let after = harness
        .get_by_role_and_label(Role::Button, &cancel)
        .accesskit_node()
        .bounding_box()
        .ok_or("action bounds missing after scrolling")?;
    assert_eq!(before, after, "scrolling keeps the footer in place");
    let viewport = placement::dialog_area(&harness.ctx);
    assert!(
        destination.y0 >= f64::from(viewport.top()) && destination.y1 <= after.y0,
        "the full wrapped destination is above the visible actions"
    );
    Ok(())
}
