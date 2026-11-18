// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;

use p4kx_extraction::{
    CancelToken, DataForge, Filter, Guid, RecordTree, SourcedValue, StructValue,
};
use schemars::JsonSchema;
use serde::Serialize;

use crate::CatalogError;
use crate::ExportAccountingError;
use crate::blueprint::as_struct;
use crate::catalog_number_capture::CapturedNumber;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_read::original::OriginalFlowFields;
use crate::catalog_text_contract::GameTextRef;
use crate::catalog_v3::location_classification::configuration::{
    self, MissionLocationConfiguration,
};
use crate::catalog_v3::location_classification::{self, LocationClassification};
use crate::catalog_v3::tag_catalog::CapturedTags;
use crate::contract::every_field_required;
use crate::guid_text::CatalogGuid;
use crate::mission_layers::{items, one};
use crate::mission_location_tag::{MissionLocationTag, MissionLocationTagTerm, Reader};
use crate::record_fields::{boolean, class_name, reference};
use crate::record_observation::RecordObservation;
use crate::record_read::{KindRead, ReadError, RecordProjection, read_kind_with_notes};
use crate::selection::Kind;
use crate::template_field_capture::TemplateFieldCapture;

/// A mission location template: a `MissionLocationTemplate` Record under
/// `missiondata/pu_locations`, a place a location query can match by its tags.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct MissionLocationTemplate<'a> {
    #[serde(skip)]
    record_index: u32,
    /// The GUID of the Record.
    pub(crate) guid: CatalogGuid,
    /// The Record name without its type, e.g. `AEGS_Reclaimer_Space_Stanton2c`.
    pub(crate) class_name: &'a str,
    /// The record path, without `.xml`.
    pub(crate) record_path: &'a str,
    /// The `disabled` flag of the location data, `null` when absent.
    pub(crate) disabled: Option<bool>,
    /// The original classification of the location data.
    classification: SourcedValue<LocationClassification<'a>>,
    configuration: SourcedValue<MissionLocationConfiguration<'a>>,
    #[serde(skip)]
    pub(crate) configuration_numbers: Vec<CapturedNumber>,
    /// The `isSecurityNetworkHost` flag, `null` when absent.
    pub(crate) is_security_network_host: Option<bool>,
    /// The `generalTags`: what the queries of `contractLocationQueries` match.
    tags: SourcedValue<Vec<MissionLocationTag>>,
    /// The `producesTags`.
    produces_tags: SourcedValue<MissionLocationTagTerm>,
    /// The `consumesTags`.
    consumes_tags: SourcedValue<MissionLocationTagTerm>,
    /// The `aiSpawnTags`.
    ai_spawn_tags: SourcedValue<Vec<MissionLocationTag>>,
    /// The `stringVariants`: the texts the location offers a mission, each under the tag that
    /// names its use.
    pub(crate) names: Vec<LocationName>,
    /// The `entityClusterMember` Record, `null` when absent.
    pub(crate) entity_cluster_member_guid: Option<CatalogGuid>,
}

/// A string variant of a location template.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct LocationName {
    /// The original tag operand for this variant.
    pub tag: MissionLocationTag,
    /// The text of the variant.
    #[schemars(extend("x-gameText" = true))]
    pub display_name: SourcedValue<GameTextRef>,
}

impl<'source> MissionLocationTemplate<'source> {
    /// Borrows the original location dictionary links.
    #[must_use]
    pub const fn classification(&self) -> &SourcedValue<LocationClassification<'source>> {
        &self.classification
    }

    /// Borrows the original location-owned module, spawn and limit settings.
    #[must_use]
    pub const fn configuration(&self) -> &SourcedValue<MissionLocationConfiguration<'source>> {
        &self.configuration
    }

    /// The original Record GUID.
    #[must_use]
    pub const fn guid(&self) -> Guid {
        self.guid.0
    }

    /// The original position of this Record within the DataForge input.
    #[must_use]
    pub const fn record_index(&self) -> u32 {
        self.record_index
    }

    /// The original class name.
    #[must_use]
    pub const fn class_name(&self) -> &'source str {
        self.class_name
    }

    /// The original Record path without its extension.
    #[must_use]
    pub const fn record_path(&self) -> &'source str {
        self.record_path
    }

    /// The authored disabled flag when present.
    #[must_use]
    pub const fn disabled(&self) -> Option<bool> {
        self.disabled
    }

    /// Returns the authored security-network-host flag when present.
    #[must_use]
    pub const fn is_security_network_host(&self) -> Option<bool> {
        self.is_security_network_host
    }

    /// Returns the authored entity-cluster member GUID when present.
    #[must_use]
    pub const fn entity_cluster_member_guid(&self) -> Option<Guid> {
        match self.entity_cluster_member_guid {
            Some(guid) => Some(guid.0),
            None => None,
        }
    }

    /// The general tag operands in source order.
    #[must_use]
    pub const fn tags(&self) -> &SourcedValue<Vec<MissionLocationTag>> {
        &self.tags
    }

    /// The produced tag term.
    #[must_use]
    pub const fn produces_tags(&self) -> &SourcedValue<MissionLocationTagTerm> {
        &self.produces_tags
    }

    /// The consumed tag term.
    #[must_use]
    pub const fn consumes_tags(&self) -> &SourcedValue<MissionLocationTagTerm> {
        &self.consumes_tags
    }

    /// The AI spawn tag operands in source order.
    #[must_use]
    pub const fn ai_spawn_tags(&self) -> &SourcedValue<Vec<MissionLocationTag>> {
        &self.ai_spawn_tags
    }

    /// The authored name variants in source order.
    #[must_use]
    pub fn names(&self) -> &[LocationName] {
        &self.names
    }
}

