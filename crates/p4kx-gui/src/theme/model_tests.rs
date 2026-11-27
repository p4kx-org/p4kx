// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::Value;

use super::desktop::FACTS_PADDING_X;
use super::model_metrics as m;

const DESKTOP: &str = include_str!("../../../../docs/design/p4kx-desktop-spec.json");

fn model_preview() -> Value {
    let spec: Value =
        serde_json::from_str(DESKTOP).unwrap_or_else(|error| panic!("the desktop spec: {error}"));
    spec["modelPreview"].clone()
}

#[test]
fn the_model_preview_metrics_match_the_desktop_spec() {
    let spec = model_preview();

    for (key, value) in [
        ("viewMinHeight", m::MODEL_VIEW_MIN_HEIGHT),
        ("overlayGap", m::MODEL_OVERLAY_GAP),
        ("hintMinWidth", m::MODEL_HINT_MIN_WIDTH),
        ("animationListWidth", m::MODEL_ANIMATIONS_WIDTH),
        ("boneStroke", m::MODEL_BONE_STROKE),
        ("barHeight", m::MODEL_BAR_HEIGHT),
        ("viewHeight", m::MODEL_VIEW_HEIGHT),
        ("panelHeight", m::PREVIEW_MODEL_HEIGHT),
        ("panelMinHeight", m::PREVIEW_MODEL_MIN_HEIGHT),
        ("factsPaddingX", FACTS_PADDING_X),
    ] {
        let expected = spec[key].as_f64();
        assert!(
            expected.is_some_and(|want| (want - f64::from(value)).abs() < 0.01),
            "modelPreview.{key} is {expected:?}, the theme has {value}"
        );
    }
}

const _: () = {
    assert!(
        m::MODEL_VIEW_HEIGHT >= 1.5 * m::MODEL_VIEW_MIN_HEIGHT,
        "the view the panel opens with is at least half as high again as its minimum"
    );
    assert!(
        m::PREVIEW_MODEL_HEIGHT > super::desktop::PREVIEW_HEIGHT,
        "the panel of a model opens higher than the default"
    );
};

#[test]
fn the_model_view_sits_on_the_inset_surface_with_a_hairline() {
    let spec = model_preview();

    assert_eq!(
        spec["background"].as_str(),
        Some("surface-inset"),
        "surface"
    );
    assert_eq!(spec["border"].as_str(), Some("color-gray-3"), "hairline");
    assert_eq!(
        super::palette::SURFACE_INSET,
        super::palette::SURFACE_INPUT,
        "the preview target is cleared with the recessed bar colour"
    );
}

#[test]
fn the_model_preview_names_its_decision_and_controls() {
    let spec = model_preview();

    assert_eq!(spec["decision"].as_str(), Some("D.35"), "decision");
    assert_eq!(
        spec["playbackDecision"].as_str(),
        Some("D.46"),
        "playback decision"
    );
    for key in [
        "mouse",
        "keyboard",
        "resetButton",
        "hint",
        "messages",
        "playbackBar",
        "animationList",
        "skeletonOverlay",
    ] {
        assert!(
            spec[key].as_str().is_some_and(|text| !text.is_empty()),
            "modelPreview.{key} is described"
        );
    }
}
