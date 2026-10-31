// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, DataForge, Filter, RecordTree, StructValue, TagTree};
use schemars::JsonSchema;
use serde::Serialize;

use crate::CatalogError;
use crate::blueprint::as_struct;
use crate::contract::{LocalisedText, every_field_required};
use crate::guid_text::CatalogGuid;
use crate::mission_layers::{items, key, one};
use crate::record_fields::{boolean, class_name, reference};
use crate::record_read::{KindRead, read_kind};
use crate::selection::Kind;
use crate::spawn_model::{TagPaths, TagTerm};
use crate::spawn_read::Reader;
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
pub(crate) type LocationTemplates<'f> = KindRead<MissionLocationTemplate<'f>>;

/// Reads the `MissionLocationTemplate` Records `filter` matches, ordered by GUID; a development
/// leftover is left out and counted.
///
/// # Errors
///
/// Returns [`CatalogError::Cancelled`] once `cancel` is cancelled.
pub(crate) fn read<'f>(
    forge: &'f DataForge<'f>,
    texts: &Texts<'f>,
    tags: &TagTree,
    filter: Option<&Filter>,
    cancel: Option<&CancelToken>,
) -> Result<LocationTemplates<'f>, CatalogError> {
    read_kind(
        forge,
        Kind::MissionLocationTemplates,
        (filter, cancel),
        |tree| Ok(template(tree, texts, tags)),
        |template| template.guid,
    )
}

fn template<'f>(
    tree: &RecordTree<'f>,
    texts: &Texts<'f>,
    tags: &TagTree,
) -> MissionLocationTemplate<'f> {
    let reader = Reader::new(tags, tree.root());
    let data = one(Some(tree.root()), "locationData");
    MissionLocationTemplate {
        guid: CatalogGuid(tree.guid()),
        class_name: class_name(tree),
        record_path: tree.record_path(),
        disabled: boolean(data, "disabled"),
        is_security_network_host: boolean(data, "isSecurityNetworkHost"),
        tags: reader.tag_list(data, "generalTags"),
        produces_tags: term(&reader, data, "producesTags"),
        consumes_tags: term(&reader, data, "consumesTags"),
        ai_spawn_tags: reader.tag_list(data, "aiSpawnTags"),
        names: names(&reader, texts, data),
        entity_cluster_member_guid: reference(data, "entityClusterMember"),
    }
}

fn term(reader: &Reader<'_, '_>, data: Option<&StructValue<'_>>, name: &str) -> TagTerm {
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
