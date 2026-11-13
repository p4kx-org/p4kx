// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, BufWriter, Write};

use anyhow::{Result, anyhow};
use p4kx_assembly::{ArchiveModels, Interior, InteriorOptions, SocContainerCapture};
use p4kx_blender::{
    InteriorScene, Issue, PackageInfo, PackageSummary, PackageWriteContext, SocScene,
    check_output_with_context, write_interior_package_with_context,
};
use p4kx_channels::channel_name;
use p4kx_extraction::{GlbOptions, is_same_file};
use serde::Serialize;

use crate::cli_interior::InteriorPackageArgs;
use crate::entity_package::build_info;
use crate::extract::cancel_on_interrupt;
use crate::interior_glb::{InteriorOutputIsArchive, open};
use crate::json;
use crate::query_table::one_line;

#[derive(Serialize)]
struct PackageJson<'a> {
    archive: String,
    containers: &'a [String],
    out: String,
    lod: u32,
    texture_mip: u32,
    containers_opened: usize,
    model_parts: usize,
    modelled_parts: usize,
    props: usize,
    nodes: usize,
    drawn_nodes: usize,
    container_nodes: usize,
    objects: usize,
    lights: usize,
    meshes: usize,
    shared_meshes: usize,
    textures: usize,
    texture_pixels: u64,
    texture_failures: usize,
    materials: usize,
    palettes: usize,
    files: usize,
    bytes: u64,
    vertices: usize,
    triangles: usize,
    skipped: usize,
    issues: &'a [Issue],
}

/// Runs `p4kx interior package`.
pub(crate) fn run(args: &InteriorPackageArgs, as_json: bool) -> Result<()> {
    if is_same_file(&args.out, &args.archive) {
        return Err(InteriorOutputIsArchive(args.out.clone()).into());
    }
    let (layers, chain) = open(&args.archive, &args.containers)?;
    let entry = chain.last().map_or("", String::as_str);
    let options = InteriorOptions::default().with_lod(args.lod);
    let interior = crate::interior_forge::build(&layers, entry, &options)?;
    let outer = layers
        .first()
        .ok_or_else(|| anyhow!("the Archive was not opened"))?;
    let cancel = cancel_on_interrupt()?;
    let protection = p4kx_extraction::ArchiveWriteProtection::new(outer.path(), cancel.flag())?;
    let context = PackageWriteContext::new(&protection, cancel.flag());
    check_output_with_context(&args.out, &context)?;
    let models = ArchiveModels::new(outer);
    let mut glb = GlbOptions::default()
        .with_lod(args.lod)
        .with_texture_mip(args.texture_mip);
    if args.no_vertex_colours {
        glb = glb.without_vertex_colours();
    }
    let info = PackageInfo::new(channel_name(outer.path()), build_info(outer)?);
    let name = name_of(entry);
    let authored = if args.authored_soc {
        let refs: Vec<_> = layers.iter().collect();
        let capture = SocContainerCapture::read(&refs, &cancel)?;
        Some(SocScene::from_archive_capture(&capture, outer, &cancel)?)
    } else {
        None
    };
    let mut scene = InteriorScene::new(&name, &interior);
    if let Some(authored) = &authored {
        scene = scene.with_authored_soc(authored);
    }
    let loader = models.textures();
    let context = context
        .with_texture_container(args.texture_container.container())
        .with_archive_texture_source(&loader);
    let summary = write_interior_package_with_context(
        &scene, &models, &loader, &glb, &info, &args.out, &context,
    )?;
    if as_json {
        json::print(&PackageJson::of(
            &json::path(&args.archive),
            &chain,
            &interior,
            args,
            &summary,
        ))?;
        return Ok(());
    }
    match print_report(&summary, args, entry) {
        Err(error) if error.kind() != io::ErrorKind::BrokenPipe => Err(error.into()),
        _ => Ok(()),
    }
}

fn name_of(entry: &str) -> String {
    let file = entry.rsplit(['\\', '/']).next().unwrap_or(entry);
    file.rsplit_once('.')
        .map_or(file, |(stem, _)| stem)
        .to_owned()
}

impl<'a> PackageJson<'a> {
    fn of(
        archive: &str,
        chain: &'a [String],
        interior: &Interior,
        args: &InteriorPackageArgs,
        summary: &'a PackageSummary,
    ) -> Self {
        Self {
            archive: archive.to_owned(),
            containers: chain,
            out: json::path(&args.out),
            lod: args.lod,
            texture_mip: args.texture_mip,
            containers_opened: interior.containers(),
            model_parts: interior.model_parts(),
            modelled_parts: interior.modelled_parts(),
            props: interior.props(),
            nodes: summary.nodes,
            drawn_nodes: summary.drawn_nodes,
            container_nodes: summary.containers,
            objects: summary.objects,
            lights: summary.lights,
            meshes: summary.meshes,
            shared_meshes: summary.shared_meshes,
            textures: summary.textures,
            texture_pixels: summary.texture_pixels,
            texture_failures: summary.texture_failures,
            materials: summary.materials,
            palettes: summary.palettes,
            files: summary.files,
            bytes: summary.bytes,
            vertices: summary.vertices,
            triangles: summary.triangles,
            skipped: summary.issues.iter().filter(|issue| issue.skipped).count(),
            issues: &summary.issues,
        }
    }
}

fn print_report(
    summary: &PackageSummary,
    args: &InteriorPackageArgs,
    entry: &str,
) -> io::Result<()> {
    let mut out = BufWriter::new(io::stdout().lock());
    writeln!(
        out,
        "wrote {} of {}: {} nodes ({} drawn), {} meshes, {} materials, {} palettes, {} textures, {} files, {} vertices, {} triangles, {} bytes",
        args.out.display(),
        one_line(entry),
        summary.nodes,
        summary.drawn_nodes,
        summary.meshes,
        summary.materials,
        summary.palettes,
        summary.textures,
        summary.files,
        summary.vertices,
        summary.triangles,
        summary.bytes
    )?;
    writeln!(
        out,
        "Interior: {} containers, {} objects, {} lights",
        summary.containers, summary.objects, summary.lights
    )?;
    if summary.shared_meshes > 0 {
        writeln!(
            out,
            "{} nodes draw a mesh another node draws; {} meshes are written once",
            summary.shared_meshes, summary.meshes
        )?;
    }
    if summary.texture_failures > 0 {
        writeln!(
            out,
            "{} textures could not be written; their materials keep the plain colours",
            summary.texture_failures
        )?;
    }
    for issue in &summary.issues {
        let what = if issue.skipped { "skipped" } else { "note" };
        let path = if issue.path.is_empty() {
            issue.name.clone()
        } else {
            format!("{} ({})", issue.path, issue.name)
        };
        writeln!(
            out,
            "{what} {}: {}",
            one_line(&path),
            one_line(&issue.reason)
        )?;
    }
    out.flush()
}
