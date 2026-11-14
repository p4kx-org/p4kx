// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use crate::CatalogError;
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::{SourceArenaOutput, SourceRecordReference};

/// An authored mission location class and the ordered scopes it exposes.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct LocationClassDefinition<'source> {
    #[serde(flatten)]
    source: CanonicalRecordHeader<'source>,
    scopes_visible_to: SourcedValue<Vec<SourcedValue<&'source str>>>,
    authored_fields: SourceArenaOutput<'source>,
}

impl<'source> LocationClassDefinition<'source> {
    pub(super) fn read(
        source: CanonicalRecordHeader<'source>,
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let scopes_visible_to =
            fields.borrowed_enum_array_to("scopesVisibleTo", "ContractScope", notes)?;
        let authored_fields =
            fields.authored_members_except([("scopesVisibleTo", scopes_visible_to.state())])?;
        Ok(Self {
            source,
            scopes_visible_to,
            authored_fields,
        })
    }
}

/// An authored mission location dictionary entry identified by its physical Record.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct LocationDictionaryEntry<'source> {
    #[serde(flatten)]
    source: CanonicalRecordHeader<'source>,
    authored_fields: SourceArenaOutput<'source>,
}

impl<'source> LocationDictionaryEntry<'source> {
    pub(super) fn read(
        source: CanonicalRecordHeader<'source>,
        fields: Observed<'_, '_, '_, 'source>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            source,
            authored_fields: fields.authored_members()?,
        })
    }
}

/// A physical mission location register with its original member occurrences.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct LocationRegistry<'source> {
    #[serde(flatten)]
    source: CanonicalRecordHeader<'source>,
    #[schemars(with = "SourcedValue<Vec<SourcedValue<CatalogRecordReference>>>")]
    members: SourcedValue<Vec<SourcedValue<Box<SourceRecordReference<'source>>>>>,
    authored_fields: SourceArenaOutput<'source>,
}

impl<'source> LocationRegistry<'source> {
    pub(super) fn read(
        source: CanonicalRecordHeader<'source>,
        fields: Observed<'_, '_, '_, 'source>,
        member: (&'static str, &'static str),
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let (name, target) = member;
        let members = fields.record_link_array_to(name, target, notes)?;
        let authored_fields = fields.authored_members_except([(name, members.state())])?;
        Ok(Self {
            source,
            members,
            authored_fields,
        })
    }
}
