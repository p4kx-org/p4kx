// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceValueState, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use crate::CatalogError;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::catalog_text_contract::GameTextRef;
use crate::guid_text::CatalogGuid;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::{SourceArenaOutput, SourceRecordReference};

const BASE: &str = "SPerkParamsBase";

type ProjectedField = (&'static str, SourceValueState);

/// One physically admitted owned perk with the original common and concrete fields.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct ReputationPerk<'source> {
    source_type: SourcedValue<&'source str>,
    #[serde(flatten)]
    presentation: PerkPresentation<'source>,
    details: PerkDetails<'source>,
    authored_fields: SourceArenaOutput<'source>,
}

impl<'source> ReputationPerk<'source> {
    pub(super) fn read_field(
        fields: Observed<'_, '_, '_, 'source>,
        projection: &CatalogProjection<'_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<SourcedValue<Self>, CatalogError> {
        fields.strong_pointer_family_to("perk", BASE, notes, |perk, notes| {
            Self::read(perk, projection, notes)
        })
    }

    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        projection: &CatalogProjection<'_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let presentation = PerkPresentation::read(fields, projection, notes)?;
        let details = PerkDetails::read(fields, notes)?;
        let authored_fields = fields.authored_members_except(
            presentation
                .projected()
                .into_iter()
                .chain(
                    details
                        .category()
                        .into_iter()
                        .flat_map(PerkCategory::projected),
                )
                .chain(details.projected_member()),
        )?;
        Ok(Self {
            source_type: fields.source_type_value()?,
            presentation,
            details,
            authored_fields,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct PerkPresentation<'source> {
    name: SourcedValue<&'source str>,
    #[schemars(extend("x-gameText" = true))]
    description: SourcedValue<&'source str>,
    #[schemars(extend("x-gameText" = true))]
    display_name: SourcedValue<GameTextRef>,
    #[schemars(extend("x-gameText" = true))]
    display_description: SourcedValue<GameTextRef>,
    icon: SourcedValue<&'source str>,
}

impl<'source> PerkPresentation<'source> {
    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        projection: &CatalogProjection<'_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            name: fields.borrowed_string("name", notes)?,
            description: fields.borrowed_string("description", notes)?,
            display_name: fields.protected_locale("displayName", projection, notes)?,
            display_description: fields.protected_locale(
                "displayDescription",
                projection,
                notes,
            )?,
            icon: fields.borrowed_string("icon", notes)?,
        })
    }

    const fn projected(&self) -> [ProjectedField; 5] {
        [
            ("name", self.name.state()),
            ("description", self.description.state()),
            ("displayName", self.display_name.state()),
            ("displayDescription", self.display_description.state()),
            ("icon", self.icon.state()),
        ]
    }
}

/// Source discriminator and the directly authored fields of that alternative.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[schemars(transform = every_field_required)]
enum PerkDetails<'source> {
    Base,
    Item {
        #[schemars(with = "SourcedValue<CatalogRecordReference>")]
        reward_item: SourcedValue<Box<SourceRecordReference<'source>>>,
    },
    ShopItemCategory {
        category: Box<PerkCategory<'source>>,
    },
    EnabledItemCategory {
        category: Box<PerkCategory<'source>>,
    },
    DiscountItemCategory {
        category: Box<PerkCategory<'source>>,
        discount_percentage: SourcedValue<ResourceFloat32>,
    },
    Authored,
}

impl<'source> PerkDetails<'source> {
    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(match fields.source_type()? {
            BASE => Self::Base,
            "SPerkParamsItem" => Self::Item {
                reward_item: fields.record_link_to("rewardItem", "EntityClassDefinition", notes)?,
            },
            "SPerkShopItemCategoryBase" => Self::ShopItemCategory {
                category: Box::new(PerkCategory::read(fields, notes)?),
            },
            "SPerkEnabledItemCategory" => Self::EnabledItemCategory {
                category: Box::new(PerkCategory::read(fields, notes)?),
            },
            "SPerkDiscountItemCategory" => Self::DiscountItemCategory {
                category: Box::new(PerkCategory::read(fields, notes)?),
                discount_percentage: fields.single("discountPercentage", notes)?,
            },
            _ => {
                fields.note_retained_variant(notes)?;
                Self::Authored
            }
        })
    }

    fn category(&self) -> Option<&PerkCategory<'source>> {
        match self {
            Self::ShopItemCategory { category }
            | Self::EnabledItemCategory { category }
            | Self::DiscountItemCategory { category, .. } => Some(category),
            Self::Base | Self::Item { .. } | Self::Authored => None,
        }
    }

    const fn projected_member(&self) -> Option<ProjectedField> {
        match self {
            Self::Item { reward_item } => Some(("rewardItem", reward_item.state())),
            Self::DiscountItemCategory {
                discount_percentage,
                ..
            } => Some(("discountPercentage", discount_percentage.state())),
            _ => None,
        }
    }
}

/// Original selectors for a shop item category.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct PerkCategory<'source> {
    id: SourcedValue<CatalogGuid>,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    franchise: SourcedValue<Box<SourceRecordReference<'source>>>,
    #[serde(rename = "type")]
    item_type: SourcedValue<&'source str>,
    sub_type: SourcedValue<&'source str>,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    required_tag: SourcedValue<Box<SourceRecordReference<'source>>>,
}

impl<'source> PerkCategory<'source> {
    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            id: fields.guid("id", notes)?,
            franchise: fields.record_link_to("franchise", "ShopFranchise", notes)?,
            item_type: fields.borrowed_enum_to("type", "EItemType", notes)?,
            sub_type: fields.borrowed_enum_to("subType", "EItemSubType", notes)?,
            required_tag: fields.record_link_to("requiredTag", "Tag", notes)?,
        })
    }

    const fn projected(&self) -> [ProjectedField; 5] {
        [
            ("id", self.id.state()),
            ("franchise", self.franchise.state()),
            ("type", self.item_type.state()),
            ("subType", self.sub_type.state()),
            ("requiredTag", self.required_tag.state()),
        ]
    }
}
