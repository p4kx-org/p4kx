// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;

use p4kx_extraction::{DataType, RecordTree, SourceUseRole, SourceValue, SourcedValue, Value};
use schemars::JsonSchema;
use serde::Serialize;

use super::Observed;
use crate::CatalogError;
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_number_capture::CapturedNumber;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_read::original::{OriginalFlowFields, OriginalValuePart};
use crate::catalog_record_reference::CatalogRecordReference;
use crate::catalog_text_contract::{GameTextRef, PublicNameRef};
use crate::resource_float::ResourceFloat32;
use crate::resource_number::ResourceNumber;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::{SourceArenaOutput, SourceRecordReference};
use crate::template_field_capture::TemplateFieldCapture;

const ATTACHABLE: &str = "SAttachableComponentParams";
const PHYSICS: &str = "SEntityPhysicsControllerParams";
const PURCHASABLE: &str = "SCItemPurchasableParams";

/// Original Item identity and independently authored presentation and physical fields.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ItemDefinition<'source> {
    #[serde(flatten)]
    header: CanonicalRecordHeader<'source>,
    components: SourcedValue<Vec<ItemDefinitionComponent<'source>>>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct ItemDefinitionComponent<'source> {
    component_ordinal: SourcedValue<u32>,
    source_type: SourcedValue<&'source str>,
    #[serde(flatten)]
    body: ItemDefinitionComponentKind<'source>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(tag = "kind", content = "value", rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
enum ItemDefinitionComponentKind<'source> {
    Attachment(SourcedValue<Box<ItemAttachmentPresentation<'source>>>),
    Physics(SourcedValue<ItemPhysics>),
    Purchasable(ItemPurchasable),
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct ItemAttachmentPresentation<'source> {
    grade: SourcedValue<ResourceNumber>,
    manufacturer: ItemManufacturer<'source>,
    localization: SourcedValue<ItemLocalization>,
    weapon_motion: Option<WeaponAttachmentMotion<'source>>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(tag = "kind", content = "value", rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
enum ItemManufacturer<'source> {
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    Record(SourcedValue<Box<SourceRecordReference<'source>>>),
    Authored(SourcedValue<SourceArenaOutput<'source>>),
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct ItemLocalization {
    name: SourcedValue<PublicNameRef>,
    #[schemars(extend("x-gameText" = true))]
    description: SourcedValue<GameTextRef>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct ItemPhysics {
    mass: SourcedValue<ResourceFloat32>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct ItemPurchasable {
    display_name: SourcedValue<PublicNameRef>,
    #[schemars(extend("x-gameText" = true))]
    display_type: SourcedValue<GameTextRef>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct WeaponAttachmentMotion<'source> {
    item_type: SourcedValue<&'source str>,
    tags: SourcedValue<&'source str>,
}

pub(crate) struct WeaponAttachment<'item, 'source> {
    pub(crate) record: &'item SourcedValue<u32>,
    pub(crate) slot: &'item SourcedValue<u32>,
    pub(crate) item_type: &'item SourcedValue<&'source str>,
    pub(crate) tags: &'item SourcedValue<&'source str>,
}

impl<'source> ItemDefinition<'source> {
    pub(crate) const fn record_index(&self) -> &SourcedValue<u32> {
        &self.header.record_index
    }

    pub(crate) fn weapon_attachments(&self) -> impl Iterator<Item = WeaponAttachment<'_, 'source>> {
        self.components
            .value()
            .into_iter()
            .flatten()
            .filter_map(|component| {
                let ItemDefinitionComponentKind::Attachment(attachment) = &component.body else {
                    return None;
                };
                let source = attachment.value()?.weapon_motion.as_ref()?;
                Some(WeaponAttachment {
                    record: &self.header.record_index,
                    slot: &component.component_ordinal,
                    item_type: &source.item_type,
                    tags: &source.tags,
                })
            })
    }

    pub(super) fn read(
        tree: &RecordTree<'source>,
        originals: &OriginalFlowFields<'_, '_, 'source>,
        capture: &TemplateFieldCapture<'_, '_, '_, 'source>,
        projection: &CatalogProjection<'_, 'source>,
        numbers: &RefCell<Vec<CapturedNumber>>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            header: CanonicalRecordHeader::read(tree, projection.origins())?,
            components: read_components(originals, capture, projection, numbers, notes)?,
        })
    }

    pub(crate) fn order(rows: &mut [Self]) {
        rows.sort_by_cached_key(|row| {
            (
                row.header
                    .guid
                    .value()
                    .copied()
                    .map(crate::guid_text::CatalogGuid::text),
                row.header.record_index.value().copied(),
            )
        });
    }
}

fn read_components<'source>(
    originals: &OriginalFlowFields<'_, '_, 'source>,
    capture: &TemplateFieldCapture<'_, '_, '_, 'source>,
    projection: &CatalogProjection<'_, 'source>,
    numbers: &RefCell<Vec<CapturedNumber>>,
    notes: &mut Vec<FieldError>,
) -> Result<SourcedValue<Vec<ItemDefinitionComponent<'source>>>, CatalogError> {
    let Some((_, cell)) = originals.field(None, "Components")? else {
        return Ok(originals.sources().value(
            SourceValue::Absent,
            &[originals.survey(None)?],
            true,
        ));
    };
    let mut uses = vec![cell.observe(OriginalValuePart::Value, SourceUseRole::Selector)?];
    let mut complete = true;
    let state = match cell.value() {
        Value::Array(values) => {
            let mut components = Vec::new();
            for value in values {
                originals.check_cancel()?;
                let fact = originals.fact(value)?;
                match value {
                    Value::Struct(_) => {
                        uses.push(
                            fact.observe(OriginalValuePart::ConcreteType, SourceUseRole::Selector)?,
                        );
                        let source =
                            Observed::new(originals, projection.original(), fact, capture, numbers);
                        if let Some(component) = read_component(source, projection, notes)? {
                            components.push(component);
                        }
                    }
                    Value::Error(_) => {
                        uses.push(
                            fact.observe(OriginalValuePart::Value, SourceUseRole::Attempted)?,
                        );
                        complete = false;
                    }
                    _ => {
                        uses.push(fact.observe(OriginalValuePart::Value, SourceUseRole::Selector)?);
                    }
                }
            }
            SourceValue::Present(components)
        }
        Value::Null => SourceValue::Null,
        Value::Error(_) => {
            complete = false;
            SourceValue::Unreadable
        }
        _ => SourceValue::Invalid,
    };
    Ok(originals.sources().value(state, &uses, complete))
}

fn read_component<'source>(
    source: Observed<'_, '_, '_, 'source>,
    projection: &CatalogProjection<'_, 'source>,
    notes: &mut Vec<FieldError>,
) -> Result<Option<ItemDefinitionComponent<'source>>, CatalogError> {
    let body = match source.source_type()? {
        ATTACHABLE => ItemDefinitionComponentKind::Attachment(
            if source.storage_type("AttachDef")? == Some(DataType::StrongPointer) {
                source.declared_pointer("AttachDef", notes, |source, notes| {
                    ItemAttachmentPresentation::read(source, projection, notes).map(Box::new)
                })?
            } else {
                source.declared_class("AttachDef", notes, |source, notes| {
                    ItemAttachmentPresentation::read(source, projection, notes).map(Box::new)
                })?
            },
        ),
        PHYSICS => ItemDefinitionComponentKind::Physics(
            if source.storage_type("PhysType")? == Some(DataType::StrongPointer) {
                source.declared_pointer("PhysType", notes, ItemPhysics::read)?
            } else {
                source.declared_class("PhysType", notes, ItemPhysics::read)?
            },
        ),
        PURCHASABLE => ItemDefinitionComponentKind::Purchasable(ItemPurchasable {
            display_name: source.public_name("displayName", projection, notes)?,
            display_type: source.protected_locale("displayType", projection, notes)?,
        }),
        _ => return Ok(None),
    };
    Ok(Some(ItemDefinitionComponent {
        component_ordinal: source.array_ordinal()?,
        source_type: source.source_type_value()?,
        body,
    }))
}

