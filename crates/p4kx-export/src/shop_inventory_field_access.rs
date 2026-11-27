// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DecimalU64, SourcedValue};

use super::{InventoryCollection, InventoryId, InventoryLine, InventoryLines, ShopInventoryFields};

impl ShopInventoryFields {
    /// Borrows the optional address of the original shop identifier.
    #[must_use]
    pub const fn shop_id(&self) -> &SourcedValue<DecimalU64> {
        &self.shop_id
    }

    /// Borrows the original root inventory list.
    #[must_use]
    pub const fn inventory(&self) -> &SourcedValue<InventoryLines> {
        &self.inventory
    }

    /// Borrows the authored collection wrapper.
    #[must_use]
    pub const fn collection(&self) -> &SourcedValue<InventoryCollection> {
        &self.collection
    }
}

impl InventoryCollection {
    /// Borrows the original inventory list within the collection.
    #[must_use]
    pub const fn inventory(&self) -> &SourcedValue<InventoryLines> {
        &self.inventory
    }
}

impl InventoryLine {
    /// Returns the line's address within the original value arena.
    #[must_use]
    pub const fn node(&self) -> DecimalU64 {
        self.node
    }

    /// Borrows the authored item identifier field.
    #[must_use]
    pub const fn id(&self) -> &SourcedValue<InventoryId> {
        &self.id
    }

    /// Borrows the original buy-price address.
    #[must_use]
    pub const fn buy_price(&self) -> &SourcedValue<DecimalU64> {
        &self.buy_price
    }

    /// Borrows the original sell-price address.
    #[must_use]
    pub const fn sell_price(&self) -> &SourcedValue<DecimalU64> {
        &self.sell_price
    }

    /// Borrows the original current-inventory address.
    #[must_use]
    pub const fn current_inventory(&self) -> &SourcedValue<DecimalU64> {
        &self.current_inventory
    }

    /// Borrows the original maximum-inventory address.
    #[must_use]
    pub const fn max_inventory(&self) -> &SourcedValue<DecimalU64> {
        &self.max_inventory
    }

    /// Borrows the original rental-offerings address.
    #[must_use]
    pub const fn rental_offerings(&self) -> &SourcedValue<DecimalU64> {
        &self.rental_offerings
    }
}

impl InventoryId {
    /// Borrows the authored identifier alternatives.
    #[must_use]
    pub const fn alternatives(&self) -> &SourcedValue<Vec<SourcedValue<DecimalU64>>> {
        &self.alternatives
    }
}
