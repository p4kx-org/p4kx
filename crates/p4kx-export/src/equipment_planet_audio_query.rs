// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Selective access to original procedural planet audio definitions.

use p4kx_extraction::{CancelToken, DataForge, RecordTree};

use super::record_query;
use super::rows::{Family, Projector, ReadOptions};
use crate::CatalogError;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_read::scoped_source_read::ScopedSourceRead;
use crate::catalog_v3::procedural_planet_audio;
use crate::record_observation::RecordObservation;

pub use crate::catalog_v3::procedural_planet_audio::{
    PlanetAudioControls, PlanetAudioRtpc, ProceduralPlanetAudio,
};

struct PlanetAudio;

impl Projector for PlanetAudio {
    type Row<'source> = ProceduralPlanetAudio<'source>;
    const FAMILY: Family = Family::ProceduralPlanetAudio;

    fn read<'source>(
        _: &'source DataForge<'source>,
        tree: &RecordTree<'source>,
        projection: &CatalogProjection<'_, 'source>,
        admission: &RecordObservation<'_, 'source>,
        _: &ReadOptions<'_>,
        cancel: Option<&CancelToken>,
    ) -> Result<ScopedSourceRead<Vec<Self::Row<'source>>>, CatalogError> {
        let read = procedural_planet_audio::read_selected(tree, projection, admission, cancel)?;
        Ok(read.map(|row| vec![row]))
    }
}

record_query::unlocalized!(
    PlanetAudio,
    ProceduralPlanetAudio,
    /// Original procedural planet audio definitions and their source evidence.
    ProceduralPlanetAudioSnapshot,
    with_procedural_planet_audio,
    with_procedural_planet_audio_from
);
