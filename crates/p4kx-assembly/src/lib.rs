// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Assembly service of P4KX: resolves the Loadout tree of an Entity and reads the CHF
//! files of Characters.
//!
//! [`Entities`] finds an Entity by record path, GUID or class name. [`LoadoutResolver`] resolves
//! its default Loadout (`SEntityComponentDefaultLoadoutParams`) into an [`EntityNode`] tree: every
//! port with the Entity attached to it, the port's definition (`SItemPortDef`: sizes, types and
//! transform), and recursively the ports of each attached Entity. A Loadout entry's own Loadout
//! overrides the attached Entity's default Loadout per port; ports a Loadout does not name fall
//! back to the port definition's default item. XML Loadouts are read through [`LoadoutFiles`],
//! e.g. [`ArchiveFiles`].
//!
//! A port that cannot be resolved, such as a dangling Reference, an unknown class name or a
//! cycle, is kept in the tree as [`Attachment::Unresolved`] with a [`LoadoutError`]; the rest of
//! the Loadout still resolves.
//!
//! [`EntityCatalogue`] lists every Entity once, with its [`EntityKind`] and [`ItemFacts`], for
//! pickers; [`EntityCatalogue::compatible`] narrows it to what a port takes.
//!
//! [`LoadoutResolver::resolve_with`] applies [`LoadoutChanges`] while it resolves: ports left out
//! and Entities attached instead, each checked against the port's [`PortConstraints`] with the
//! [`ItemFacts`] of the Entity, reported as [`ChangeError`]s.
//!
//! [`Assembly::build`] turns the tree into a scene graph of placed models: the geometry each
//! Entity's Record names ([`Geometry`]), read through a [`ModelProvider`] such as
//! [`ArchiveModels`], placed at its port's helper in the parent's model, with the material
//! library the Entity chooses; [`Assembly::write_glb`] exports it as one GLB with a node per part
//! named `<port>/<entity>`. Parts that cannot be resolved are listed as [`Issue`]s.
//! [`Assembly::build_with_palettes`] also resolves the tint palette of each part
//! ([`ForgePalettes`] reads the `TintPaletteTree` Records), which the GLB draws the materials with
//! that take one; [`Assembly::write_glb_with`] embeds the textures of the materials.
//!
//! An Entity whose Record names a vehicle implementation (`VehicleComponentParams`) gets it with
//! the Modification it selects applied, as [`EntityNode::vehicle`]: the parts with their item
//! ports, treads and wheels, and per port the [`PortVisibility`] that [`PortNode::visibility`]
//! shows.
//!
//! [`PrefabSet`] reads the prefab libraries below `Data\Prefabs` and [`PrefabSet::resolve`] turns
//! a prefab into its [`ResolvedObject`]s with transforms to the prefab's space, every nested prefab
//! expanded in place with the parent's rotation applied. Cycles, depth and size are bounded; what
//! cannot be expanded is listed as [`PrefabIssue`]s.
//!
//! [`audio`] indexes the audio of an Archive once: the ATL definitions that map audio trigger
//! names to Wwise events and the soundbanks whose `HIRC` sections hold those events, matched by
//! the events' Wwise short IDs ([`audio::wwise_id`]). [`audio::AudioIndex::entity_audio`] lists
//! the audio triggers an Entity names with their events and soundbanks.
//!
//! [`character`] reads and writes the CHF files of Characters and their JSON form.
//!
//! [`Armour::build`] puts [`Piece`]s of armour and clothing on the default body of a
//! [`BodyVariant`]: the variant's skeleton and default body skin, constant paths, and per piece
//! the geometry its Record names for the variant's tag ([`Wearable`]), skinned to the body's
//! skeleton by bone name; [`Armour::write_glb`] exports it as one skinned GLB. A piece that cannot
//! be fitted is an [`Issue`]. [`LoadoutResolver::worn`] takes the pieces from a resolved Loadout
//! tree, and [`Piece::layer`] puts pieces given by hand on top of them.
//!
//! [`Head::from_character`] makes the head of a CHF [`character::Character`]: the head skin of its
//! rig, shaped by the Character's face DNA ([`Piece::shaped`]) and worn on the body of the rig's
//! variant. [`Head::dressed`] adds the teeth and the items of the Character's head port (eyes,
//! hair, brows, lashes, beard), read through [`HeadItems`] such as [`ForgeItems`], with the
//! material choices of the Character ([`Look`]); [`Armour::write_glb_with`] embeds the textures.
//! [`Head::with_expressions`] adds the head's expression blendshapes as morph targets.

mod animation_folders;
mod armour;
mod assembly;
mod assortment;
mod assortment_group;
mod assortment_requirement;
mod attached;
pub mod audio;
mod build;
mod build_attachments;
mod build_gear;
mod build_hidden;
mod build_models;
mod build_palettes;
mod build_place;
mod build_zones;
mod catalogue;
mod cdf;
mod cdf_bone;
mod changes;
mod character_path;
mod chf_names;
mod compat;
mod creature_rig;
mod entities;
mod entity_interior;
mod error;
mod files;
mod flatten;
mod gear_clip;
mod gear_pose;
mod geometry;
mod hide_rule;
mod hologram;
mod interior;
#[cfg(feature = "verification")]
mod interior_check;
mod issue;
mod issue_code;
mod issue_record;
mod issue_text;
mod loadout_entry;
mod markup;
mod merge;
mod model_cache;
mod node;
mod node_body;
mod node_geometry;
mod node_nested;
mod node_notes;
mod object_containers;
mod outfit;
mod outfit_read;
mod palette;
mod path_match;
mod pick;
mod placement;
mod plan;
mod port;
mod prefab;
mod provider;
mod read;
mod resolver;
mod resolver_prefetch;
mod resolver_vehicle;
mod skeleton;
mod spec;
mod tree;
mod vehicle;
mod walk;
mod xml;

