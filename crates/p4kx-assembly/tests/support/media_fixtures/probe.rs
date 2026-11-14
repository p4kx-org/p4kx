// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_assembly::audio::TriggerReference;
use p4kx_assembly::{Entities, EntityNode, LoadoutFiles};
use p4kx_extraction::{Archive, Record, RecordTree};

use super::{Case, geometry};

/// Geometry and audio references retained by an Assembly media fixture.
pub(crate) struct Prepared {
    geometry: geometry::Prepared,
    audio: Vec<TriggerReference>,
}

impl Prepared {
    pub(crate) fn node(&self) -> super::Result<&EntityNode> {
        self.geometry.node()
    }

    pub(crate) fn into_audio(self) -> Vec<TriggerReference> {
        self.audio
    }

    pub(super) const fn facts(&self) -> &geometry::Capabilities {
        self.geometry.facts()
    }
}

pub(super) fn prepare<'f>(
    archive: &Archive,
    files: &'f dyn LoadoutFiles,
    entities: Arc<Entities<'f>>,
    record: &Record<'f>,
    canonical: &RecordTree<'f>,
    case: &Case,
) -> super::Result<Option<Prepared>> {
    let audio = if case.requirements.requires_audio {
        p4kx_assembly::audio::trigger_references(canonical)
    } else {
        Vec::new()
    };
    let geometry = geometry::prepare_with_audio(
        archive,
        files,
        entities,
        record,
        canonical,
        case,
        audio.len(),
    )?;
    Ok(geometry.map(|geometry| Prepared { geometry, audio }))
}
