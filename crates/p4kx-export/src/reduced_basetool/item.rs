// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde::Serialize;

use super::cell::Cell;
use super::record_identity::{self, RecordIdentity, Variant};
use crate::item::Item as CanonicalItem;
use crate::{ItemClass, Playability};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Item<'source> {
    #[serde(flatten)]
    identity: RecordIdentity<'source>,
    item_class: Cell<&'source ItemClass>,
    playability: Cell<&'source Playability>,
    variant_of: Cell<Variant<'source>>,
}

impl<'source> From<&'source CanonicalItem<'source>> for Item<'source> {
    fn from(item: &'source CanonicalItem<'source>) -> Self {
        let source = item.source();
        Self {
            identity: source.source().into(),
            item_class: source.item_class().into(),
            playability: source.playability().into(),
            variant_of: record_identity::variant(item.variant_of()),
        }
    }
}