pub use animation_folders::{AnimationFolders, animation_choice, character_animations};
pub use armour::{
    Armour, ArmourPart, BodyVariant, CharacterAnimations, Expressions, ForgeItems, Head, HeadError,
    HeadItems, Look, MAX_ANIMATION_VALUES, MAX_DNA_RIG_LEN, NpcHead, PartOrigin, Piece, Replaced,
    Wearable, ZoneChunk, is_worn_type,
};
pub use assembly::{
    Assembly, AssemblyOptions, MAX_ASSEMBLY_DEPTH, MAX_ASSEMBLY_PARTS, MAX_ASSEMBLY_VERTICES, Part,
    Placement,
};
pub use attached::AttachedModel;
pub use catalogue::{EntityCatalogue, EntityKind, EntitySummary};
pub use changes::{ChangeError, ChangedLoadout, LoadoutChanges, PortChange, ReplacementMismatch};
pub use chf_names::archive_names;
pub use compat::{AcceptedType, Incompatibility, ItemFacts, PortConstraints};
pub use entities::{ENTITY_TYPE, Entities};
pub use entity_interior::{EntityContainer, EntityInterior, INTERIOR_NODE};
pub use error::{AssemblyError, EntityLookupError, LoadoutError};
pub use files::{ArchiveFiles, LoadoutFileError, LoadoutFiles, MAX_XML_LOADOUT_LEN};
pub use geometry::{Geometry, GeometryVariant, MaterialVariant, PaletteRef};
pub use hologram::HologramCamera;
pub use interior::{
    Interior, InteriorForge, InteriorOptions, InteriorPart, InteriorPartKind, InteriorProgress,
    InteriorWatch, MAX_INTERIOR_CONTAINERS, MAX_INTERIOR_DEPTH, MAX_INTERIOR_FILE_LEN,
    MAX_INTERIOR_PARTS,
};
#[cfg(feature = "verification")]
pub use interior_check::{
    AnimatedPart, ContainerFit, HULL_SLACK, Hardpoint, HardpointFit, HardpointKind, InteriorFit,
    Misfit, ROOM_VOLUME_SHARE, WorldBox,
};
pub use issue::IssueKind;
pub use issue_record::Issue;
pub use loadout_entry::LoadoutEntry;
pub use model_cache::{CachedModels, DEFAULT_MODEL_CACHE_BYTES, ModelCache};
pub use node_body::CharacterBody;
pub use object_containers::{ContainerSource, ObjectContainerRef};
pub use outfit::{Outfit, OutfitReport};
pub use p4kx_character as character;
pub use palette::{ForgePalettes, PaletteFailure, PaletteProvider};
pub use pick::LoadoutPicks;
pub use port::{PortDefinition, PortTransform, PortType};
pub use prefab::{
    MAX_PREFAB_DEPTH, MAX_PREFAB_ISSUES, MAX_PREFAB_LIBRARY_LEN, MAX_PREFAB_OBJECTS,
    MAX_PREFAB_VISITS, PrefabError, PrefabFile, PrefabIssue, PrefabIssueKind, PrefabSet,
    ResolvedObject, ResolvedPrefab,
};
pub use provider::{ArchiveModels, ModelFailure, ModelProvider, ProvidedAttachment, ProvidedModel};
pub use resolver::LoadoutResolver;
pub use skeleton::{MAX_SKELETON_LEN, SkeletonProblem};
pub use spec::LoadoutOrigin;
pub use tree::{Attachment, EntityNode, PortNode, PortSource, Target, Unresolved};
pub use vehicle::{
    ElemOverride, LandingGear, ModificationReport, OriginTally, PartKind, PartThreshold,
    PartialValue, PortVisibility, UnreadableValue, ValueOrigin, VehicleImplementation,
    VehicleItemPort, VehicleItemType, VehiclePart, VehicleTotals,
};
pub use xml::{MAX_XML_INCLUDE_DEPTH, MAX_XML_INCLUDES};

/// The deepest element nesting of a vehicle implementation or patch file that is read; deeper
/// files are refused.
pub const MAX_XML_ELEMENT_DEPTH: usize = markup::MAX_ELEMENT_DEPTH;

/// The deepest chain of attached Entities below the root that is resolved; deeper attachments
/// are [`LoadoutError::TooDeep`].
pub const MAX_LOADOUT_DEPTH: usize = 32;

/// The most ports one Loadout tree holds; an Entity whose ports would pass it keeps the ports so
/// far and has [`LoadoutError::TooManyPorts`] among its issues.
pub const MAX_LOADOUT_PORTS: usize = 1 << 16;
