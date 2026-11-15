// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::KindTextSummary;
use serde::Serialize;

#[derive(Serialize)]
pub(super) struct TextKindJson {
    owner: &'static str,
    language: &'static str,
    resolved: usize,
    missing_language: usize,
    missing_key: usize,
    empty_source: usize,
    empty_display: usize,
    placeholder: usize,
}

impl From<&KindTextSummary> for TextKindJson {
    fn from(text: &KindTextSummary) -> Self {
        Self {
            owner: text.owner.name(),
            language: text.language.code(),
            resolved: text.resolved,
            missing_language: text.missing_language,
            missing_key: text.missing_key,
            empty_source: text.empty_source,
            empty_display: text.empty_display,
            placeholder: text.placeholder,
        }
    }
}
