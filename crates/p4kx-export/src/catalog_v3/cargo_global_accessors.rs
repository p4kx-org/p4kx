// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;

use super::{CargoBoxSizes, CargoLoadingGlobal, ShopCommodityGlobal};
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::resource_float::ResourceFloat32;
use crate::source_value_catalog::SourceArenaOutput;

impl<'source> CargoLoadingGlobal<'source> {
    /// Borrows the original physical Record header.
    #[must_use]
    pub const fn source(&self) -> &CanonicalRecordHeader<'source> {
        &self.source
    }

    /// Borrows the authored automatic loading base time.
    #[must_use]
    pub const fn auto_loading_base_loading_time(&self) -> &SourcedValue<ResourceFloat32> {
        &self.auto_loading_base_loading_time
    }

    /// Borrows the authored automatic unloading base time.
    #[must_use]
    pub const fn auto_loading_base_unloading_time(&self) -> &SourcedValue<ResourceFloat32> {
        &self.auto_loading_base_unloading_time
    }

    /// Borrows the authored cargo deck loading time per SCU.
    #[must_use]
    pub const fn cargo_deck_loading_time_per_scu(&self) -> &SourcedValue<ResourceFloat32> {
        &self.cargo_deck_loading_time_per_scu
    }

    /// Borrows the authored automatic loading times by box size.
    #[must_use]
    pub const fn auto_loading_box_size_loading_time(
        &self,
    ) -> &SourcedValue<CargoBoxSizes<'source, ResourceFloat32>> {
        &self.auto_loading_box_size_loading_time
    }

    /// Borrows the authored automatic unloading times by box size.
    #[must_use]
    pub const fn auto_loading_box_size_unloading_time(
        &self,
    ) -> &SourcedValue<CargoBoxSizes<'source, ResourceFloat32>> {
        &self.auto_loading_box_size_unloading_time
    }

    /// Borrows original members outside the typed cells.
    #[must_use]
    pub const fn additional_fields(&self) -> &SourceArenaOutput<'source> {
        &self.additional_fields
    }
}

impl<'source> ShopCommodityGlobal<'source> {
    /// Borrows the original physical Record header.
    #[must_use]
    pub const fn source(&self) -> &CanonicalRecordHeader<'source> {
        &self.source
    }

    /// Borrows the authored automatic loading base price.
    #[must_use]
    pub const fn auto_loading_base_price(&self) -> &SourcedValue<i32> {
        &self.auto_loading_base_price
    }

    /// Borrows the authored automatic loading prices by box size.
    #[must_use]
    pub const fn auto_loading_box_size_prices(&self) -> &SourcedValue<CargoBoxSizes<'source, i32>> {
        &self.auto_loading_box_size_prices
    }

    /// Borrows original members outside the typed cells.
    #[must_use]
    pub const fn additional_fields(&self) -> &SourceArenaOutput<'source> {
        &self.additional_fields
    }
}
