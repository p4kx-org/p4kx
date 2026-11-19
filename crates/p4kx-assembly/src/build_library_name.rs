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

pub(crate) struct Selection<'a> {
    pub(crate) entry: &'a str,
    pub(crate) name: LibraryName<'a>,
}

pub(crate) fn choose<'a>(draft: &'a Draft<'_>, loaded: &'a Loaded) -> Option<Selection<'a>> {
    let (entry, model) = if loaded.rig_only {
        let primary = loaded.attachments.first()?;
        (primary.entry.as_str(), primary.model.as_ref())
    } else {
        (loaded.entry.as_str(), loaded.model.as_ref())
    };
    let name = draft
        .library
        .or_else(|| draft.geometry.and_then(|geometry| geometry.material()))
        .map(LibraryName::Override)
        .or_else(|| model.material_library.as_deref().map(LibraryName::Model))?;
    Some(Selection { entry, name })
}
