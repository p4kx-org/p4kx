// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;

use p4kx_extraction::{RecordTree, SourcedValue};

use super::Observed;
use super::item_definition::ItemPurchasable;
use super::selection::ItemSourceSelection;
use crate::CatalogError;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_read::original::OriginalFlowFields;
use crate::item::playability_projection::read_purchase_rank;
use crate::playability_rules::PurchaseRank;
use crate::record_observation::RecordObservation;
use crate::template_field_capture::TemplateFieldCapture;

/// Reads the purchase rank from the original first purchasable component.
pub(crate) fn read_item_purchase_rank<'source>(
    input: &OriginalCatalogInput<'_, 'source>,
    tree: &RecordTree<'source>,
    originals: &OriginalFlowFields<'_, '_, 'source>,
) -> Result<SourcedValue<PurchaseRank>, CatalogError> {
    originals.check_tree(tree)?;
    ItemSourceSelection::with_purchasable(tree, originals, |ordinal, value| {
        let Some(value) = value else {
            return read_purchase_rank(&ordinal, None, originals.sources());
        };
        let scope = originals.scope();
        let admission = RecordObservation::capture(input, scope.record())?;
        let capture = TemplateFieldCapture::new(originals, input, &admission)?;
        let numbers = RefCell::new(Vec::new());
        let fact = originals.fact(value)?;
        let observed = Observed::new(originals, input, fact, &capture, &numbers);
        let (_, name) = ItemPurchasable::read_display_name(observed, &mut Vec::new(), |_| Ok(()))?;
        read_purchase_rank(&ordinal, Some(&name), originals.sources())
    })
}
