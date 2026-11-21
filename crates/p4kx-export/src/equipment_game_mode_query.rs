// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, CancelToken, DataForgeEntry, RecordTree};

use super::{ComponentDefinitionSnapshot, Query};
use crate::CatalogError;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, ParsedCatalogQueries,
};
use crate::catalog_read::scoped_source_read::ScopedSourceRead;
use crate::catalog_v3::catalog_resource_network_read::{self, GAME_MODE_TYPES};
use crate::catalog_v3::component_definition_sources::Definition;
use crate::record_observation::RecordObservation;

pub use crate::catalog_v3::catalog_radar_options::{
    RadarModeParameters, RadarOptionParameters, RadarOptionsConfiguration,
};

struct RadarOptions;

impl Definition for RadarOptions {
    type Row<'source> = RadarOptionsConfiguration<'source>;
    const SOURCE_TYPES: &'static [&'static str] = GAME_MODE_TYPES;

    fn project<'source>(
        tree: &RecordTree<'source>,
        projection: &CatalogProjection<'_, 'source>,
        admission: &RecordObservation<'_, 'source>,
        cancel: Option<&CancelToken>,
    ) -> Result<ScopedSourceRead<Self::Row<'source>>, CatalogError> {
        catalog_resource_network_read::read_radar_selected(
            tree,
            projection.original(),
            admission,
            cancel,
        )
    }
}

definition_query!(
    RadarOptions, RadarOptionsConfiguration, RadarOptionsSnapshot, with_radar_options,
    with_radar_options_from, CatalogLanguageSelection::Selected(&[]);
);
