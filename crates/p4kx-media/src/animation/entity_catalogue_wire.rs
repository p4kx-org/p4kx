// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::ClipDatabaseUnavailable;
use p4kx_extraction::{FieldPath, InputScope, SourceLocation, SourceUseRole};
use schemars::JsonSchema;
use serde::Serialize;
use serde_json::{Value, json};

use crate::animation::origin;
use crate::origin_capture::OriginSite;
use crate::origin_contract::OriginProfile;
use crate::origin_contributions::OriginContributions;
use crate::origin_input::MediaOriginInput;
use crate::origin_prepared::PreparedRecordOrigins;
use crate::{MediaError, MediaIssue, MediaWriter, ReferenceTarget, Severity, Status, Subresource};

pub(crate) const CATALOGUE: &str = "animation-entity-catalogue";
pub(super) const RIG: &str = "animation-entity-rig";
pub(super) const PARAMETER: &str = "animation-entity-parameter";
pub(super) const LINK: &str = "animation-entity-source-link";
pub(super) const SOURCE: &str = "animation-entity-source";
pub(super) const CLIP: &str = "animation-entity-clip";
pub(super) const BANK: &str = "animation-entity-named-dba-bank";
pub(super) const BANK_MATCH: &str = "animation-entity-named-dba-match";

#[path = "entity_catalogue_project.rs"]
mod project;
pub(super) use project::Rows;

#[derive(Serialize, JsonSchema)]
#[serde(
    tag = "record_type",
    content = "payload",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
pub(super) enum Payload<'a> {
    #[serde(rename = "entity-catalogue")]
    Catalogue(Catalogue<'a>),
    #[serde(rename = "entity-rig")]
    Rig(Rig<'a>),
    #[serde(rename = "entity-parameter")]
    Parameter(Parameter<'a>),
    #[serde(rename = "entity-source-link")]
    Link(Link),
    #[serde(rename = "entity-source")]
    Source(Source<'a>),
    #[serde(rename = "entity-clip")]
    Clip(Clip<'a>),
    #[serde(rename = "entity-named-dba-bank")]
    NamedDbaBank(Bank<'a>),
    #[serde(rename = "entity-named-dba-match")]
    NamedDbaMatch(BankMatch<'a>),
}

#[derive(Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[schemars(rename = "AnimationEntityCatalogueState")]
pub(crate) enum CatalogueState {
    Available,
    MissingDataforge,
    DataforgeNotSelected,
    DataforgeUnavailable,
    InvalidDataforge,
    IncompleteLoadout,
}

impl CatalogueState {
    pub(super) fn issue(self) -> Option<MediaIssue> {
        let detail = match self {
            Self::Available => return None,
            Self::MissingDataforge => "root Archive has no DataForge Entry",
            Self::DataforgeNotSelected => "root DataForge is outside the Media input selection",
            Self::DataforgeUnavailable => "root DataForge has no completed input receipt",
            Self::InvalidDataforge => "root DataForge cannot be decoded",
            Self::IncompleteLoadout => "some original Loadout positions could not be resolved",
        };
        Some(MediaIssue::new(
            "animation.entity-catalogue",
            "scope",
            Severity::Warning,
            detail,
        ))
    }
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "AnimationEntityCatalogue")]
pub(super) struct Catalogue<'a> {
    pub(super) selector: &'a str,
    pub(super) state: CatalogueState,
    pub(super) owner: Option<Owner>,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "AnimationEntityOwner")]
pub(super) struct Owner {
    pub(super) guid: String,
    pub(super) record: Option<u32>,
}

impl Owner {
    pub(super) const fn new(guid: String, record: u32) -> Self {
        Self {
            guid,
            record: Some(record),
        }
    }
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "AnimationEntityRig")]
pub(super) struct Rig<'a> {
    pub(super) owner: Owner,
    pub(super) placement: Placement<'a>,
    pub(super) geometry: Option<&'a str>,
    pub(super) definition: Option<Attempt<'a>>,
    pub(super) skeleton: Option<Attempt<'a>>,
    pub(super) parameters: Option<Attempt<'a>>,
}

#[derive(Serialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
#[schemars(rename = "AnimationEntityPlacement")]
pub(super) enum Placement<'a> {
    Root,
    Attachment {
        port_path: &'a str,
    },
    LandingGear {
        owner_path: &'a str,
        ordinal: usize,
        bone: Option<&'a str>,
    },
    Unknown,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "AnimationEntityAttempt")]
pub(super) struct Attempt<'a> {
    pub(super) path: &'a str,
    pub(super) status: ReadStatus,
    pub(super) primary_chunk: Option<usize>,
}

#[derive(Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[schemars(rename = "AnimationEntityReadStatus")]
pub(super) enum ReadStatus {
    Missing,
    Ambiguous,
    NotSelected,
    KeyRequired,
    WrongKey,
    Unreadable,
    Malformed,
    Partial,
    Read,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "AnimationEntityParameter")]
pub(super) struct Parameter<'a> {
    pub(super) list: usize,
    pub(super) ordinal: usize,
    pub(super) name: &'a str,
    pub(super) path: &'a str,
    pub(super) positions: &'a [SourceLocation],
}

