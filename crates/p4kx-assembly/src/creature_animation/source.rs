// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;

use p4kx_extraction::{AnimationChoice, AnimationSet, ArchiveAnimationRead, ConversionError};

use super::clip_files;
use super::database::{Clips, DatabaseProblem, NameSelector, clips};
use super::owner::{
    self, CdfModel, CreatureAnimationSourceReport, OwnerInputs, ParameterRoute, SelectionInputs,
};
use crate::cdf::{self, MAX_CDF_BYTES, is_character_definition};
use crate::creature_rig::{is_skinned, skeleton_of};
use crate::gear_clip::{
    ChrParams, MAX_CHRPARAMS_BYTES, MAX_PARAM_SOURCES, params_path, read_params,
};
use crate::{ArchiveModels, Assembly, EntityNode, Issue, IssueKind, ModelFailure};

/// The clips of a creature Entity, read from the Archive, and what kept some from being read.
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct CreatureClips {
    /// The clips the Entity's animation database names that were found, as read by
    /// [`ArchiveModels::animations`].
    pub set: AnimationSet,
    /// Original Entity route and selected physical clip contributors.
    pub sources: Option<CreatureAnimationSourceReport>,
    /// What could not be found or read, as [`IssueKind::AnimationsUnavailable`] of the root Entity,
    /// whose [`Issue::path`] is empty.
    pub issues: Vec<Issue>,
}

/// Reads the clips the animation database of the creature `node` names, from the folders and the
/// track databases the parameter file of its skeleton lists.
///
/// A clip is found by its file name without extension. A name no folder has a clip for, such as a
/// blend space, is skipped. Nothing is read, and no issue is listed, when `assembly` writes no
/// skinned creature (see [`Assembly::write_glb_with`]) or `node` names no animation database.
///
/// # Errors
///
/// Returns the [`ConversionError`] of an animation file that exists and cannot be read.
pub fn creature_animations(
    models: &ArchiveModels<'_>,
    assembly: &Assembly,
    node: &EntityNode,
) -> Result<CreatureClips, ConversionError> {
    let (Some(database), Some(geometry)) = (
        node.animation_database.as_ref(),
        assembly.root().and_then(|root| root.geometry()),
    ) else {
        return Ok(CreatureClips::default());
    };
    let skinned = assembly.parts.iter().any(|part| {
        part.model
            .as_deref()
            .is_some_and(|model| is_skinned(model, assembly.lod))
    });
    if !skinned || skeleton_of(&assembly.parts, &assembly.issues, assembly.lod).is_none() {
        return Ok(CreatureClips::default());
    }
    let mut reasons = Vec::new();
    let database_path = database.authored().trim();
    let gathered = gather(models, (geometry, database_path), &mut reasons)?;
    let sources = gathered
        .parameters
        .as_ref()
        .map(|parameters| {
            owner::report(
                &OwnerInputs {
                    forge: node
                        .forge_origin
                        .as_deref()
                        .filter(|origin| origin.matches_archive(models.archive)),
                    database,
                    geometry,
                    geometry_field: geometry_field(assembly, node),
                    parameters,
                },
                &SelectionInputs {
                    names: &gathered.selectors,
                    clips: &gathered.clips,
                    complete: gathered.selection_complete,
                },
            )
        })
        .transpose()?;
    let set = gathered.clips.into_set();
    let issues = reasons
        .into_iter()
        .map(|reason| Issue {
            path: String::new(),
            name: node.class_name().to_owned(),
            kind: IssueKind::AnimationsUnavailable { reason },
        })
        .collect();
    Ok(CreatureClips {
        set,
        sources,
        issues,
    })
}

fn geometry_field(
    assembly: &Assembly,
    node: &EntityNode,
) -> Option<p4kx_extraction::SourceLocation> {
    let root = assembly.root()?;
    let record_index = root.entity_record_index()?;
    if node.record_index() != Some(record_index)
        || root.guid() != node.guid()
        || assembly.forge_origin() != node.forge_origin.as_deref()
    {
        return None;
    }
    root.geometry_selectors.model_field()
}

/// Reads the clips of `database`, a path below `Data`, for the root geometry `geometry`, and adds
/// to `reasons` what kept some from being read; an empty set when nothing can be read at all.
#[derive(Default)]
struct Gathered {
    clips: ArchiveAnimationRead,
    selectors: Vec<NameSelector>,
    parameters: Option<ParameterRoute>,
    selection_complete: bool,
}

