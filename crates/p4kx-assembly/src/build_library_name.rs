// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::build_models::Loaded;
use crate::flatten::Draft;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LibrarySource {
    Override,
    Geometry,
    CharacterDefinition,
    Model,
}

impl LibrarySource {
    pub(crate) const fn uses_model_material(self) -> bool {
        matches!(self, Self::CharacterDefinition | Self::Model)
    }
}

pub(crate) struct Selection<'a> {
    pub(crate) entry: &'a str,
    pub(crate) name: &'a str,
    pub(crate) source: LibrarySource,
}

pub(crate) fn choose<'a>(draft: &'a Draft<'_>, loaded: &'a Loaded) -> Option<Selection<'a>> {
    let (entry, model) = if loaded.rig_only {
        let primary = loaded.attachments.first()?;
        (primary.entry.as_str(), primary.model.as_ref())
    } else {
        (loaded.entry.as_str(), loaded.model.as_ref())
    };
    let geometry = || draft.geometry.and_then(|geometry| geometry.material());
    let model = model.material_library.as_deref();
    let definition = loaded.cdf_material.as_ref().and(model);
    let prefers_definition = draft
        .node
        .rendered_paint(draft.geometry)
        .is_some_and(|paint| paint.prefers_cdf_material);
    let (name, source) = draft
        .library
        .map(|name| (name, LibrarySource::Override))
        .or_else(|| {
            definition
                .filter(|_| prefers_definition)
                .map(|name| (name, LibrarySource::CharacterDefinition))
        })
        .or_else(|| geometry().map(|name| (name, LibrarySource::Geometry)))
        .or_else(|| definition.map(|name| (name, LibrarySource::CharacterDefinition)))
        .or_else(|| model.map(|name| (name, LibrarySource::Model)))?;
    Some(Selection {
        entry,
        name,
        source,
    })
}
