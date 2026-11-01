// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_character::Dna;

use super::hidden;
use super::piece::{Mount, Note, Source};
use super::wearable::{Unfit, Worn};
use super::{BodyVariant, Expressions, Look, PartOrigin, Piece, ZoneChunk};
use crate::bespoke::{Bespoke, exists, missing_library};
use crate::{Issue, IssueKind, MaterialVariant, ModelProvider, PaletteRef};

/// One skin to read and wear.
pub(super) struct Job {
    pub(super) origin: PartOrigin,
    pub(super) part: String,
    pub(super) owner: String,
    pub(super) label: String,
    pub(super) path: String,
    pub(super) material: Option<String>,
    pub(super) palette: Option<PaletteRef>,
    pub(super) shape: Option<Dna>,
    pub(super) expressions: Expressions,
    pub(super) look: Option<Look>,
    pub(super) follows_head: bool,
    pub(super) bespoke: bool,
    pub(super) zones: Arc<[ZoneChunk]>,
    pub(super) mount: Option<Mount>,
}

impl Job {
    /// Returns the job of the default body skin at `path`.
    pub(super) fn body(path: &str) -> Self {
        Self {
            origin: PartOrigin::Body,
            part: "body".to_owned(),
            owner: String::new(),
            label: "body".to_owned(),
            path: path.to_owned(),
            material: None,
            palette: None,
            shape: None,
            expressions: Expressions::None,
            look: None,
            follows_head: false,
            bespoke: false,
            zones: Arc::default(),
            mount: None,
        }
    }
}

pub(super) fn issue(job: &Job, kind: IssueKind) -> Issue {
    Issue {
        path: job.owner.clone(),
        name: job.label.clone(),
        kind,
    }
}

pub(super) fn piece_issue(piece: &Piece, kind: IssueKind) -> Issue {
    Issue {
        path: String::new(),
        name: piece.name.clone(),
        kind,
    }
}

fn note_kind(note: &Note) -> IssueKind {
    match note {
        Note::UnknownMaterial(material) => IssueKind::MaterialChoiceUnknown {
            material: material.clone(),
        },
        Note::LookIgnored(what) => IssueKind::LookIgnored { what: what.clone() },
        Note::BodyTypeMismatch { body_type, rig } => IssueKind::BodyTypeMismatch {
            body_type: body_type.clone(),
            rig: (*rig).to_owned(),
        },
        Note::LoadoutUnreadable(reason) => IssueKind::LoadoutUnreadable {
            reason: reason.clone(),
        },
    }
}

/// Returns the skin of the story character's own head that stands for the generic skin at
/// `path`, if the Archive has one: the file `<prefix>_<part>.skin` in the folder of `bespoke`.
fn bespoke_path(path: &str, bespoke: &Bespoke, models: &dyn ModelProvider) -> Option<String> {
    bespoke
        .skin_for(path)
        .filter(|candidate| exists(candidate, models))
}

/// Returns the skin of the story character's own head that stands for the generic skin at `path`
/// of `piece`, if the piece has such a head and the Archive has the skin.
fn own_skin(piece: &Piece, path: &str, models: &dyn ModelProvider) -> Option<String> {
    let bespoke = piece.bespoke.as_ref()?;
    bespoke_path(path, bespoke, models)
}

fn is_skinned_file(path: &str) -> bool {
    path.rsplit_once('.').is_some_and(|(_, extension)| {
        extension.eq_ignore_ascii_case("skin") || extension.eq_ignore_ascii_case("chr")
    })
}

