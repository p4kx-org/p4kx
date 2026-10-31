// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, BufWriter, Write};
use std::path::Path;

use anyhow::Result;
use p4kx_assembly::{
    AnimationFolders, ArchiveFiles, ArchiveModels, Armour, AssemblyOptions, BodyVariant,
    CharacterAnimations, ForgePalettes, Issue, LoadoutResolver, PartOrigin, character_animations,
};
use p4kx_extraction::{
    AnimationSet, ConversionError, DataForgeEntry, GlbOptions, GlbSummary, GltfError, MaterialMode,
    is_same_file, write_replacing,
};
use serde::Serialize;

use crate::armour_loadout::{Files, HeadJson, LoadoutJson, ReplacedJson, Selection};
use crate::chf_glb::expressions_choice;
use crate::cli_armour::{ArmourGlbArgs, BodyArg};
use crate::cli_chf::ChfMaterialsArg;
use crate::entity_glb::GlbOutputIsArchive;
use crate::entity_glb_json::IssueJson;
use crate::entity_glb_report::{print_issues, print_textures};
use crate::json;
use crate::layers_json::LayersJson;
use crate::model_animations::{WrittenJson, write_notes};
use crate::query_table::one_line;

#[derive(Serialize)]
struct PartJson<'a> {
    name: &'a str,
    origin: &'static str,
    file: &'a str,
    material: Option<&'a str>,
}

#[derive(Serialize)]
struct GlbJson<'a> {
    archive: String,
    body: &'static str,
    skeleton: &'a str,
    out: String,
    lod: u32,
    loadout: Option<&'a LoadoutJson<'a>>,
    head: Option<HeadJson>,
    pieces: usize,
    replaced: Vec<ReplacedJson<'a>>,
    parts: Vec<PartJson<'a>>,
    nodes: usize,
    bones: usize,
    meshes: usize,
    skinned_meshes: usize,
    primitives: usize,
    materials: usize,
    textured_materials: usize,
    textures: usize,
    texture_pixels: u64,
    vertices: usize,
    triangles: usize,
    bytes: u64,
    skipped: usize,
    #[serde(flatten)]
    layers: LayersJson,
    animations: Vec<WrittenJson>,
    issues: Vec<IssueJson<'a>>,
}

/// Resolves the pieces and the Loadout the arguments name; the head of the Loadout's character is
/// written with the expressions they ask for.
fn gather<'a>(
    resolver: &mut LoadoutResolver<'_>,
    files: &Files<'_>,
    args: &'a ArmourGlbArgs,
) -> Result<Selection<'a>> {
    let names = (args.pieces.as_slice(), args.loadout.as_deref());
    let expressions = expressions_choice(&args.expressions, args.no_expressions);
    Selection::gather(resolver, files, names, &expressions)
}

/// Runs `p4kx armour glb`.
pub(crate) fn run(args: &ArmourGlbArgs, as_json: bool) -> Result<()> {
    if is_same_file(&args.out, &args.archive) {
        return Err(GlbOutputIsArchive(args.out.clone()).into());
    }
    let archive = crate::key::open(&args.archive)?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let files = Files::open(ArchiveFiles::new(&archive), args.loadout.as_deref())?;
    let mut resolver = LoadoutResolver::new(&forge).with_files(&files);
    let selection = gather(&mut resolver, &files, args)?;
    let variant = args
        .body
        .map(BodyVariant::from)
        .or_else(|| selection.variant())
        .unwrap_or_else(|| BodyVariant::from(BodyArg::Male));
    let models = ArchiveModels::new(&archive);
    let options = AssemblyOptions::default().with_lod(args.lod);
    let armour = Armour::build_with_palettes(
        variant,
        &selection.pieces,
        &models,
        &ForgePalettes::new(&forge),
        &options,
    )?;
    let folders = animation_folders(args, &selection, &models, variant);
    let found = character_animations(
        &models,
        variant,
        (&args.animations, &args.animation_sources),
        &folders,
    )?;
    let animated = armour.animate(&found.animations, &models);
    let summary = write(&armour, &animated, args, &models)?;
    let layers = match args.materials {
        ChfMaterialsArg::Textures => armour.layer_issues(),
        ChfMaterialsArg::Colours => Vec::new(),
    };
    if as_json {
        let done = Done {
            archive: archive.path(),
            variant,
            selection: &selection,
            armour: &armour,
            animated: &animated,
            summary: &summary,
            layers: &layers,
        };
        json::print(&report(&done, args))?;
        return Ok(());
    }
    let own = folders
        .own()
        .filter(|_| !args.animations.is_empty())
        .map(one_line);
    let printed = print_report(&armour, &summary, &selection, (&args.out, &layers))
        .and_then(|()| print_animations(&summary, &found, &animated, own.as_deref()));
    match printed {
        Err(error) if error.kind() != io::ErrorKind::BrokenPipe => Err(error.into()),
        _ => Ok(()),
    }
}

/// What was built and written.
struct Done<'a> {
    archive: &'a Path,
    variant: BodyVariant,
    selection: &'a Selection<'a>,
    armour: &'a Armour,
    animated: &'a CharacterAnimations<'a>,
    summary: &'a GlbSummary,
    layers: &'a [Issue],
}

