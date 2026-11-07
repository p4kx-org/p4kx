// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::starmap_report::StarmapReport;
use crate::{
    CatalogFailure, CatalogSummary, CraftingWarning, LoadoutSummary, PaintSummary, ShopSummary,
    StarmapSummary, StoreSlugSummary,
};
/// What a build knows of a catalog besides its size.
pub(super) struct Parts {
    pub(super) export_health: crate::ExportHealth,
    pub(super) kinds: Vec<crate::KindSummary>,
    pub(super) texts: Vec<crate::TextSummary>,
    pub(super) notes: Vec<p4kx_extraction::LocalizationNote>,
    pub(super) item_classes: crate::ItemClassSummary,
    pub(super) paints: PaintSummary,
    pub(super) loadouts: LoadoutSummary,
    pub(super) starmap: StarmapSummary,
    pub(super) starmap_report: Box<StarmapReport>,
    pub(super) shops: Box<ShopSummary>,
    pub(super) store_slugs: Box<StoreSlugSummary>,
    pub(super) playability: crate::PlayabilitySummary,
    pub(super) variants: crate::VariantSummary,
    pub(super) quantizations: crate::QuantizationSummary,
    pub(super) failures: Vec<CatalogFailure>,
    pub(super) warnings: Vec<CraftingWarning>,
    pub(super) archive_key: crate::ArchiveKeyUse,
}

impl Parts {
    pub(super) fn summary(self, size: u64, compressed_size: Option<u64>) -> CatalogSummary {
        CatalogSummary {
            export_health: self.export_health,
            kinds: self.kinds,
            texts: self.texts,
            notes: self.notes,
            item_classes: self.item_classes,
            paints: self.paints,
            loadouts: self.loadouts,
            starmap: self.starmap,
            starmap_report: self.starmap_report,
            shops: self.shops,
            store_slugs: self.store_slugs,
            playability: self.playability,
            variants: self.variants,
            quantizations: self.quantizations,
            failures: self.failures,
            warnings: self.warnings,
            archive_key: self.archive_key,
            size,
            compressed_size,
        }
    }
}
