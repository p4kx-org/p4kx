// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceValueState, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::super::super::super::authored_fields::Observed;
use crate::CatalogError;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::{SourceArenaOutput, SourceRecordReference};

const INPUT_TYPE: &str = "AbstractMissionInitParam";

/// One ordered original mission-module input.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct MissionModuleInput<'source> {
    source_type: SourcedValue<&'source str>,
    name: SourcedValue<&'source str>,
    #[serde(flatten)]
    payload: InputPayload<'source>,
    authored_fields: SourceArenaOutput<'source>,
}

impl<'source> MissionModuleInput<'source> {
    /// Borrows the original concrete input type.
    #[must_use]
    pub const fn source_type(&self) -> &SourcedValue<&'source str> {
        &self.source_type
    }

    /// Borrows the original input name.
    #[must_use]
    pub const fn name(&self) -> &SourcedValue<&'source str> {
        &self.name
    }

    /// Borrows the existing typed input interpretation.
    #[must_use]
    pub const fn payload(&self) -> &InputPayload<'source> {
        &self.payload
    }

    /// Borrows retained original input fields.
    #[must_use]
    pub const fn authored_fields(&self) -> &SourceArenaOutput<'source> {
        &self.authored_fields
    }

    pub(super) fn read_array(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<SourcedValue<Vec<SourcedValue<Self>>>, CatalogError> {
        fields.strong_pointer_family_array_to("missionInputs", INPUT_TYPE, notes, Self::read)
    }

    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let source_type = fields.source_type_value()?;
        let name = fields.borrowed_string("name", notes)?;
        let payload = InputPayload::read(fields, notes)?;
        let authored_fields = fields.authored_members_except(
            std::iter::once(("name", name.state())).chain(payload.projected()),
        )?;
        Ok(Self {
            source_type,
            name,
            payload,
            authored_fields,
        })
    }
}

/// The existing interpretation of one original mission-module input.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub enum InputPayload<'source> {
    /// An original Boolean input.
    Boolean {
        /// The original value with its existing state and evidence.
        value: SourcedValue<bool>,
    },
    /// An original signed integer input.
    Integer {
        /// The original value with its existing state and evidence.
        value: SourcedValue<i32>,
    },
    /// An original Single input.
    Float {
        /// The original value with its existing state and evidence.
        value: SourcedValue<ResourceFloat32>,
    },
    /// An original activity input.
    Activity {
        /// The original value with its existing state and evidence.
        value: SourcedValue<&'source str>,
    },
    /// An original Tag-term input.
    Tag {
        /// The original value with its existing state and evidence.
        value: SourcedValue<MissionInputTags<'source>>,
    },
    /// The original abstract base input.
    Base,
    /// An additional concrete input retained in its source arena.
    Authored,
}

impl<'source> InputPayload<'source> {
    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(match fields.source_type()? {
            "MissionInitParamBoolean" => Self::Boolean {
                value: fields.boolean("value", notes)?,
            },
            "MissionInitParamInteger" => Self::Integer {
                value: fields.int32("value", notes)?,
            },
            "MissionInitParamFloat" => Self::Float {
                value: fields.single("value", notes)?,
            },
            "MissionInitParamActivity" => Self::Activity {
                value: fields.borrowed_string("value", notes)?,
            },
            "MissionInitParamTag" => Self::Tag {
                value: fields.declared_class_to(
                    "value",
                    "TagSearchTerm",
                    notes,
                    MissionInputTags::read,
                )?,
            },
            INPUT_TYPE => Self::Base,
            _ => {
                fields.note_retained_variant(notes)?;
                Self::Authored
            }
        })
    }

    const fn projected(&self) -> Option<(&'static str, SourceValueState)> {
        let state = match self {
            Self::Boolean { value } => value.state(),
            Self::Integer { value } => value.state(),
            Self::Float { value } => value.state(),
            Self::Activity { value } => value.state(),
            Self::Tag { value } => value.state(),
            Self::Base | Self::Authored => return None,
        };
        Some(("value", state))
    }
}

type TagLinks<'source> = SourcedValue<Vec<SourcedValue<Box<SourceRecordReference<'source>>>>>;

/// Original positive and negative Tag-reference operands for one input.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct MissionInputTags<'source> {
    #[schemars(with = "SourcedValue<Vec<SourcedValue<CatalogRecordReference>>>")]
    positive_tags: TagLinks<'source>,
    #[schemars(with = "SourcedValue<Vec<SourcedValue<CatalogRecordReference>>>")]
    negative_tags: TagLinks<'source>,
    authored_fields: SourceArenaOutput<'source>,
}

impl<'source> MissionInputTags<'source> {
    /// Borrows ordered original positive Tag-reference slots.
    #[must_use]
    pub const fn positive_tags(
        &self,
    ) -> &SourcedValue<Vec<SourcedValue<Box<SourceRecordReference<'source>>>>> {
        &self.positive_tags
    }

    /// Borrows ordered original negative Tag-reference slots.
    #[must_use]
    pub const fn negative_tags(
        &self,
    ) -> &SourcedValue<Vec<SourcedValue<Box<SourceRecordReference<'source>>>>> {
        &self.negative_tags
    }

    /// Borrows additional original fields in source order.
    #[must_use]
    pub const fn authored_fields(&self) -> &SourceArenaOutput<'source> {
        &self.authored_fields
    }

    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let positive_tags = fields.record_link_array_to("positiveTags", "Tag", notes)?;
        let negative_tags = fields.record_link_array_to("negativeTags", "Tag", notes)?;
        let authored_fields = fields.authored_members_except([
            ("positiveTags", positive_tags.state()),
            ("negativeTags", negative_tags.state()),
        ])?;
        Ok(Self {
            positive_tags,
            negative_tags,
            authored_fields,
        })
    }
}
