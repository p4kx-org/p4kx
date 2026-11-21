// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, DataForge};

use crate::CatalogError;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_queries::{CatalogRecordCoverage, CatalogRecordSelection};
use crate::catalog_table::CatalogTable;

pub(crate) trait PhysicalComponent {
    type Row<'source>;

    fn query<'source>(
        forge: &'source DataForge<'source>,
        input: &OriginalCatalogInput<'_, 'source>,
        owners: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
    ) -> Result<(CatalogTable<Self::Row<'source>>, CatalogRecordCoverage), CatalogError>;
}

macro_rules! physical_component {
    ($name:ident, $row:ident, $kind:ident) => {
        pub(crate) struct $name;

        impl $crate::catalog_v3::component_sources::PhysicalComponent for $name {
            type Row<'source> = $row<'source>;

            fn query<'source>(
                forge: &'source p4kx_extraction::DataForge<'source>,
                input: &$crate::build::original_input::OriginalCatalogInput<'_, 'source>,
                owners: $crate::catalog_queries::CatalogRecordSelection,
                cancel: Option<&p4kx_extraction::CancelToken>,
            ) -> Result<
                (
                    $crate::catalog_table::CatalogTable<Self::Row<'source>>,
                    $crate::catalog_queries::CatalogRecordCoverage,
                ),
                $crate::CatalogError,
            > {
                let (rows, coverage) =
                    $crate::catalog_v3::catalog_entity_component_read::read_selected(
                        forge,
                        input,
                        owners,
                        cancel,
                        &$kind,
                        $row::read,
                    )?;
                Ok((
                    $crate::catalog_table::CatalogTable::new(forge, &[$kind.source_type], rows),
                    coverage,
                ))
            }
        }
    };
}

pub(in crate::catalog_v3) use physical_component;
