// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use thiserror::Error;

use crate::LoadoutError;
use crate::issue_text::{animations_left_out, attachments_left_out};

/// What went wrong with a part of an Assembly.
#[derive(Debug, Clone, PartialEq, Error)]
#[non_exhaustive]
pub enum IssueKind {
    /// The Loadout names an Entity that cannot be resolved, so the port holds no part.
    #[error("the attachment cannot be resolved: {0}")]
    Unresolved(LoadoutError),
    /// The Entity's Record names no geometry, so the part has no model.
    #[error("the Entity names no geometry")]
    NoGeometry,
    /// The geometry file the Record names does not exist, so the part has no model.
    #[error("the geometry {path} does not exist")]
    GeometryMissing {
        /// The path the Record names.
        path: String,
    },
    /// The sub-geometry the port's flags choose does not exist, so the part keeps the main
    /// geometry of its Entity.
    #[error("the sub-geometry {path} does not exist; the main geometry is used")]
    SubGeometryMissing {
        /// The path of the sub-geometry.
        path: String,
    },
    /// The sub-geometry tagged for the body of the character does not exist, so the part has no
    /// model; the main geometry, a placeholder, is not drawn instead.
    #[error("the {body} sub-geometry {path} does not exist")]
    BodyGeometryMissing {
        /// The body variant the sub-geometry is tagged for, e.g. `Male`.
        body: String,
        /// The path of the sub-geometry.
        path: String,
    },
    /// A worn item names only a prop of the world (a cardboard box) as its geometry and no model
    /// for the body, so the part has no model.
    #[error("the geometry {path} is a prop of the world, not a model of the worn item")]
    WorldPropNotWorn {
        /// The path of the prop the Record names.
        path: String,
    },
    /// The geometry file cannot be read as a model, so the part has no model.
    #[error("the geometry {path} cannot be read: {reason}")]
    GeometryUnreadable {
        /// The path the Record names.
        path: String,
        /// What is wrong.
        reason: String,
    },
    /// The material library of the model is missing or unreadable, so the engine's default
    /// material is used.
    #[error("{reason}")]
    MaterialLibrary {
        /// What is wrong.
        reason: String,
    },
    /// A tint palette the Entity or its material library names cannot be resolved, so the part
    /// falls back to the next palette or to the colours of its library.
    #[error("{reason}")]
    Palette {
        /// What is wrong.
        reason: String,
    },
    /// The helper of a port or of an object container is no node of the model of the Entity it
    /// belongs to, so the part or the container sits at that Entity's origin.
    #[error("the helper {helper} is no node of the Entity's model")]
    HelperNotFound {
        /// The helper the port or the container names.
        helper: String,
    },
    /// The geometry is a character definition (`.cdf`); its skin is drawn and the files of its
    /// other attachments are not.
    #[error("{}", attachments_left_out(.files))]
    AttachmentsLeftOut {
        /// The files of the attachments that are not drawn.
        files: Vec<String>,
    },
    /// The skeleton file of a character definition (`.cdf`) does not exist; the skin is drawn as
    /// it is.
    #[error("the skeleton {path} the character definition names does not exist")]
    SkeletonMissing {
        /// The path the definition names, with `/` as separator.
        path: String,
    },
    /// The skeleton file of a character definition (`.cdf`) is too large or cannot be read as a
    /// model; the skin is drawn as it is.
    #[error("the skeleton {path} the character definition names cannot be read: {reason}")]
    SkeletonUnreadable {
        /// The path the definition names, with `/` as separator.
        path: String,
        /// What is wrong.
        reason: String,
    },
    /// The landing gear could not be posed at the end of its deploy clip, so it keeps the pose of
    /// its model.
    #[error("the landing gear keeps the pose of its model: {reason}")]
    GearNotDeployed {
        /// Why the gear was not posed.
        reason: String,
    },
    /// A container nested in an Interior cannot be found, so it and what is in it are left out.
    #[error("the container {name} does not exist")]
    ContainerMissing {
        /// The name the description gives.
        name: String,
    },
    /// A nested container cannot be opened, so it and what is in it are left out.
    #[error("the container {name} cannot be opened: {reason}")]
    ContainerUnreadable {
        /// The name the description gives.
        name: String,
        /// What is wrong.
        reason: String,
    },
    /// A container is nested in itself, directly or through others, so it is not entered again.
    #[error("the container {name} is nested in itself and not entered again")]
    ContainerCycle {
        /// The name the description gives.
        name: String,
    },
    /// An Interior holds [`MAX_INTERIOR_CONTAINERS`](crate::MAX_INTERIOR_CONTAINERS) containers,
    /// so further nested containers are left out.
    #[error("the Interior reached {limit} containers; further containers are left out")]
    TooManyContainers {
        /// The limit.
        limit: usize,
    },
    /// A file a container names does not exist, so what it holds is left out.
    #[error("the file {file} does not exist")]
    FileMissing {
        /// The file's path in the container.
        file: String,
    },
    /// A file of a container cannot be read, so what it holds is left out.
    #[error("the file {file} cannot be read: {reason}")]
    FileUnreadable {
        /// The file's path in the container.
        file: String,
        /// What is wrong.
        reason: String,
    },
    /// A nested container has a placement that is not usable, so it is left out.
    #[error("the placement of the container {name} is not usable: {reason}")]
    InvalidPlacement {
        /// The name the description gives.
        name: String,
        /// What is wrong.
        reason: String,
    },
    /// The offset of an Entity's object container is not finite, so the container sits at its
    /// bone alone or at the origin.
    #[error("the offset of the container {name} is not finite and is left out")]
    InvalidOffset {
        /// The container's file as the Record spells it.
        name: String,
    },
    /// The offset of an Entity's object container differs from its bone in the Entity's model by
    /// more than the tolerance, so the bone alone places the container.
    #[error(
        "the offset of the container differs from the bone {bone} of the Entity; the bone is used"
    )]
    OffsetDiffers {
        /// The bone of the Entity's model.
        bone: String,
    },
    /// Placed models of a `.soc` file name no listed model or have no usable transform, so they
    /// are left out.
    #[error(
        "{count} placed models name no listed model or have no usable transform and are left out"
    )]
    ObjectsLeftOut {
        /// How many.
        count: usize,
    },
    /// A `.soc` file lists objects of a kind that is not read, so the list ends there.
    #[error("the objects from the first of kind {kind} on are not read")]
    UnsupportedObjects {
        /// The kind of the first object that is not a placed model.
        kind: u32,
    },
    /// Props or lights of a `.soc` file have a position, rotation, scale or parameters that are
    /// not usable, so they are left out.
    #[error("{count} props or lights have no usable placement and are left out")]
    EntitiesLeftOut {
        /// How many.
        count: usize,
    },
    /// Lights of a type `KHR_lights_punctual` cannot express (ambient lights, which stand in for
    /// global illumination, planar area lights and unknown types), so they are left out.
    #[error(
        "{count} lights of the type {light_type} cannot be written as glTF lights and are left out"
    )]
    LightUnsupported {
        /// The `lightType` as the `.soc` file spells it.
        light_type: String,
        /// How many.
        count: usize,
    },
    /// The Assembly holds [`MAX_ASSEMBLY_PARTS`](crate::MAX_ASSEMBLY_PARTS) parts, so further
    /// parts are left out.
    #[error("the Assembly or Interior reached {limit} parts; further parts are left out")]
    TooManyParts {
        /// The limit.
        limit: usize,
    },
    /// The parts' geometry reached [`MAX_ASSEMBLY_VERTICES`](crate::MAX_ASSEMBLY_VERTICES)
    /// vertices, so this part has no model.
    #[error("the Assembly reached {limit} vertices; the part's geometry is left out")]
    TooManyVertices {
        /// The limit.
        limit: usize,
    },
    /// The part is nested deeper than [`MAX_ASSEMBLY_DEPTH`](crate::MAX_ASSEMBLY_DEPTH), so it
    /// and what is below it are left out.
    #[error("the part is nested deeper than {limit} levels and left out")]
    TooDeep {
        /// The limit.
        limit: usize,
    },
    /// Worn clothing covers these zones of the body, so the body's triangles in them are left out.
    #[error("the body zones {} are covered by the clothing and left out", .0.join(", "))]
    BodyZonesHidden(Vec<String>),
    /// The body has no part to cut or its submeshes do not fit its zone table, so no body
    /// triangles are left out.
    #[error("the body cannot be cut by zone: {0}")]
    BodyZonesUnknown(String),
    /// Clothing covers these body zones only under conditions that are not evaluated.
    #[error("the body zones {} are covered only under unevaluated conditions", .0.join(", "))]
    ClothingConditionsUndecided(Vec<String>),
    /// Another worn piece hides the port the piece sits at, so the piece is left out.
    #[error("the piece at the port {port} is hidden by {by} and left out")]
    PartHidden {
        /// The port the piece sits at.
        port: String,
        /// The name of the piece whose Record hides the port.
        by: String,
    },
    /// This many layered materials of the part have no diffuse map, neither their own nor one in
    /// a layer, and are drawn in the colours of their layers.
    #[error(
        "layered materials whose layers have no diffuse map ({0}) are drawn in their layer colours"
    )]
    LayersWithoutDiffuse(usize),
    /// The piece of armour or clothing cannot be found or read as an Entity, so it is left out.
    #[error("the piece cannot be resolved: {reason}")]
    PieceNotFound {
        /// Why.
        reason: String,
    },
    /// The piece's Record names no geometry to wear, so it is left out.
    #[error("the Entity names no geometry to wear")]
    NoWornGeometry,
    /// The piece's Record names geometry per body, but none for this one, so it is left out.
    #[error("the Entity has no geometry for the {body} body")]
    NoVariantGeometry {
        /// The code of the body variant.
        body: String,
    },
    /// The piece's Record names no geometry per body, so its main geometry, which is not a skin, is
    /// worn on this body as it is.
    #[error(
        "the Entity has no geometry for the {body} body; its main geometry, which is not a skin, is worn"
    )]
    MainGeometryWorn {
        /// The code of the body variant.
        body: String,
    },
    /// Some bones of the model are not in the skeleton it is bound to; their vertices follow the
    /// nearest ancestor bone the skeleton has.
    #[error("{count} bones are not in the skeleton and follow their nearest parent")]
    BonesNotInSkeleton {
        /// The number of bones.
        count: usize,
    },
    /// The piece's model has no bone weights, so it is placed as it is, without the skeleton.
    #[error("the model has no bone weights and is not skinned to the body")]
    NotSkinned,
    /// The piece is shaped by a DNA, but the DNA rig cannot be read or does not fit the model, so
    /// the piece is left out.
    #[error("the piece cannot be shaped by its DNA: {reason}")]
    ShapeFailed {
        /// Why.
        reason: String,
    },
    /// The Character's base material for the piece is in no lookup table, so the piece keeps the
    /// material its Record or model names.
    #[error("the material {material} the character chose is unknown; the piece keeps its own")]
    MaterialChoiceUnknown {
        /// The GUID of the material, as text.
        material: String,
    },
    /// Some of the Character's material choices for the piece are not drawn.
    #[error("{what} are not drawn")]
    LookIgnored {
        /// What is left out.
        what: String,
    },
    /// The body type of the Character is not the body type of the head rig its DNA names, so the
    /// file is inconsistent; the head is shaped by the DNA as it is.
    #[error("the body type {body_type} does not belong to the head rig {rig} of the DNA")]
    BodyTypeMismatch {
        /// The GUID of the Character's body type, as text.
        body_type: String,
        /// The name of the head rig the DNA names.
        rig: String,
    },
    /// A piece that follows the head has several meshes at the level of detail built; only the
    /// first follows the head's expressions.
    #[error("{meshes} meshes: only the first follows the expressions of the head")]
    FollowMorphsFirstMeshOnly {
        /// The meshes of the piece at the level of detail built.
        meshes: usize,
    },
    /// The morph targets of a piece that follows the head take no animation weights.
    #[error("its expression targets are not animated: {reason}")]
    FollowWeightsSkipped {
        /// Why.
        reason: String,
    },
    /// A piece that follows the head is not bound to the head's joints, so a facial animation does
    /// not move it; it keeps its own bones.
    #[error("it is not bound to the facial joints of the head: {reason}")]
    FollowJointsSkipped {
        /// Why.
        reason: String,
    },
    /// The Loadout of the Entity could not be read in full; what could be read is used.
    #[error("the Loadout of the Entity cannot be read in full: {reason}")]
    LoadoutUnreadable {
        /// Why.
        reason: String,
    },
    /// The expression morph targets of a shaped piece cannot be made, so the piece is kept
    /// without them.
    #[error("the expression morph targets cannot be made: {reason}")]
    MorphsFailed {
        /// Why.
        reason: String,
    },
    /// An expression the piece was asked for is no blendshape of its head rig, so it is not
    /// written.
    #[error("the head rig has no expression {name}")]
    UnknownExpression {
        /// The name that was asked for.
        name: String,
    },
    /// An expression the piece was asked for moves no vertex of this Character's head, so it is
    /// not written.
    #[error("the expression {name} moves no vertex of the head")]
    EmptyExpression {
        /// The name that was asked for.
        name: String,
    },
    /// The joints of a shaped head's face cannot be moved to the Character's own positions, so
    /// they stay where the skin binds them.
    #[error("the facial joints cannot be placed by the DNA: {reason}")]
    FaceJointsFailed {
        /// Why.
        reason: String,
    },
    /// The rig of the face cannot be read, so its animations are not written.
    #[error("the face animation cannot be evaluated: {reason}")]
    FaceAnimationFailed {
        /// Why.
        reason: String,
    },
    /// Animations are not written, all for the same reason: no track fits the Character, or the
    /// budget of values is used up.
    #[error("{}", animations_left_out(*.count, .reason, .examples))]
    AnimationNotUsed {
        /// Why.
        reason: String,
        /// How many animations are left out for this reason.
        count: usize,
        /// The names of the first few of them.
        examples: Vec<String>,
    },
}
