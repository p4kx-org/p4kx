// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};

use anyhow::Result;
use p4kx_assembly::character::{CHF_SIZE, Chf};
use p4kx_assembly::{
    AnimationFolders, ArchiveModels, Armour, AssemblyOptions, CharacterAnimations, Expressions,
    ForgeItems, ForgePalettes, Head, IssueKind, LoadoutResolver, PartOrigin,
    character_animations_with_archive_sources,
};
use p4kx_extraction::{
    AnimationSet, DataForgeEntry, GlbOptions, GlbSummary, MaterialMode, is_same_file,
};
use thiserror::Error;

use crate::chf::{InvalidChf, read_capped_observed};
use crate::cli::ChfGlbArgs;
use crate::cli_chf::ChfMaterialsArg;
use crate::entity_glb::GlbOutputIsArchive;
use crate::entity_glb_report::{print_issues, print_textures};
use crate::model_animations::write_notes;
use crate::query_table::one_line;

#[path = "chf_glb_sources.rs"]
mod origin;

#[path = "chf_glb_report.rs"]
mod report;

use origin::Export;
use report::Outcome;

/// Parts of the head of the character could not be shaped by its DNA, so the file lacks them.
#[derive(Debug, Error)]
#[error("{} could not be shaped by its DNA and is not in {}", .parts.join(", "), .out.display())]
pub(crate) struct HeadNotShaped {
    parts: Vec<String>,
    out: PathBuf,
}

const fn origin_code(origin: PartOrigin) -> &'static str {
    match origin {
        PartOrigin::Body => "body",
        _ => "head",
    }
}

/// Some expressions of the head could not be made, so the file lacks them.
#[derive(Debug, Error)]
#[error("some expressions of the head could not be made and are not in {}", .0.display())]
pub(crate) struct ExpressionsNotMade(PathBuf);

fn expressions_of(args: &ChfGlbArgs) -> Expressions {
    expressions_choice(&args.expressions, args.no_expressions)
}

/// Returns the expressions the options ask for: none with `none`, those named, else all.
pub(crate) fn expressions_choice(named: &[String], none: bool) -> Expressions {
    if none {
        Expressions::None
    } else if !named.is_empty() {
        Expressions::Only(named.to_vec())
    } else {
        Expressions::All
    }
}

fn glb_options(args: &ChfGlbArgs) -> GlbOptions {
    let mode = match args.materials {
        ChfMaterialsArg::Colours => MaterialMode::Colours,
        ChfMaterialsArg::Textures => MaterialMode::Textures,
    };
    let glb = GlbOptions::default()
        .with_lod(args.lod)
        .with_materials(mode)
        .with_texture_mip(args.texture_mip);
    if args.no_vertex_colours {
        glb.without_vertex_colours()
    } else {
        glb
    }
}

/// Builds the Armour of `head` at the level of detail `lod`, drawn with its tint palettes.
fn armour_of(
    head: &Head,
    models: &ArchiveModels<'_>,
    palettes: ForgePalettes<'_>,
    lod: u32,
) -> Result<Armour> {
    let options = AssemblyOptions::default().with_lod(lod);
    Ok(Armour::build_with_palettes(
        head.variant(),
        head.pieces(),
        models,
        &palettes,
        &options,
    )?)
}

/// Runs `p4kx chf glb`.
pub(crate) fn run(args: &ChfGlbArgs, as_json: bool) -> Result<()> {
    if is_same_file(&args.out, &args.archive) {
        return Err(GlbOutputIsArchive(args.out.clone()).into());
    }
    let input = read_capped_observed(&args.file, CHF_SIZE as u64)?;
    let chf = Chf::read(input.bytes()).map_err(|source| InvalidChf {
        path: args.file.clone(),
        source,
    })?;
    let archive = crate::key::open(&args.archive)?;
    let cancel = crate::extract::cancel_on_interrupt()?;
    let write_guard = p4kx_extraction::ArchiveWriteProtection::new(archive.path(), cancel.flag())?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let mut resolver = LoadoutResolver::new(&forge);
    let head = Head::dressed(&chf.character, &mut ForgeItems::new(&mut resolver))?
        .with_expressions(&expressions_of(args));
    let models = ArchiveModels::new(&archive);
    let armour = armour_of(&head, &models, ForgePalettes::new(&forge), args.lod)?;

    let glb = glb_options(args);
    let textures = models.textures().observed();
    let found = character_animations_with_archive_sources(
        &models,
        head.variant(),
        (&args.animations, &args.animation_sources),
        &AnimationFolders::default(),
    )?;
    let animated = armour.animate(&found.set().animations, &models);
    let export = Export {
        name: head.variant().code(),
        armour: &armour,
        animated: &animated,
        found: &found,
        options: &glb,
        textures: &textures,
        input: &input,
        forge: entry.origin(),
    };
    let (summary, projection, receipt) =
        origin::publish(args, archive.path(), &write_guard, &cancel, &export)?;

    if as_json {
        let outcome = Outcome {
            armour: &armour,
            animated: &animated,
            summary: &summary,
        };
        report::print(
            archive.path(),
            args,
            &head,
            &outcome,
            projection.summary(receipt),
        )?;
    } else if let Err(error) = print_report(&armour, &head, &summary, args)
        .and_then(|()| print_animations(&summary, found.set(), &animated))
        && error.kind() != io::ErrorKind::BrokenPipe
    {
        return Err(error.into());
    }
    failure(&armour, &args.out).map_or(Ok(()), Err)
}

