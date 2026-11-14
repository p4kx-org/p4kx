// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use egui_kittest::wgpu::WgpuTestRenderer;
use p4kx_gui::{Language, PrerequisiteNotice, theme};

use crate::rendering;

type TestResult = Result<(), Box<dyn Error>>;

struct State {
    notice: PrerequisiteNotice,
    closed: bool,
}

fn draw(ui: &mut egui::Ui, state: &mut State) {
    state.closed |= state.notice.show(ui);
}

#[test]
fn a_prerequisite_notice_is_localised_and_can_be_closed_at_the_minimum_window_size() {
    for (language, close) in [
        (Language::BritishEnglish, "Close"),
        (Language::German, "Schließen"),
    ] {
        let mut harness = Harness::builder()
            .with_context_setup(theme::apply)
            .with_size([640.0, 400.0])
            .build_ui_state(
                draw,
                State {
                    notice: PrerequisiteNotice::new(language),
                    closed: false,
                },
            );
        harness.run_steps(3);
        assert!(
            harness.query_by_label_contains("API 5").is_some(),
            "the missing prerequisite is visible"
        );
        assert!(
            !harness.state().closed,
            "the notice remains open for the user"
        );
        harness.get_by_label(close).click();
        harness.run_steps(2);
        assert!(
            harness.state().closed,
            "the explicit close action works in {language:?}"
        );
    }
}

#[test]
fn prerequisite_notices_render_in_both_languages_at_minimum_and_default_sizes() -> TestResult {
    for (language, suffix) in [
        (Language::BritishEnglish, "en-GB"),
        (Language::German, "de-DE"),
    ] {
        for (size, name) in [([640.0, 400.0], "minimum"), ([1280.0, 800.0], "default")] {
            let Some(state) = rendering::render_state() else {
                return Ok(());
            };
            let mut harness = Harness::builder()
                .with_context_setup(theme::apply)
                .with_size(size)
                .renderer(WgpuTestRenderer::from_render_state(state))
                .build_ui_state(
                    draw,
                    State {
                        notice: PrerequisiteNotice::new(language),
                        closed: false,
                    },
                );
            harness.run_steps(3);
            let image = harness.render()?;
            rendering::save(
                &format!("prerequisite-{suffix}-{name}.png"),
                image.width(),
                image.height(),
                &image.into_raw(),
            )?;
            assert!(
                harness.query_by_label_contains("API 5").is_some(),
                "the rendered notice contains the prerequisite"
            );
        }
    }
    Ok(())
}