#[derive(Serialize, JsonSchema)]
#[serde(tag = "disposition", rename_all = "kebab-case", deny_unknown_fields)]
#[schemars(rename = "AnimationEntityLink")]
pub(super) enum Link {
    FolderCandidate {
        source: usize,
    },
    FolderEmpty,
    FolderUnavailable,
    Database {
        source: usize,
    },
    NamedCaf {
        source: usize,
        filepath_parameter: Option<usize>,
    },
    Unable {
        reason: SelectionFailure,
        filepath_parameter: Option<usize>,
    },
    Unexpanded,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[schemars(rename = "AnimationEntitySelectionFailure")]
pub(super) enum SelectionFailure {
    InvalidPath,
    Missing,
    Ambiguous,
    MissingScope,
    Unreadable,
}

#[path = "entity_catalogue_bank_wire.rs"]
mod bank_wire;
use bank_wire::{Bank, BankMatch, BankState};

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "AnimationEntitySource")]
pub(super) struct Source<'a> {
    pub(super) entry: usize,
    pub(super) directory_ordinal: Option<p4kx_extraction::DecimalU64>,
    pub(super) path: &'a str,
    pub(super) status: ReadStatus,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "AnimationEntityClip")]
pub(super) struct Clip<'a> {
    pub(super) metadata_range: crate::SourceRange,
    pub(super) metadata_version: u32,
    pub(super) flags: u32,
    pub(super) frames_per_second: Option<u16>,
    pub(super) end_frame: Option<u32>,
    pub(super) name_bytes: Option<&'a [u8]>,
    pub(super) controller_count: Option<usize>,
    #[schemars(schema_with = "optional_reference_target")]
    pub(super) physical_record: Option<ReferenceTarget>,
}

pub(super) struct Scope(PreparedRecordOrigins);

impl Scope {
    pub(super) fn new(writer: &MediaWriter<'_>) -> Self {
        let mut origins = writer.record_origins();
        origins.set_profile(OriginProfile::Animation);
        Self(origins)
    }

    pub(super) fn at(
        &mut self,
        field: &str,
        proof: OriginContributions,
        writer: &MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        origin::at(
            &mut self.0,
            &format!("/data/payload/{field}"),
            proof,
            writer,
        )
    }

    pub(super) fn clip_reference(
        &mut self,
        target: &ReferenceTarget,
        input: &MediaOriginInput,
        clip: OriginContributions,
        writer: &MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let target_path = FieldPath::parse("/data/payload/physical_record")
            .map_err(|cause| crate::origin_error::address(&cause))?;
        let independent = OriginContributions::independent(&writer.origins.owner);
        crate::origin_prepared_source::identity(
            &mut self.0,
            &crate::origin_reference::child(&target_path, "source")?,
            &target.source,
            input,
            &independent,
            writer,
        )?;
        let mut root = writer.capture_origins();
        if let Some(original) = &writer.origins.root {
            root.add(
                original,
                InputScope::Payload,
                OriginSite::new("media-physical-input", &[], "complete-archive-file"),
                SourceUseRole::Conversion,
                writer,
            )?;
        }
        self.at("physical_record/root/hex", root.finish(), writer)?;
        for field in [
            "kind",
            "root/algorithm",
            "root/scope",
            "subresource/namespace",
        ] {
            self.at(
                &format!("physical_record/{field}"),
                independent.clone(),
                writer,
            )?;
        }
        self.at("physical_record/subresource/indices", clip, writer)
    }

    pub(super) fn health(
        &mut self,
        proof: OriginContributions,
        writer: &MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        origin::at(&mut self.0, "/issues/0", proof, writer)
    }

    pub(super) fn write(
        mut self,
        namespace: &str,
        indices: Vec<u64>,
        payload: &Payload<'_>,
        issues: &[MediaIssue],
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let independent = OriginContributions::independent(&writer.origins.owner);
        origin::at(&mut self.0, "/data/record_type", independent, writer)?;
        self.0.write(
            writer,
            &Subresource::new(namespace, indices),
            if issues.is_empty() {
                Status::Complete
            } else {
                Status::Partial
            },
            payload,
            issues,
        )
    }
}

fn optional_reference_target(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
    schemars::json_schema!({"anyOf": [{"$ref": "#/$defs/ReferenceTarget"}, {"type": "null"}]})
}

pub(crate) fn bind_schema(base: &mut Value) -> Result<(), MediaError> {
    const NAME: &str = "AnimationEntityCataloguePayload";
    crate::schema_merge::insert_payload(
        base,
        NAME,
        crate::schema_merge::serialized_payload::<Payload<'static>>(),
    )?;
    let branches = base
        .get_mut("allOf")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| MediaError::Contract("media schema has no built-in constraints".into()))?;
    for (namespace, record_type, indices) in [
        (CATALOGUE, "entity-catalogue", 0),
        (RIG, "entity-rig", 1),
        (PARAMETER, "entity-parameter", 2),
        (LINK, "entity-source-link", 3),
        (SOURCE, "entity-source", 1),
        (CLIP, "entity-clip", 2),
        (BANK, "entity-named-dba-bank", 3),
        (BANK_MATCH, "entity-named-dba-match", 4),
    ] {
        branches.push(json!({
            "if":{"properties":{"kind":{"const":"animation"},"subresource":{"properties":{"namespace":{"const":namespace}},"required":["namespace"]}},"required":["kind","subresource"]},
            "then":{"properties":{"data":{"allOf":[{"$ref":format!("#/$defs/{NAME}")},{"properties":{"record_type":{"const":record_type}},"required":["record_type"]}]},"subresource":{"properties":{"indices":{"minItems":indices,"maxItems":indices}}}},"required":["data"]}
        }));
    }
    Ok(())
}
