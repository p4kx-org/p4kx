// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::hash::Hasher;
use std::io;

use p4kx_extraction::{RecordTree, Value};

use crate::contract_stats::ItemStats;
use crate::digest::{ComparedValue, Digest, Wide, hash_value};
use crate::guid_text::CatalogGuid;
use crate::record_fields::{integer, reference, text};

#[path = "item_variant_component_origin.rs"]
mod component_origin;
pub(crate) use component_origin::capture_component_key;

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

/// What the items of one variant group share (`docs/catalog.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct ItemKey<'f> {
    components: ItemComponentKey<'f>,
    stats: Digest,
}

/// The part of an Item variant key established by the owning Record alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct ItemComponentKey<'f> {
    kind: &'f str,
    sub_type: Option<&'f str>,
    size: Option<i64>,
    grade: Option<i64>,
    manufacturer: Option<CatalogGuid>,
    digest: Digest,
}

impl<'source> ItemKey<'source> {
    pub(crate) const fn components(self) -> ItemComponentKey<'source> {
        self.components
    }
}

impl<'f> ItemComponentKey<'f> {
    pub(crate) const fn kind(self) -> &'f str {
        self.kind
    }
    pub(crate) fn with_stats(self, stats: Option<&ItemStats<'_>>) -> Option<ItemKey<'f>> {
        Some(ItemKey {
            components: self,
            stats: stats_digest(stats)?,
        })
    }
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

/// Reads the comparable components of an Item without resolving linked stat Records.
pub(crate) fn component_key<'source>(
    tree: &RecordTree<'source>,
) -> Option<ItemComponentKey<'source>> {
    match read_component_key(tree) {
        ComponentKeyRead::Comparable(key) => Some(key),
        ComponentKeyRead::Ineligible
        | ComponentKeyRead::UnresolvedLinks
        | ComponentKeyRead::Unavailable => None,
    }
}

/// The component comparison outcome of one physical Item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ComponentKeyRead<'source> {
    Comparable(ItemComponentKey<'source>),
    Ineligible,
    UnresolvedLinks,
    Unavailable,
}

pub(crate) fn read_component_key<'source>(tree: &RecordTree<'source>) -> ComponentKeyRead<'source> {
    let components = match tree.field("Components") {
        None | Some(Value::Null) => return ComponentKeyRead::Ineligible,
        Some(Value::Array(components)) => components,
        Some(_) => return ComponentKeyRead::Unavailable,
    };
    let Some(attachment) = first_attachment(components) else {
        return ComponentKeyRead::Ineligible;
    };
    let Value::Struct(attachment) = attachment else {
        return ComponentKeyRead::Unavailable;
    };
    let definition = match attachment.field("AttachDef") {
        None | Some(Value::Null) => return ComponentKeyRead::Ineligible,
        Some(Value::Struct(definition)) => definition,
        Some(_) => return unavailable_attachment_key(components),
    };
    let kind = match definition.field("Type") {
        None | Some(Value::Null | Value::String("") | Value::Enum("")) => {
            return ComponentKeyRead::Ineligible;
        }
        Some(Value::String(kind) | Value::Enum(kind)) => *kind,
        Some(_) => return unavailable_attachment_key(components),
    };
    let digest = match component_digest(components, Some(Wide::new())) {
        ComponentDigest::Comparable(digest) => digest,
        ComponentDigest::Ineligible => return ComponentKeyRead::Ineligible,
        ComponentDigest::UnresolvedLinks => return ComponentKeyRead::UnresolvedLinks,
        ComponentDigest::Unavailable => return ComponentKeyRead::Unavailable,
    };
    ComponentKeyRead::Comparable(ItemComponentKey {
        kind,
        sub_type: text(Some(definition), "SubType"),
        size: integer(Some(definition), "Size"),
        grade: integer(Some(definition), "Grade"),
        manufacturer: reference(Some(definition), "Manufacturer"),
        digest,
    })
}

/// Reads the original Item type even when another compared component is unreadable.
pub(crate) fn known_item_type<'source>(tree: &RecordTree<'source>) -> Option<&'source str> {
    let Value::Array(components) = tree.field("Components")? else {
        return None;
    };
    let Value::Struct(attachment) = first_attachment(components)? else {
        return None;
    };
    let Value::Struct(definition) = attachment.field("AttachDef")? else {
        return None;
    };
    text(Some(definition), "Type")
}

fn first_attachment<'read, 'source>(
    components: &'read [Value<'source>],
) -> Option<&'read Value<'source>> {
    components.iter().find(|value| match value {
        Value::Struct(component) => component.type_name() == ATTACHABLE,
        Value::Null => false,
        _ => true,
    })
}

fn unavailable_attachment_key<'source>(components: &[Value<'source>]) -> ComponentKeyRead<'source> {
    match component_digest(components, None) {
        ComponentDigest::Ineligible => ComponentKeyRead::Ineligible,
        _ => ComponentKeyRead::Unavailable,
    }
}

enum ComponentDigest {
    Comparable(Digest),
    Ineligible,
    UnresolvedLinks,
    Unavailable,
}

fn component_digest(components: &[Value<'_>], mut hasher: Option<Wide>) -> ComponentDigest {
    let mut beyond_attach = false;
    let mut outcome = ComparedValue::Comparable;
    for entry in components {
        let name = match entry {
            Value::Struct(inner) => inner.type_name(),
            Value::Null => continue,
            _ => return ComponentDigest::Unavailable,
        };
        if PER_ITEM_COMPONENTS.contains(&name) {
            continue;
        }
        beyond_attach |= name != ATTACHABLE;
        if let Some(state) = &mut hasher
            && outcome != ComparedValue::Unavailable
        {
            let skipped = skipped_fields(name);
            outcome = outcome.max(hash_value(state, entry, &|field| skipped.contains(&field)));
        }
    }
    if !beyond_attach {
        return ComponentDigest::Ineligible;
    }
    match (hasher, outcome) {
        (Some(state), ComparedValue::Comparable) => ComponentDigest::Comparable(state.digest()),
        (Some(_), ComparedValue::UnresolvedLink) => ComponentDigest::UnresolvedLinks,
        _ => ComponentDigest::Unavailable,
    }
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

#[cfg(test)]
#[path = "item_variant_component_tests.rs"]
mod component_tests;
