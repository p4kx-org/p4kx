// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, RecordTree};

use super::family::Family;
use crate::CatalogError;
use crate::catalog_origins::CatalogOrigins;
use crate::quality_catalog::{contract, read};
use crate::source_fields::FieldCapture;

pub(super) trait Projector {
    type Row<'source>;
    const FAMILY: Family;

    fn read<'source>(
        tree: &RecordTree<'source>,
        origins: &CatalogOrigins,
        cancel: Option<&CancelToken>,
        capture: &dyn FieldCapture<'source>,
    ) -> Result<read::Read<Self::Row<'source>>, CatalogError>;
}

pub(super) struct Distributions;
pub(super) struct Quantizations;

impl Projector for Distributions {
    type Row<'source> = contract::QualityDistributionDefinition<'source>;
    const FAMILY: Family = Family::Distributions;

    fn read<'source>(
        tree: &RecordTree<'source>,
        origins: &CatalogOrigins,
        cancel: Option<&CancelToken>,
        capture: &dyn FieldCapture<'source>,
    ) -> Result<read::Read<Self::Row<'source>>, CatalogError> {
        read::distribution(tree, origins, cancel, capture)
    }
}

impl Projector for Quantizations {
    type Row<'source> = contract::QualityQuantizationDefinition<'source>;
    const FAMILY: Family = Family::Quantizations;

    fn read<'source>(
        tree: &RecordTree<'source>,
        origins: &CatalogOrigins,
        cancel: Option<&CancelToken>,
        capture: &dyn FieldCapture<'source>,
    ) -> Result<read::Read<Self::Row<'source>>, CatalogError> {
        read::quantization(tree, origins, cancel, capture)
    }
}