fn report<'a>(done: &Done<'a>, args: &ArmourGlbArgs) -> GlbJson<'a> {
    let Done {
        archive,
        variant,
        selection,
        armour,
        animated,
        summary,
        layers,
    } = *done;
    GlbJson {
        archive: json::path(archive),
        body: variant.code(),
        skeleton: armour.skeleton_file(),
        out: json::path(&args.out),
        lod: args.lod,
        loadout: selection.loadout.as_ref(),
        head: selection.head_json(),
        pieces: selection.pieces.len(),
        replaced: selection.replaced.iter().map(ReplacedJson::from).collect(),
        parts: parts_json(armour),
        nodes: summary.nodes,
        bones: summary.bones,
        meshes: summary.meshes,
        skinned_meshes: summary.skinned_meshes,
        primitives: summary.primitives,
        materials: summary.materials,
        textured_materials: summary.textured_materials,
        textures: summary.textures,
        texture_pixels: summary.texture_pixels,
        vertices: summary.vertices,
        triangles: summary.triangles,
        bytes: summary.bytes,
        skipped: skipped(armour),
        layers: LayersJson::from(&armour.unresolved_layers()),
        animations: summary.animations.iter().map(WrittenJson::of).collect(),
        issues: armour
            .issues()
            .iter()
            .chain(layers)
            .chain(animated.issues())
            .map(IssueJson::from)
            .collect(),
    }
}

/// Returns the folders `--anim` looks in: those of `--anim-folder`, else the character's own
/// folder of a Loadout's head, else the folders of the body.
fn animation_folders(
    args: &ArmourGlbArgs,
    selection: &Selection<'_>,
    models: &ArchiveModels<'_>,
    variant: BodyVariant,
) -> AnimationFolders {
    if !args.animation_folders.is_empty() {
        return AnimationFolders::only(&args.animation_folders);
    }
    selection
        .head
        .as_ref()
        .and_then(|head| head.folder())
        .map_or_else(AnimationFolders::default, |folder| {
            AnimationFolders::of_npc(models, variant, folder)
        })
}

fn skipped(armour: &Armour) -> usize {
    armour
        .issues()
        .iter()
        .filter(|issue| issue.kind().skips())
        .count()
}

fn parts_json(armour: &Armour) -> Vec<PartJson<'_>> {
    armour
        .parts()
        .iter()
        .map(|part| PartJson {
            name: part.name(),
            origin: origin_code(part.origin()),
            file: part.file(),
            material: part
                .materials()
                .library()
                .map(|library| library.path.as_str()),
        })
        .collect()
}

/// Writes the notes about the animations: what was written, what was left out and what could not
/// be done.
fn print_animations(
    summary: &GlbSummary,
    found: &AnimationSet,
    animated: &CharacterAnimations<'_>,
    own: Option<&str>,
) -> io::Result<()> {
    let mut stdout = BufWriter::new(io::stdout().lock());
    if let Some(own) = own {
        writeln!(stdout, "animations are looked for in {own} first")?;
    }
    write_notes(&mut stdout, &summary.animations, found)?;
    print_issues(&mut stdout, animated.issues())?;
    stdout.flush()
}

const fn origin_code(origin: PartOrigin) -> &'static str {
    match origin {
        PartOrigin::Body => "body",
        _ => "piece",
    }
}

fn write(
    armour: &Armour,
    animated: &CharacterAnimations<'_>,
    args: &ArmourGlbArgs,
    models: &ArchiveModels<'_>,
) -> Result<GlbSummary> {
    let mode = match args.materials {
        ChfMaterialsArg::Colours => MaterialMode::Colours,
        ChfMaterialsArg::Textures => MaterialMode::Textures,
    };
    let mut options = GlbOptions::default()
        .with_lod(args.lod)
        .with_materials(mode)
        .with_texture_mip(args.texture_mip);
    if args.no_vertex_colours {
        options = options.without_vertex_colours();
    }
    let out: &Path = &args.out;
    let name = armour.variant().code().to_owned();
    let textures = models.textures();
    Ok(write_replacing(out, |writer| {
        armour
            .write_glb_animated_with(animated, &options, &textures, writer)
            .map_err(|error| match error {
                GltfError::Write(source) => ConversionError::Write {
                    path: out.to_path_buf(),
                    source,
                },
                source => ConversionError::Glb {
                    file: name.clone(),
                    source,
                },
            })
    })?)
}

fn print_report(
    armour: &Armour,
    summary: &GlbSummary,
    selection: &Selection<'_>,
    (out, layers): (&Path, &[Issue]),
) -> io::Result<()> {
    let mut stdout = BufWriter::new(io::stdout().lock());
    selection.print(&mut stdout)?;
    writeln!(
        stdout,
        "wrote {} on the {} body: {} pieces, {} parts ({} of the body), {} bones, {} nodes, {} meshes ({} skinned), {} primitives, {} materials, {} vertices, {} triangles, {} bytes",
        out.display(),
        armour.variant().code(),
        selection.pieces.len(),
        armour.parts().len(),
        armour.count(PartOrigin::Body),
        summary.bones,
        summary.nodes,
        summary.meshes,
        summary.skinned_meshes,
        summary.primitives,
        summary.materials,
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
    if summary.lod_fallbacks > 0 {
        writeln!(
            stdout,
            "{} models lack LOD{} and are written at the nearest lower level",
            summary.lod_fallbacks,
            armour.lod()
        )?;
    }
    if summary.hidden_primitives > 0 {
        writeln!(
            stdout,
            "left out {} submeshes whose material draws nothing",
            summary.hidden_primitives
        )?;
    }
    print_issues(&mut stdout, armour.issues())?;
    print_issues(&mut stdout, layers)?;
    stdout.flush()
}
