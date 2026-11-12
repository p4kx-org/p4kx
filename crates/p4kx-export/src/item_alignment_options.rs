// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use p4kx_extraction::{CancelToken, DataForge, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use crate::CatalogError;

pub(super) const ENUM_NAME: &str = "EItemPortAttachRotationLimitAxis";

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = crate::schema_fields::every_field_required)]
pub(super) struct AlignmentAxis<'source> {
    pub(super) label: SourcedValue<&'source str>,
    pub(super) option_status: Option<SourcedValue<AttachmentAxisOptionStatus>>,
}

#[derive(Debug, Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(extend("x-p4kx-vocabularyClass" = "control"))]
pub(super) enum AttachmentAxisOptionStatus {
    Declared,
    Undeclared,
}

#[derive(Default)]
pub(crate) struct AlignmentOptions<'source> {
    enums: HashMap<u16, Options<'source>>,
}

#[derive(Default)]
struct Options<'source> {
    labels: HashMap<&'source str, usize>,
    unreadable: Option<usize>,
}

pub(super) enum OptionEvidence {
    Declared(usize),
    Undeclared,
    Unreadable(usize),
}

impl<'source> AlignmentOptions<'source> {
    pub(crate) fn read(
        forge: &'source DataForge<'_>,
        cancel: Option<&CancelToken>,
    ) -> Result<Self, CatalogError> {
        let mut this = Self::default();
        for (index, enumeration) in forge.enums().enumerate() {
            if cancel.is_some_and(CancelToken::is_cancelled) {
                return Err(CatalogError::Cancelled);
            }
            let Ok(index) = u16::try_from(index) else {
                break;
            };
            if enumeration.name().ok() != Some(ENUM_NAME) {
                continue;
            }
            let mut options = Options::default();
            for (ordinal, option) in enumeration.options().enumerate() {
                if cancel.is_some_and(CancelToken::is_cancelled) {
                    return Err(CatalogError::Cancelled);
                }
                match option {
                    Ok(label) => {
                        options.labels.entry(label).or_insert(ordinal);
                    }
                    Err(_) => {
                        options.unreadable.get_or_insert(ordinal);
                    }
                }
            }
            this.enums.insert(index, options);
        }
        Ok(this)
    }

    pub(super) fn lookup(&self, index: u16, label: &str) -> Option<OptionEvidence> {
        let options = self.enums.get(&index)?;
        Some(options.labels.get(label).copied().map_or_else(
            || {
                options
                    .unreadable
                    .map_or(OptionEvidence::Undeclared, OptionEvidence::Unreadable)
            },
            OptionEvidence::Declared,
        ))
    }
}
