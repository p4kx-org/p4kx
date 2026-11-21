// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::{OutputTargetText as T, Texts};

/// A system chooser failed without committing a selection.
#[derive(Debug)]
pub(crate) enum PickerFailure {
    Spawn(std::io::Error),
    Disconnected,
    RelativePath,
}

impl PickerFailure {
    pub(crate) fn message(&self, texts: &Texts) -> String {
        match self {
            Self::Spawn(error) => {
                texts.format(T::PickerFailed, &[("reason", error.to_string().into())])
            }
            Self::Disconnected => texts.get(T::PickerDisconnected),
            Self::RelativePath => texts.get(T::PickerRelative),
        }
    }
}
