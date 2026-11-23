// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;

use p4kx_extraction::{DataType, RecordTree, SourceUseRole, SourceValue, SourcedValue, Value};
use schemars::JsonSchema;
use serde::Serialize;

use super::Observed;
use super::item_attachment::{self, AttachmentDefinition};
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_number_capture::CapturedNumber;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_read::original::{OriginalFlowFields, OriginalValueFact, OriginalValuePart};
use crate::catalog_text_contract::{GameTextRef, PublicNameRef};
use crate::catalog_v3::authored_fields::authored_manufacturer::AuthoredManufacturer;
use crate::catalog_v3::authored_fields::locale_input::AuthoredLocaleInput;
use crate::guid_text::{CatalogGuid, GuidText};
use crate::resource_float::ResourceFloat32;
use crate::resource_number::ResourceNumber;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::template_field_capture::TemplateFieldCapture;
use crate::{CatalogError, ExportAccountingError};

#[path = "item_localization_read.rs"]
mod localization_read;
#[path = "item_definition_staging.rs"]
mod staging;
#[path = "item_definition_views.rs"]
mod views;

pub(crate) use self::staging::{ItemDefinitionStaging, StagedItemDefinitions};

const PHYSICS: &str = "SEntityPhysicsControllerParams";
pub(super) const PURCHASABLE: &str = "SCItemPurchasableParams";

/// Original Item identity and independently authored presentation and physical fields.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct ItemDefinition<'source> {
    #[serde(flatten)]
    header: CanonicalRecordHeader<'source>,
    components: SourcedValue<Vec<ItemDefinitionComponent<'source>>>,
}

/// One original presentation component at its physical Item component position.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct ItemDefinitionComponent<'source> {
    component_ordinal: SourcedValue<u32>,
    source_type: SourcedValue<&'source str>,
    #[serde(flatten)]
    body: ItemDefinitionComponentKind<'source>,
}

/// Typed original presentation values selected from an Item component.
#[derive(Debug, Serialize, JsonSchema)]
#[non_exhaustive]
#[serde(tag = "kind", content = "value", rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub enum ItemDefinitionComponentKind<'source> {
    /// The original attachment classification and presentation fields.
    Attachment(SourcedValue<Box<ItemAttachmentPresentation<'source>>>),
    /// The original physics component fields.
    Physics(SourcedValue<ItemPhysics>),
    /// The original purchasable component fields.
    Purchasable(ItemPurchasable<'source>),
}

/// Independently authored attachment display and manufacturer values.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct ItemAttachmentPresentation<'source> {
    item_type: SourcedValue<&'source str>,
    item_subtype: SourcedValue<&'source str>,
    size: SourcedValue<ResourceNumber>,
    tags: SourcedValue<&'source str>,
    required_tags: SourcedValue<&'source str>,
    grade: SourcedValue<ResourceNumber>,
    manufacturer: AuthoredManufacturer<'source>,
    localization: SourcedValue<ItemLocalization<'source>>,
    weapon_motion: Option<WeaponAttachmentMotion<'source>>,
}

/// Separate public-name and protected-description references from an attachment.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct ItemLocalization<'source> {
    name: SourcedValue<PublicNameRef>,
    #[schemars(extend("x-gameText" = true))]
    description: SourcedValue<GameTextRef>,
    #[serde(skip)]
    #[schemars(skip)]
    name_input: AuthoredLocaleInput<'source>,
    #[serde(skip)]
    #[schemars(skip)]
    description_input: AuthoredLocaleInput<'source>,
}

/// Original physical mass from an Item physics declaration.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct ItemPhysics {
    mass: SourcedValue<ResourceFloat32>,
}

/// Original public display name and protected display type of a purchasable Item.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct ItemPurchasable<'source> {
    display_name: SourcedValue<PublicNameRef>,
    #[schemars(extend("x-gameText" = true))]
    display_type: SourcedValue<GameTextRef>,
    #[serde(skip)]
    #[schemars(skip)]
    name_input: AuthoredLocaleInput<'source>,
}