/// Adds the skin `piece` contributes for `variant` to `jobs`, or the reason it contributes none
/// to `issues`; the notes of the piece go to `issues` in either case.
pub(super) fn gather(
    piece: &Piece,
    variant: BodyVariant,
    models: &dyn ModelProvider,
    jobs: &mut Vec<Job>,
    issues: &mut Vec<Issue>,
) {
    issues.extend(
        piece
            .notes
            .iter()
            .map(|note| piece_issue(piece, note_kind(note))),
    );
    let (path, material, palette) = match &piece.source {
        Source::Body => return,
        Source::Failed(reason) => {
            let kind = IssueKind::PieceNotFound {
                reason: reason.clone(),
            };
            issues.push(piece_issue(piece, kind));
            return;
        }
        Source::File { path, material } => (path.clone(), material.clone(), None),
        Source::Wearable(wearable) => match wearable.geometry_selected(variant, &piece.selecting) {
            Ok(Worn { geometry, main }) => {
                if main && !is_skinned_file(geometry.path()) {
                    let body = variant.code().to_owned();
                    issues.push(piece_issue(piece, IssueKind::MainGeometryWorn { body }));
                }
                (
                    geometry.path().to_owned(),
                    chosen_material(piece, wearable, variant, geometry.material()),
                    geometry.palette().cloned(),
                )
            }
            Err(Unfit::NoGeometry) => {
                issues.push(piece_issue(piece, IssueKind::NoWornGeometry));
                return;
            }
            Err(Unfit::NoVariant) => {
                let body = variant.code().to_owned();
                issues.push(piece_issue(piece, IssueKind::NoVariantGeometry { body }));
                return;
            }
        },
    };
    let own = own_skin(piece, &path, models);
    let bespoke = own.is_some();
    let material = existing_library(piece, material, bespoke, models, issues);
    let zones = covered_zones(piece, issues);
    jobs.push(Job {
        zones,
        origin: PartOrigin::Piece,
        part: piece.name.clone(),
        owner: String::new(),
        label: piece.name.clone(),
        path: own.unwrap_or(path),
        material: piece.material.clone().or(material),
        palette,
        shape: piece.shape.clone(),
        expressions: piece.expressions.clone(),
        look: piece.look.clone(),
        follows_head: piece.follows_head,
        bespoke,
        mount: piece.mount.clone(),
    });
}

/// Returns `material` for a piece of a story character's head, or, when the Archive lacks that
/// library, the head's own library for a part with a skin of its own, and none otherwise; the
/// missing library is reported to `issues`. Other pieces keep `material`.
fn existing_library(
    piece: &Piece,
    material: Option<String>,
    bespoke: bool,
    models: &dyn ModelProvider,
    issues: &mut Vec<Issue>,
) -> Option<String> {
    let (Some(head), Some(library)) = (&piece.bespoke, &material) else {
        return material;
    };
    let head_library = bespoke.then_some(head.library.as_str());
    let Some(missing) = missing_library(library, head_library, |name| !exists(name, models)) else {
        return material;
    };
    let reason = missing.reason;
    issues.push(piece_issue(piece, IssueKind::MaterialLibrary { reason }));
    missing.replacement
}

/// Returns the material library of a worn Entity: the one its material tree has for the tags of
/// the piece and the body, else the one of its geometry.
fn chosen_material(
    piece: &Piece,
    wearable: &super::Wearable,
    variant: BodyVariant,
    geometry: Option<&str>,
) -> Option<String> {
    if piece.variant_tags.is_empty() {
        return geometry.map(str::to_owned);
    }
    let mut tags = piece.variant_tags.clone();
    tags.push(variant.tag().to_owned());
    MaterialVariant::resolve(wearable.material_variants(), &tags)
        .or(geometry)
        .map(str::to_owned)
}

/// Gives the body job, the first of `jobs`, the material library and the look of the
/// [`Piece::body`] `piece`. Only the first such piece of an Armour restyles the body.
fn restyle_body(jobs: &mut [Job], piece: &Piece) {
    if let Some(body) = jobs.first_mut() {
        body.material.clone_from(&piece.material);
        body.look.clone_from(&piece.look);
    }
}

/// Returns the skins to wear: the body, always first, and what each piece contributes, at most
/// `max_parts` of them; the jobs beyond are listed in `issues`.
pub(super) fn planned(
    variant: BodyVariant,
    pieces: &[Piece],
    models: &dyn ModelProvider,
    max_parts: usize,
    issues: &mut Vec<Issue>,
) -> Vec<Job> {
    let mut jobs = vec![Job::body(variant.skin())];
    let shown = hidden::worn(pieces, issues);
    let mut restyled = false;
    for piece in pieces
        .iter()
        .zip(shown)
        .filter_map(|(piece, shown)| shown.then_some(piece))
    {
        if matches!(piece.source, Source::Body) {
            if restyled {
                continue;
            }
            restyled = true;
            restyle_body(&mut jobs, piece);
        }
        gather(piece, variant, models, &mut jobs, issues);
    }
    if jobs.len() > max_parts {
        for job in jobs.split_off(max_parts) {
            let kind = IssueKind::TooManyParts { limit: max_parts };
            issues.push(issue(&job, kind));
        }
    }
    jobs
}

/// Returns the body zones the piece covers; a Record with more chunks than are read is reported.
fn covered_zones(piece: &Piece, issues: &mut Vec<Issue>) -> Arc<[ZoneChunk]> {
    let Source::Wearable(wearable) = &piece.source else {
        return Arc::default();
    };
    if wearable.zones.truncated {
        let reason = "the Record lists more body zones than are read; the rest are ignored";
        let kind = IssueKind::BodyZonesUnknown(reason.to_owned());
        issues.push(piece_issue(piece, kind));
    }
    Arc::clone(&wearable.zones.chunks)
}
