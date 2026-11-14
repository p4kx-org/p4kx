// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;

use p4kx_extraction::{
    CancelToken, DataForge, Filter, RecordTree, SourcedValue, StructValue, TagTree,
};
use schemars::JsonSchema;
use serde::Serialize;

use crate::CatalogError;
use crate::blueprint::as_struct;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_number_capture::CapturedNumber;
use crate::catalog_read::original::OriginalFlowFields;
use crate::catalog_v3::location_classification::configuration::{
    self, MissionLocationConfiguration,
};
use crate::catalog_v3::location_classification::{self, LocationClassification};
use crate::contract::{LocalisedText, every_field_required};
use crate::guid_text::CatalogGuid;
use crate::mission_layers::{items, key, one};
use crate::record_fields::{boolean, class_name, reference};
use crate::record_observation::RecordObservation;
use crate::record_read::{KindRead, ReadError, RecordProjection, read_kind_with_notes};
use crate::selection::Kind;
use crate::spawn_model::{TagPaths, TagTerm};
use crate::spawn_read::Reader;
use crate::template_field_capture::TemplateFieldCapture;
use crate::texts::Texts;

/// A mission location template: a `MissionLocationTemplate` Record under
/// `missiondata/pu_locations`, a place a location query can match by its tags.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct MissionLocationTemplate<'a> {
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
    pub(crate) tags: TagPaths,
    /// The `producesTags`.
    pub(crate) produces_tags: TagTerm,
    /// The `consumesTags`.
    pub(crate) consumes_tags: TagTerm,
    /// The `aiSpawnTags`.
    pub(crate) ai_spawn_tags: TagPaths,
    /// The `stringVariants`: the texts the location offers a mission, each under the tag that
    /// names its use.
    pub(crate) names: Vec<LocationName<'a>>,
    /// The `entityClusterMember` Record, `null` when absent.
    pub(crate) entity_cluster_member_guid: Option<CatalogGuid>,
}

/// A string variant of a location template.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct LocationName<'a> {
    /// The tag path of the variant, e.g. `Missions.Text.Name`; `null` when it names none.
    pub(crate) tag: Option<String>,
    /// The text of the variant.
    #[schemars(extend("x-gameText" = true))]
    pub(crate) display_name: LocalisedText<'a>,
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
    input: &OriginalCatalogInput<'_, 'f>,
    texts: &Texts<'f>,
    tags: &TagTree,
    filter: Option<&Filter>,
    cancel: Option<&CancelToken>,
) -> Result<LocationTemplates<'f>, CatalogError> {
    read_kind_with_notes(
        forge,
        input,
        Kind::MissionLocationTemplates,
        (filter, cancel),
        |admission, tree| template(input, admission, tree, texts, tags, cancel),
        |template| template.guid,
    )
}

fn template<'f>(
    input: &OriginalCatalogInput<'_, 'f>,
    admission: &RecordObservation<'_, 'f>,
    tree: &RecordTree<'f>,
    texts: &Texts<'f>,
    tags: &TagTree,
    cancel: Option<&CancelToken>,
) -> Result<RecordProjection<MissionLocationTemplate<'f>>, ReadError> {
    let scope = input.record_scope(admission.record(), tree)?;
    let fields = OriginalFlowFields::read(&scope, cancel)?;
    let capture = TemplateFieldCapture::new(&fields, input, admission)?;
    let reader = Reader::for_observed(tags, tree, &capture)?;
    let data = one(Some(tree.root()), "locationData");
    let mut notes = Vec::new();
    let classification = location_classification::read(&fields, input, &capture, &mut notes)?;
    let numbers = RefCell::default();
    let configuration = configuration::read(&fields, input, &capture, &numbers, &mut notes)?;
    let value = MissionLocationTemplate {
        guid: CatalogGuid(tree.guid()),
        class_name: class_name(tree),
        record_path: tree.record_path(),
        disabled: boolean(data, "disabled"),
        classification,
        configuration,
        configuration_numbers: numbers.into_inner(),
        is_security_network_host: boolean(data, "isSecurityNetworkHost"),
        tags: reader.tag_list(data, "generalTags"),
        produces_tags: term(&reader, data, "producesTags"),
        consumes_tags: term(&reader, data, "consumesTags"),
        ai_spawn_tags: reader.tag_list(data, "aiSpawnTags"),
        names: names(&reader, texts, data),
        entity_cluster_member_guid: reference(data, "entityClusterMember"),
    };
    Ok(RecordProjection { value, notes })
}

fn term<'source>(
    reader: &Reader<'_, 'source>,
    data: Option<&StructValue<'source>>,
    name: &str,
) -> TagTerm {
    let term = one(data, name);
    TagTerm {
        positive_tags: reader.tag_array(term, "positiveTags"),
        negative_tags: reader.tag_array(term, "negativeTags"),
    }
}

fn names<'f>(
    reader: &Reader<'_, 'f>,
    texts: &Texts<'f>,
    data: Option<&StructValue<'f>>,
) -> Vec<LocationName<'f>> {
    let variants = one(data, "stringVariants");
    items(variants, "variants")
        .iter()
        .filter_map(as_struct)
        .map(|variant| LocationName {
            tag: reader.tag_path(Some(variant), "tag"),
            display_name: texts.localised(key(Some(variant), "string")),
        })
        .collect()
}