/// The location templates of a DataForge and what was left out.
pub(crate) type LocationTemplates<'f> = KindRead<'f, MissionLocationTemplate<'f>>;

/// Reads the `MissionLocationTemplate` Records `filter` matches, ordered by GUID; a development
/// leftover is left out and counted.
///
/// # Errors
///
/// Returns [`CatalogError::Cancelled`] once `cancel` is cancelled.
pub(crate) fn read<'f>(
    forge: &'f DataForge<'f>,
    projection: &CatalogProjection<'_, 'f>,
    tags: &CapturedTags<'_, 'f>,
    filter: Option<&Filter>,
    cancel: Option<&CancelToken>,
) -> Result<LocationTemplates<'f>, CatalogError> {
    read_kind_with_notes(
        forge,
        projection.original(),
        Kind::MissionLocationTemplates,
        (filter, cancel, None),
        |admission, tree| template(projection, admission, tree, tags, cancel),
        |template| template.guid,
    )
}

pub(crate) fn template<'f>(
    projection: &CatalogProjection<'_, 'f>,
    admission: &RecordObservation<'_, 'f>,
    tree: &RecordTree<'f>,
    tags: &CapturedTags<'_, 'f>,
    cancel: Option<&CancelToken>,
) -> Result<RecordProjection<MissionLocationTemplate<'f>>, ReadError> {
    let input = projection.original();
    let scope = input.record_scope(admission.record(), tree)?;
    let fields = OriginalFlowFields::read(&scope, cancel)?;
    let capture = TemplateFieldCapture::new(&fields, input, admission)?;
    let reader = Reader::new(&fields, tags);
    let tags = reader.location()?;
    let data = one(Some(tree.root()), "locationData");
    let mut notes = Vec::new();
    let classification = location_classification::read(&fields, input, &capture, &mut notes)?;
    let numbers = RefCell::default();
    let configuration = configuration::read(&fields, input, &capture, &numbers, &mut notes)?;
    let value = MissionLocationTemplate {
        record_index: tree.record_index(),
        guid: CatalogGuid(tree.guid()),
        class_name: class_name(tree),
        record_path: tree.record_path(),
        disabled: boolean(data, "disabled"),
        classification,
        configuration,
        configuration_numbers: numbers.into_inner(),
        is_security_network_host: boolean(data, "isSecurityNetworkHost"),
        tags: tags.tags,
        produces_tags: tags.produces,
        consumes_tags: tags.consumes,
        ai_spawn_tags: tags.ai_spawn,
        names: names(&reader, &fields, projection, data)?,
        entity_cluster_member_guid: reference(data, "entityClusterMember"),
    };
    Ok(RecordProjection { value, notes })
}

fn names<'fields, 'scope, 'read, 'f>(
    reader: &Reader<'_, 'fields, 'scope, 'read, 'f>,
    fields: &'fields OriginalFlowFields<'scope, 'read, 'f>,
    projection: &CatalogProjection<'_, 'f>,
    data: Option<&StructValue<'f>>,
) -> Result<Vec<LocationName>, CatalogError> {
    let variants = one(data, "stringVariants");
    let failures = reader.variant_failures()?;
    items(variants, "variants")
        .iter()
        .filter_map(as_struct)
        .map(|variant| {
            let owner = fields
                .owner_of(variant)?
                .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
            Ok(LocationName {
                tag: reader.variant(owner, &failures)?,
                display_name: projection.protected_key_field(fields, Some(owner), "string")?,
            })
        })
        .collect()
}
