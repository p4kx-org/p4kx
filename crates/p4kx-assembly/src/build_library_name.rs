// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::build_models::Loaded;
use crate::flatten::Draft;

#[derive(Clone, Copy)]
pub(crate) enum LibraryName<'a> {
    Override(&'a str),
    Model(&'a str),
}

impl<'a> LibraryName<'a> {
    pub(crate) const fn path(self) -> &'a str {
        match self {
            Self::Override(name) | Self::Model(name) => name,
        }
    }
}

pub(crate) fn choose<'a>(draft: &'a Draft<'_>, loaded: &'a Loaded) -> Option<LibraryName<'a>> {
    draft
        .library
        .or_else(|| draft.geometry.and_then(|geometry| geometry.material()))
        .map(LibraryName::Override)
        .or_else(|| {
            loaded
                .model
                .material_library
                .as_deref()
                .map(LibraryName::Model)
        })
}
