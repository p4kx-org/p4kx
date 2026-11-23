// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{FieldOrigin, SourcedValue};

use crate::catalog_text_contract::GameTextRef;
use crate::guid_text::CatalogGuid;
use crate::starmap_containers::proof::PlacementEvidence;

/// Starmap object data used for placement and catalog relationship projections.
#[derive(Debug)]
pub(crate) struct StarmapObject<'source> {
    pub(crate) guid: CatalogGuid,
    pub(crate) record_index: u32,
    pub(crate) class_name: &'source str,
    pub(crate) name: SourcedValue<GameTextRef>,
    pub(crate) r#type: Option<&'source str>,
    pub(crate) system: Option<String>,
    pub(crate) entity_name: Option<String>,
    pub(crate) placed_under_guid: Option<CatalogGuid>,
    pub(crate) position: Option<[f64; 3]>,
    pub(crate) placement_proof: Option<PlacementEvidence>,
    pub(crate) mining_claim: bool,
    pub(crate) location_origin: Option<FieldOrigin>,
    pub(crate) shop_link_header: Option<ShopLinkHeader>,
}

#[derive(Debug)]
pub(crate) struct ShopLinkHeader {
    pub(crate) guid: FieldOrigin,
    pub(crate) class_name: FieldOrigin,
}
