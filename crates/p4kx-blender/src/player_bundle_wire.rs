// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    DecimalU64, FieldOrigin, OriginArtifactDigest, PhysicalSource, PhysicalSourceRef,
    SourceUseDescription,
};
use schemars::JsonSchema;
use serde::Serialize;

use crate::PlayerDocumentRejection;
use crate::player_adb_declarations::Fragment;
use crate::player_rig_clip::{Bank, Clip, ImageInventory, Rig, Rule};
use crate::player_source_body::Body;
use crate::player_source_classes::ClassTarget;
use crate::player_source_loadout::Loadout;
use crate::player_source_motion::{
    BranchFields, BranchOption, ControllerRigOwner, ControllerTopology, DatabaseLink,
    MotionComponentFields, MotionOptionFields, OptionRow, RigCandidate,
};
use crate::player_tag_definitions::{
    AdbBranchSelectors, AdbTagSelectors, CycleEdge, Document, Edge, TagScope, WeaponMotionTag,
};
use crate::player_weapon_motion_sources::{LoadoutLink, Weapon};
use p4kx_extraction::{SourceValueState, SourcedValue};

pub(super) const FORMAT: &str = "p4kx-player-source-bundle";
pub(super) const VERSION: u8 = 1;
pub(super) const RECORDS: &str = "records.jsonl";
pub(super) const MANIFEST: &str = "manifest.json";
pub(super) const ORIGINALS: &str = "originals";

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Manifest<'a> {
    #[schemars(extend("const" = FORMAT))]
    pub(super) format: &'static str,
    #[schemars(extend("const" = VERSION))]
    pub(super) schema_version: u8,
    #[schemars(length(min = 1))]
    pub(super) bodies: &'a [&'static str],
    pub(super) records: Stream,
    pub(super) raw_artifacts: Totals,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Stream {
    #[schemars(extend("const" = RECORDS))]
    pub(super) path: &'static str,
    #[serde(flatten)]
    pub(super) digest: OriginArtifactDigest,
    pub(super) row_count: DecimalU64,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Totals {
    pub(super) count: DecimalU64,
    pub(super) byte_length: DecimalU64,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Artifact {
    #[schemars(regex(pattern = r"^originals/[0-9]+\.bin$"))]
    pub(super) path: String,
    #[serde(flatten)]
    pub(super) digest: OriginArtifactDigest,
    #[schemars(regex(pattern = "^[0-9a-f]{8}$"))]
    pub(super) crc32c: String,
}

#[derive(Serialize, JsonSchema)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub(crate) enum Row<'a> {
    Header {
        #[schemars(extend("const" = FORMAT))]
        format: &'static str,
        #[schemars(extend("const" = VERSION))]
        schema_version: u8,
    },
    Body {
        index: usize,
        body_variant: &'static str,
        value: &'a SourcedValue<Body>,
    },
    Loadout {
        index: usize,
        value: &'a SourcedValue<Loadout>,
    },
    MotionActor {
        value: &'a SourcedValue<ClassTarget>,
    },
    MotionComponents {
        state: SourceValueState,
        #[serde(flatten)]
        origin: &'a FieldOrigin,
    },
    MotionComponent {
        index: usize,
        value: MotionComponentFields<'a>,
    },
    MotionControllerTopology {
        component: usize,
        value: &'a SourcedValue<ControllerRigOwner>,
    },
    ControllerTopology {
        value: &'a ControllerTopology,
    },
    ActorRigCandidates {
        state: SourceValueState,
        #[serde(flatten)]
        origin: &'a FieldOrigin,
    },
    ActorRigCandidate {
        index: usize,
        value: &'a RigCandidate,
    },
    RigClipImage {
        value: &'a ImageInventory,
    },
    RigClipRig {
        index: usize,
        value: &'a Rig,
    },
    RigClipRule {
        rig: usize,
        index: usize,
        value: &'a Rule,
    },
    RigClipBank {
        rig: usize,
        index: usize,
        value: &'a Bank,
    },
    RigClipCandidate {
        rig: usize,
        value: &'a Clip,
    },
    WeaponMotionSource {
        index: usize,
        value: &'a Weapon,
    },
    WeaponMotionLoadout {
        value: &'a LoadoutLink,
    },
    WeaponMotionTag {
        value: &'a WeaponMotionTag,
    },
    MotionOptions {
        component: usize,
        value: MotionOptionFields<'a>,
    },
    MotionOption {
        component: usize,
        index: usize,
        value: &'a OptionRow,
    },
    DatabaseGraph {
        component: usize,
        root: &'a SourcedValue<usize>,
    },
    DatabaseBranch {
        component: usize,
        branch: usize,
        value: BranchFields<'a>,
    },
    DatabaseOption {
        component: usize,
        branch: usize,
        index: usize,
        value: &'a BranchOption,
    },
    DatabaseLink {
        component: usize,
        branch: usize,
        index: usize,
        value: &'a DatabaseLink,
    },
    AdbDocument {
        document: usize,
        source: &'a PhysicalSourceRef,
    },
    AdbFragment {
        document: usize,
        index: usize,
        value: &'a Fragment,
    },
    AdbTagSelectors {
        #[serde(flatten)]
        value: &'a AdbTagSelectors,
    },
    AdbBranchSelectors {
        #[serde(flatten)]
        value: &'a AdbBranchSelectors,
    },
    AdbTagScope {
        index: usize,
        value: &'a TagScope,
    },
    TagDefinitionRoot {
        index: usize,
        value: &'a Edge,
    },
    TagDefinitionDocument {
        value: &'a Document,
    },
    TagCycleBackEdge {
        value: &'a CycleEdge,
    },
    Source {
        value: &'a PhysicalSource,
    },
    SourceUse {
        value: &'a SourceUseDescription,
    },
    Document {
        index: usize,
        source: &'a PhysicalSourceRef,
        #[serde(flatten)]
        origin: &'a FieldOrigin,
        format: &'static str,
        lossy: bool,
        artifact: &'a Artifact,
    },
    DocumentRejection {
        source: &'a PhysicalSourceRef,
        #[serde(flatten)]
        origin: &'a FieldOrigin,
        reason: PlayerDocumentRejection,
        artifact: &'a Artifact,
    },
    End {
        rows: DecimalU64,
        raw_artifacts: &'a Totals,
    },
}

/// Returns the JSON Schema of a Player source bundle manifest.
#[must_use]
pub fn player_bundle_manifest_schema() -> serde_json::Value {
    schemars::schema_for!(Manifest<'static>).to_value()
}

/// Returns the JSON Schema of one Player source bundle JSONL record.
#[must_use]
pub fn player_bundle_record_schema() -> serde_json::Value {
    schemars::schema_for!(Row<'static>).to_value()
}
