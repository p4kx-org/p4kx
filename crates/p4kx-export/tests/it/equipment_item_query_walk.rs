// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::CanonicalText;
use p4kx_export::catalog_queries::equipment::{
    AuthoredManufacturer, ItemAttachmentPresentation, ItemDefinition, ItemDefinitionComponent,
    ItemDefinitionComponentKind, ItemLocalization, ItemPhysics, ItemPurchasable,
    WeaponAttachmentMotion,
};
use p4kx_extraction::SourceInventory;
use serde::Serialize;
use serde_json::Value;

use crate::canonical_header::originals;
use crate::canonical_header::wire::Wire;
use crate::catalog_resources::resource_consumer_walk;
use crate::commodity_v3_fixture::{self as common, TestResult};

pub(super) fn definition(
    row: &ItemDefinition<'_>,
    inventory: &SourceInventory,
    canonical: &Value,
) -> TestResult {
    let wire = Wire::new(canonical, inventory);
    resource_consumer_walk::visit_header(row.source(), wire)?;
    wire.field("components")?
        .cell(row.components(), |components, wire| {
            wire.len(components.len())?;
            for (ordinal, component) in components.iter().enumerate() {
                visit_component(component, wire.index(ordinal)?)?;
            }
            Ok(())
        })
}

fn visit_component(component: &ItemDefinitionComponent<'_>, wire: Wire<'_>) -> TestResult {
    wire.field("componentOrdinal")?
        .leaf(component.component_ordinal())?;
    wire.field("sourceType")?.leaf(component.source_type())?;
    let payload = wire.field("value")?;
    match component.body() {
        ItemDefinitionComponentKind::Attachment(attachment) => {
            wire.field("kind")?.raw("attachment")?;
            payload.cell(attachment, |value, wire| visit_attachment(value, wire))
        }
        ItemDefinitionComponentKind::Physics(physics) => {
            wire.field("kind")?.raw("physics")?;
            payload.cell(physics, visit_physics)
        }
        ItemDefinitionComponentKind::Purchasable(purchasable) => {
            wire.field("kind")?.raw("purchasable")?;
            visit_purchasable(purchasable, payload)
        }
        _ => Err("a new original Item component requires a typed traversal".into()),
    }
}

fn visit_attachment(row: &ItemAttachmentPresentation<'_>, wire: Wire<'_>) -> TestResult {
    wire.field("grade")?
        .cell(row.grade(), |number, wire| wire.number(number))?;
    visit_manufacturer(row.manufacturer(), wire.field("manufacturer")?)?;
    wire.field("localization")?
        .cell(row.localization(), visit_localization)?;
    let motion = wire.field("weaponMotion")?;
    row.weapon_motion().map_or_else(
        || motion.raw(&Option::<()>::None),
        |value| visit_motion(value, motion),
    )
}

fn visit_manufacturer(row: &AuthoredManufacturer<'_>, wire: Wire<'_>) -> TestResult {
    let payload = wire.field("value")?;
    match row {
        AuthoredManufacturer::Record(reference) => {
            wire.field("kind")?.raw("record")?;
            payload.cell(reference, |reference, wire| {
                originals::visit_record(reference, wire)
            })
        }
        AuthoredManufacturer::Authored(arena) => {
            wire.field("kind")?.raw("authored")?;
            payload.cell(arena, originals::visit_arena)
        }
        _ => Err("a new original manufacturer storage requires a typed traversal".into()),
    }
}

fn visit_localization(row: &ItemLocalization, wire: Wire<'_>) -> TestResult {
    wire.field("name")?.leaf(row.name())?;
    wire.field("description")?.leaf(row.description())
}

fn visit_physics(row: &ItemPhysics, wire: Wire<'_>) -> TestResult {
    wire.field("mass")?
        .cell(row.mass(), |number, wire| wire.float32(number))
}

fn visit_purchasable(row: &ItemPurchasable, wire: Wire<'_>) -> TestResult {
    wire.field("displayName")?.leaf(row.display_name())?;
    wire.field("displayType")?.leaf(row.display_type())
}

fn visit_motion(row: &WeaponAttachmentMotion<'_>, wire: Wire<'_>) -> TestResult {
    wire.field("itemType")?.leaf(row.item_type())?;
    wire.field("tags")?.leaf(row.tags())
}

pub(super) fn text<R: Serialize>(
    row: &CanonicalText<'_, R>,
    inventory: &SourceInventory,
    catalog: &Value,
    domain: &str,
) -> TestResult {
    let id = serde_json::to_value(row.id().value().ok_or("canonical text ID")?)?;
    let expected = common::text_row(catalog, domain, &id)?;
    let wire = Wire::new(expected, inventory);
    wire.field("id")?.leaf(row.id())?;
    wire.field("key")?.leaf(row.key())?;
    wire.field("translations")?
        .cell(row.translations(), |translations, wire| {
            wire.raw(translations)?;
            for (language, forms) in translations {
                let wire = wire.field(language)?;
                wire.field("sourceText")?.leaf(&forms.source_text)?;
                wire.field("displayText")?.leaf(&forms.display_text)?;
            }
            Ok(())
        })
}
