// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::UiTab;
use super::render::RenderKey;
use super::settings::UiFormat;
use crate::TextLanguage;

/// The exact render request and output format selected in the UI tab.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OutputPicture {
    key: RenderKey,
    format: UiFormat,
}

impl UiTab {
    pub(crate) fn output_picture(&self, language: Option<TextLanguage>) -> Option<OutputPicture> {
        if !self.export_available() {
            return None;
        }
        Some(OutputPicture {
            key: self.wanted(language)?,
            format: self.settings.format,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::UiTab;
    use crate::{UiBackground, UiFormat, UiMode};

    fn picked() -> UiTab {
        UiTab {
            mode: UiMode::Files,
            file: Some("ui/fixture.gfx".to_owned()),
            ..UiTab::default()
        }
    }

    #[test]
    fn a_picture_export_can_be_selected_before_a_preview_finishes() {
        let tab = picked();
        assert!(tab.rendered().is_none());
        assert!(
            tab.output_picture(None).is_some(),
            "actual export request is the owner"
        );
    }

    #[test]
    fn changing_the_requested_source_or_format_or_background_invalidates_the_scope() {
        let mut tab = picked();
        let first = tab.output_picture(None).expect("selected movie");
        tab.set_format(UiFormat::Svg);
        assert_ne!(Some(&first), tab.output_picture(None).as_ref(), "format");
        tab.set_format(UiFormat::Png);
        tab.set_background(UiBackground::White);
        assert_ne!(
            Some(&first),
            tab.output_picture(None).as_ref(),
            "background"
        );
        tab.set_background(UiBackground::Transparent);
        tab.file = Some("ui/different.gfx".to_owned());
        assert_ne!(Some(&first), tab.output_picture(None).as_ref(), "source");
    }

    #[test]
    fn clearing_the_selected_movie_removes_its_output_scope() {
        let mut tab = picked();
        assert!(tab.output_picture(None).is_some());
        tab.file = None;
        assert!(tab.output_picture(None).is_none());
    }
}