impl<'source> ItemAttachmentPresentation<'source> {
    fn read(
        source: Observed<'_, '_, '_, 'source>,
        projection: &CatalogProjection<'_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            weapon_motion: WeaponAttachmentMotion::read(source, notes)?,
            grade: source.authored_number("Grade", notes)?,
            manufacturer: match source.storage_type("Manufacturer")? {
                None | Some(DataType::Reference) => {
                    ItemManufacturer::Record(source.record_link("Manufacturer", notes)?)
                }
                _ => {
                    ItemManufacturer::Authored(source.authored_member_with_errors("Manufacturer")?)
                }
            },
            localization: source.declared_class("Localization", notes, |source, notes| {
                Ok(ItemLocalization {
                    name: source.public_name("Name", projection, notes)?,
                    description: source.protected_locale("Description", projection, notes)?,
                })
            })?,
        })
    }
}

impl ItemPhysics {
    fn read(
        source: Observed<'_, '_, '_, '_>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            mass: source.single("Mass", notes)?,
        })
    }
}

impl<'source> WeaponAttachmentMotion<'source> {
    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Option<Self>, CatalogError> {
        let raw = source.structure()?.field("Type");
        let Some(Value::Enum(name) | Value::String(name)) = raw else {
            return Ok(None);
        };
        if !name.eq_ignore_ascii_case("WeaponPersonal") {
            return Ok(None);
        }
        let item_type = if matches!(raw, Some(Value::Enum(_))) {
            source.borrowed_declared_enum("Type", notes)?
        } else {
            source.borrowed_string("Type", notes)?
        };
        Ok(Some(Self {
            item_type,
            tags: source.borrowed_string("Tags", notes)?,
        }))
    }
}