/// Writes the notes about the animations: what was written, what was left out and what could not
/// be done.
fn print_animations(
    summary: &GlbSummary,
    found: &AnimationSet,
    animated: &CharacterAnimations<'_>,
) -> io::Result<()> {
    let mut stdout = BufWriter::new(io::stdout().lock());
    write_notes(&mut stdout, &summary.animations, found)?;
    print_issues(&mut stdout, animated.issues())?;
    stdout.flush()
}

/// Returns the error for what could not be shaped or made, if there is any.
pub(crate) fn failure(armour: &Armour, out: &Path) -> Option<anyhow::Error> {
    let parts: Vec<String> = armour
        .issues()
        .iter()
        .filter(|issue| matches!(issue.kind(), IssueKind::ShapeFailed { .. }))
        .map(|issue| issue.name().to_owned())
        .collect();
    if !parts.is_empty() {
        return Some(
            HeadNotShaped {
                parts,
                out: out.to_path_buf(),
            }
            .into(),
        );
    }
    armour
        .issues()
        .iter()
        .any(|issue| {
            matches!(
                issue.kind(),
                IssueKind::MorphsFailed { .. }
                    | IssueKind::UnknownExpression { .. }
                    | IssueKind::EmptyExpression { .. }
            )
        })
        .then(|| ExpressionsNotMade(out.to_path_buf()).into())
}

fn print_report(
    armour: &Armour,
    head: &Head,
    summary: &GlbSummary,
    args: &ChfGlbArgs,
) -> io::Result<()> {
    let mut stdout = BufWriter::new(io::stdout().lock());
    writeln!(
        stdout,
        "wrote {} with the head of {} on the {} body: {} parts, {} bones, {} nodes, {} meshes ({} skinned), {} primitives, {} materials, {} morph targets, {} vertices, {} triangles, {} bytes",
        args.out.display(),
        head.rig(),
        head.variant().code(),
        armour.parts().len(),
        summary.bones,
        summary.nodes,
        summary.meshes,
        summary.skinned_meshes,
        summary.primitives,
        summary.materials,
        summary.morph_targets,
        summary.vertices,
        summary.triangles,
        summary.bytes
    )?;
    for part in armour.parts() {
        writeln!(
            stdout,
            "part {}: {}",
            one_line(part.name()),
            one_line(part.file())
        )?;
    }
    print_textures(&mut stdout, summary)?;
    if summary.default_material_ids > 0 {
        writeln!(
            stdout,
            "{} material IDs are drawn with the engine's default material",
            summary.default_material_ids
        )?;
    }
    if summary.lod_fallbacks > 0 {
        writeln!(
            stdout,
            "{} models lack LOD{} and are written at the nearest lower level",
            summary.lod_fallbacks,
            armour.lod()
        )?;
    }
    print_issues(&mut stdout, armour.issues())?;
    stdout.flush()
}

#[cfg(test)]
mod tests {
    use p4kx_assembly::Expressions;

    use super::expressions_choice;

    #[test]
    fn expressions_default_to_all_and_the_options_narrow_them() {
        let named = vec!["jaw_open".to_owned()];

        assert_eq!(expressions_choice(&[], false), Expressions::All);
        assert_eq!(
            expressions_choice(&named, false),
            Expressions::Only(named.clone())
        );
        assert_eq!(expressions_choice(&named, true), Expressions::None);
        assert_eq!(expressions_choice(&[], true), Expressions::None);
    }
}