fn gather(
    models: &ArchiveModels<'_>,
    (geometry, database): (&str, &str),
    reasons: &mut Vec<String>,
) -> Result<Gathered, ConversionError> {
    let (params, parameters) = match parameters(models, geometry) {
        Ok(params) => params,
        Err(reason) => {
            reasons.push(reason);
            return Ok(Gathered::default());
        }
    };
    let mut gathered = Gathered {
        parameters: Some(parameters),
        ..Gathered::default()
    };
    if params.folders.is_empty() {
        reasons.push("the parameter file of the skeleton names no animation folder".to_owned());
        return Ok(gathered);
    }
    if params.capped {
        reasons.push(format!(
            "the parameter file of the skeleton lists more than {MAX_PARAM_SOURCES} animation folders or track databases, the others are left out"
        ));
    }
    let Some((
        Clips {
            mut names,
            selectors,
            ..
        },
        database_complete,
    )) = database_names(models, database, reasons)
    else {
        return Ok(gathered);
    };
    let extra: Vec<String> = params
        .databases
        .iter()
        .filter_map(|path| models.entry_of(path))
        .collect();
    let folders: Vec<&str> = params.folders.iter().map(String::as_str).collect();
    let files = clip_files::below(models, &folders);
    if !files.has_database && extra.is_empty() {
        names.retain(|name| files.stems.contains(&name.to_ascii_lowercase()));
    }
    gathered.clips = read_named(models, &folders, names, &extra)?;
    gathered.selectors = selectors;
    gathered.selection_complete = database_complete
        && !params.capped
        && extra.len() == params.databases.len()
        && gathered.clips.set().unread.is_empty();
    if gathered.clips.set().animations.is_empty() {
        reasons.push(format!(
            "no clip of the animation database {database} is in the animation folders of the skeleton"
        ));
    }
    Ok(gathered)
}

/// Returns the clip names of `database`, adding to `reasons` what it could not read or had to drop;
/// `None` when the database itself gives none.
fn database_names(
    models: &ArchiveModels<'_>,
    database: &str,
    reasons: &mut Vec<String>,
) -> Option<(Clips, bool)> {
    match clips(models, database) {
        Ok(mut read) => {
            let complete =
                read.unread.is_empty() && read.truncated.is_empty() && !read.unpositioned_selectors;
            reasons.extend(std::mem::take(&mut read.unread).into_iter().map(|(file, reason)| {
                format!(
                    "the animation database {file}, nested in {database}, cannot be read: {reason}"
                )
            }));
            reasons.append(&mut read.truncated);
            Some((read, complete))
        }
        Err(DatabaseProblem::Missing) => {
            reasons.push(format!(
                "the animation database {database} is not in the Archive"
            ));
            None
        }
        Err(DatabaseProblem::Unreadable(reason)) => {
            reasons.push(format!(
                "the animation database {database} cannot be read: {reason}"
            ));
            None
        }
    }
}

/// Reads the parameter file beside the skeleton of the root geometry `geometry`: the file itself
/// for a skeleton, the one a character definition names otherwise.
fn parameters(
    models: &ArchiveModels<'_>,
    geometry: &str,
) -> Result<(ChrParams, ParameterRoute), String> {
    let (skeleton, cdf) = if is_character_definition(geometry) {
        let (origin, bytes) = models.observed_file(geometry, MAX_CDF_BYTES);
        let bytes =
            bytes.map_err(|failure| format!("the character definition {geometry}: {failure}"))?;
        let (skeleton, position) = cdf::read_skeleton_with_position(&bytes)
            .map_err(|reason| format!("the character definition {geometry}: {reason}"))?;
        let skeleton = skeleton
            .ok_or_else(|| format!("the character definition {geometry} names no skeleton"))?;
        (
            skeleton,
            Some(CdfModel {
                input: origin,
                field: position,
            }),
        )
    } else {
        (geometry.to_owned(), None)
    };
    let path = params_path(&skeleton);
    let (params, bytes) = models.observed_file(&path, MAX_CHRPARAMS_BYTES);
    let (parsed, length) = match bytes {
        Ok(bytes) => read_params(&bytes)
            .map(|parsed| (parsed, bytes.len() as u64))
            .map_err(|reason| format!("the parameter file {path} cannot be read: {reason}")),
        Err(ModelFailure::Missing) => {
            Err(format!("the parameter file {path} is not in the Archive"))
        }
        Err(other) => Err(format!("the parameter file {path} cannot be read: {other}")),
    }?;
    Ok((
        parsed,
        ParameterRoute {
            skeleton,
            cdf,
            params: params.map(|input| (input, length)),
        },
    ))
}
/// Reads the clips `names` below `folders` and the Entries `extra`. A name no clip has is left
/// out; names that only a database can lack cost a second read.
fn read_named(
    models: &ArchiveModels<'_>,
    folders: &[&str],
    names: Vec<String>,
    extra: &[String],
) -> Result<ArchiveAnimationRead, ConversionError> {
    if names.is_empty() {
        return Ok(ArchiveAnimationRead::default());
    }
    let choice = AnimationChoice::Named(names);
    match models.animations_with_clip_evidence(folders, &choice, extra) {
        Err(ConversionError::UnknownAnimations { names: missing }) => {
            let AnimationChoice::Named(mut present) = choice else {
                return Ok(ArchiveAnimationRead::default());
            };
            let missing: HashSet<&str> = missing.iter().map(String::as_str).collect();
            present.retain(|name| !missing.contains(name.as_str()));
            if present.is_empty() {
                return Ok(ArchiveAnimationRead::default());
            }
            models.animations_with_clip_evidence(folders, &AnimationChoice::Named(present), extra)
        }
        other => other,
    }
}
