// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::hash::Hasher;
use std::io;

use p4kx_extraction::{RecordTree, Value};

use crate::contract_stats::ItemStats;
use crate::digest::{Digest, Wide, hash_value};
use crate::guid_text::CatalogGuid;
use crate::record_fields::{component, integer, reference, structure, text};

const ATTACHABLE: &str = "SAttachableComponentParams";
const GEOMETRY: &str = "SGeometryResourceParams";
const INVENTORY: &str = "SCItemInventoryContainerComponentParams";

/// The components whose data is per-item Records of equal content (interaction, carrying and
/// actor-usage setups); they are not compared.
const PER_ITEM_COMPONENTS: [&str; 4] = [
    "SEntityInteractableParams",
    "SInteractionStateMachineParams",
    "SEntityComponentCarryableParams",
    "SActorUsableParams",
];

/// What the items of one variant group share (`docs/basetool-catalog.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct ItemKey<'f> {
    kind: &'f str,
    sub_type: Option<&'f str>,
    size: Option<i64>,
    grade: Option<i64>,
    manufacturer: Option<CatalogGuid>,
    components: Digest,
    stats: Digest,
}

/// Returns the field names of the component `type_name` that are left out of the comparison.
fn skipped_fields(type_name: &str) -> &'static [&'static str] {
    match type_name {
        ATTACHABLE => &["Tags"],
        GEOMETRY => &["Material", "Materials", "Palette"],
        INVENTORY => &["openInventoryInteraction", "openLootingInteraction"],
        _ => &[],
    }
}

/// Returns the key of the item `tree` with the stat blocks `stats`; `None` for an item without
/// an `AttachDef` type, without a compared component besides `SAttachableComponentParams`, or
/// whose data holds an error.
pub(crate) fn key<'f>(tree: &RecordTree<'f>, stats: Option<&ItemStats<'_>>) -> Option<ItemKey<'f>> {
    let definition = structure(component(tree, ATTACHABLE), "AttachDef");
    let kind = text(definition, "Type")?;
    let mut hasher = Wide::new();
    let mut beyond_attach = false;
    if let Some(Value::Array(components)) = tree.field("Components") {
        for entry in components {
            let Value::Struct(inner) = entry else {
                continue;
            };
            let name = inner.type_name();
            if PER_ITEM_COMPONENTS.contains(&name) {
                continue;
            }
            beyond_attach |= name != ATTACHABLE;
            let skipped = skipped_fields(name);
            if !hash_value(&mut hasher, entry, &|field| skipped.contains(&field)) {
                return None;
            }
        }
    }
    if !beyond_attach {
        return None;
    }
    Some(ItemKey {
        kind,
        sub_type: text(definition, "SubType"),
        size: integer(definition, "Size"),
        grade: integer(definition, "Grade"),
        manufacturer: reference(definition, "Manufacturer"),
        components: hasher.digest(),
        stats: stats_digest(stats)?,
    })
}

fn stats_digest(stats: Option<&ItemStats<'_>>) -> Option<Digest> {
    let mut hasher = Wide::new();
    if let Some(stats) = stats {
        serde_json::to_writer(HasherWriter(&mut hasher), stats).ok()?;
    }
    Some(hasher.digest())
}

struct HasherWriter<'h, H: Hasher>(&'h mut H);

impl<H: Hasher> io::Write for HasherWriter<'_, H> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.write(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
#[path = "item_variant_stats_tests.rs"]
mod stats_tests;