/// Original personal-weapon attachment values used by animation joins.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct WeaponAttachmentMotion<'source> {
    item_type: SourcedValue<&'source str>,
    tags: SourcedValue<&'source str>,
}

/// The original sourced attachment cells needed by the weapon animation join.
pub(crate) struct WeaponAttachment<'source> {
    pub(crate) record: SourcedValue<u32>,
    pub(crate) slot: SourcedValue<u32>,
    pub(crate) item_type: SourcedValue<&'source str>,
    pub(crate) tags: SourcedValue<&'source str>,
}

impl<'source> ItemDefinition<'source> {
    pub(crate) fn weapon_attachments(&self) -> impl Iterator<Item = WeaponAttachment<'source>> {
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
                    record: self.header.record_index.clone(),
                    slot: component.component_ordinal.clone(),
                    item_type: source.item_type.clone(),
                    tags: source.tags.clone(),
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

    fn order_key(&self) -> Result<(GuidText, u32), ExportAccountingError> {
        let guid = self.header.guid.value().copied().map(CatalogGuid::text);
        let index = self.header.record_index.value().copied();
        guid.zip(index)
            .ok_or(ExportAccountingError::ConflictingSourceDatum)
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
        item_attachment::COMPONENT => ItemDefinitionComponentKind::Attachment(
            AttachmentDefinition::read(source, notes, |definition, notes| {
                ItemAttachmentPresentation::read(definition, projection, notes).map(Box::new)
            })?,
        ),
        PHYSICS => ItemDefinitionComponentKind::Physics(
            if source.storage_type("PhysType")? == Some(DataType::StrongPointer) {
                source.declared_pointer("PhysType", notes, ItemPhysics::read)?
            } else {
                source.declared_class("PhysType", notes, ItemPhysics::read)?
            },
        ),
        PURCHASABLE => {
            let (display_name, name_input) =
                ItemPurchasable::read_display_name(source, notes, |fact| {
                    projection.public_name_fact(fact)
                })?;
            ItemDefinitionComponentKind::Purchasable(ItemPurchasable {
                display_name,
                display_type: source.protected_locale("displayType", projection, notes)?,
                name_input,
            })
        }
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
        definition: &AttachmentDefinition<'_, '_, '_, 'source>,
        projection: &CatalogProjection<'_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let item_type = definition.item_type(notes)?;
        let tags = definition.tags(notes)?;
        let source = definition.observed();
        let weapon_motion = WeaponAttachmentMotion::read(&item_type, &tags);
        Ok(Self {
            item_type,
            item_subtype: source.borrowed_label("SubType", notes)?,
            size: source.authored_number("Size", notes)?,
            tags,
            required_tags: definition.required_tags(notes)?,
            weapon_motion,
            grade: source.authored_number("Grade", notes)?,
            manufacturer: AuthoredManufacturer::read(source, "Manufacturer", notes)?,
            localization: localization_read::read(source, projection, notes)?,
        })
    }
}

impl<'source> ItemPurchasable<'source> {
    pub(super) fn read_display_name<'scope, 'read, T>(
        source: Observed<'_, 'scope, 'read, 'source>,
        notes: &mut Vec<FieldError>,
        display: impl FnOnce(&OriginalValueFact<'scope, 'read, 'source>) -> Result<T, CatalogError>,
    ) -> Result<(SourcedValue<T>, AuthoredLocaleInput<'source>), CatalogError> {
        source.locale_input("displayName", notes, display)
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
        item_type: &SourcedValue<&'source str>,
        tags: &SourcedValue<&'source str>,
    ) -> Option<Self> {
        item_type
            .value()
            .is_some_and(|name| name.eq_ignore_ascii_case("WeaponPersonal"))
            .then(|| Self {
                item_type: item_type.clone(),
                tags: tags.clone(),
            })
    }
}
