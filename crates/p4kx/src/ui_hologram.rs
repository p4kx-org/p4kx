// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};
use std::process::ExitCode;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use p4kx_assembly::{
    ArchiveFiles, ArchiveModels, Assembly, AssemblyOptions, ForgePalettes, HologramCamera,
    LoadoutResolver, ObservedHologramCamera,
};
use p4kx_extraction::{ArchiveWriteProtection, CancelToken, DataForgeEntry, is_same_file};
use p4kx_render::{
    Adapters, AssemblyDraw, AssemblyScene, BuildOptions, Camera, Colours, DrawList, Frame,
    Hologram, HologramView, Hooks, Look, Offscreen, Picture, Released, Shading, Subject,
};
use p4kx_settings::ModelMaterials;
use serde::Serialize;

use crate::cli_hologram::HologramArgs;
use crate::entity_glb::GlbOutputIsArchive;
use crate::entity_glb_json::IssueJson;
use crate::{exit, json};

#[path = "ui_hologram_bone_origins.rs"]
mod bone_origins;

#[path = "ui_hologram_cdf_origins.rs"]
mod cdf_origins;

#[path = "ui_hologram_gear_origins.rs"]
mod gear_origins;

#[path = "ui_hologram_material_origins.rs"]
mod material_origins;

#[path = "ui_hologram_node_origins.rs"]
mod node_origins;

#[path = "ui_hologram_origins.rs"]
mod origins;

#[path = "ui_hologram_png_origins.rs"]
mod png_origins;
#[path = "ui_hologram_text.rs"]
mod text;

use self::origins::{CameraInputs, Evidence, Projection};

#[derive(Serialize)]
struct CameraJson {
    source: &'static str,
    fov_degrees: f32,
    view_angle_degrees: f32,
    elevation_degrees: f32,
    distance_scaler: f32,
}

#[derive(Serialize, Clone, Copy)]
struct TimesJson {
    assembly: u128,
    scene: u128,
    draw: u128,
    write: u128,
    total: u128,
}

#[derive(Serialize)]
struct HologramJson<'a> {
    archive: String,
    entity: &'a str,
    guid: String,
    out: String,
    width: u32,
    height: u32,
    bytes: u64,
    parts: usize,
    modelled_parts: usize,
    drawn_parts: usize,
    meshes: usize,
    triangles: usize,
    camera: CameraJson,
    adapter: &'a str,
    milliseconds: TimesJson,
    #[serde(skip_serializing_if = "<[String]>::is_empty")]
    ignored: &'a [String],
    #[serde(skip_serializing_if = "Vec::is_empty")]
    incompatible: Vec<crate::entity_ignored::IncompatibleJson<'a>>,
    issues: Vec<IssueJson<'a>>,
    #[serde(flatten)]
    evidence: Evidence<'a>,
}

/// Returns the view of the camera the options and the Record ask for, and where it came from.
fn view_of(args: &HologramArgs, record: Option<HologramCamera>) -> (HologramView, &'static str) {
    let camera = record.unwrap_or_default();
    let view = HologramView {
        fov_degrees: args.fov.unwrap_or_else(|| camera.fov_degrees()),
        view_angle_degrees: args
            .view_angle
            .unwrap_or_else(|| camera.view_angle_degrees()),
        elevation_degrees: args.elevation,
        distance_scaler: args
            .distance_scaler
            .unwrap_or_else(|| camera.distance_scaler()),
    };
    let asked = args.fov.is_some() || args.view_angle.is_some() || args.distance_scaler.is_some();
    let source = match (record.is_some(), asked) {
        (true, false) => "record",
        (true, true) => "record and options",
        (false, true) => "defaults and options",
        (false, false) => "defaults",
    };
    (view.sanitised(), source)
}

#[expect(
    clippy::cast_precision_loss,
    reason = "a picture side is at most 16384, far below the 2^24 an f32 holds exactly"
)]
fn aspect(size: [u32; 2]) -> f32 {
    size[0] as f32 / size[1].max(1) as f32
}

#[expect(
    clippy::cast_precision_loss,
    reason = "a picture side is at most 16384, far below the 2^24 an f32 holds exactly"
)]
const fn height(size: [u32; 2]) -> f32 {
    size[1] as f32
}

fn frame(
    scene: &Arc<AssemblyScene>,
    view: &HologramView,
    args: &HologramArgs,
) -> (Frame, Vec<usize>) {
    let mut camera = Camera::holographic(scene.bounds(), view);
    camera.fit(aspect(args.size));
    let uniforms = camera.uniforms(aspect(args.size));
    let hidden = vec![false; scene.parts().len()];
    let (list, selected) =
        DrawList::of_observed(scene, &hidden, &uniforms.view_proj, height(args.size)).into_parts();
    let frame = Frame {
        subject: Subject::Assembly(AssemblyDraw {
            scene: Arc::clone(scene),
            list: Arc::new(list),
        }),
        uniforms,
        size: args.size,
        look: Look::default(),
        colours: Colours {
            background: args.background,
            ..Colours::default()
        },
        shading: Shading::Hologram(Hologram {
            tint: args.tint,
            opacity: args.opacity,
            glow: args.glow,
        }),
    };
    (frame, selected)
}

const fn millis(time: Duration) -> u128 {
    time.as_millis()
}

struct Outcome {
    scene: AssemblyScene,
    picture: Picture,
    bytes: u64,
    adapter: String,
    camera: (HologramView, &'static str),
    times: TimesJson,
    published: png_origins::Published,
}

impl Outcome {
    fn summary<'a>(
        &'a self,
        args: &'a HologramArgs,
        assembly: &'a Assembly,
        applied: &'a p4kx_assembly::ChangedLoadout,
    ) -> Summary<'a> {
        Summary {
            args,
            class_name: applied.node().class_name(),
            picture: &self.picture,
            bytes: self.bytes,
            assembly,
            applied,
            scene: &self.scene,
            adapter: &self.adapter,
            camera: self.camera,
            times: self.times,
        }
    }
}

struct HologramInputs<'a> {
    projection: &'a Projection,
    camera: &'a ObservedHologramCamera,
    settings: &'a CameraInputs,
}

fn draw_picture(
    args: &HologramArgs,
    (assembly, class_name): (&Assembly, &str),
    original: &HologramInputs<'_>,
    (started, assembled): (Instant, Duration),
    protection: &ArchiveWriteProtection,
    cancel: &CancelToken,
) -> Result<Outcome> {
    let camera = view_of(args, original.camera.camera());
    let scene = AssemblyScene::build_observed(
        assembly,
        &BuildOptions {
            materials: ModelMaterials::Colours,
            textures: None,
        },
        &Hooks {
            cancelled: &|| cancel.is_cancelled(),
            progress: &|_| {},
        },
    )
    .with_context(|| format!("the hologram of {class_name} cannot be drawn"))?;
    let scene = Arc::new(scene);
    let built = started.elapsed();
    let mut offscreen = Offscreen::new(if args.software {
        Adapters::Software
    } else {
        Adapters::Best
    })?;
    let (frame, visible) = frame(&scene, &camera.0, args);
    let picture = offscreen.draw(&frame)?;
    drop(frame);
    let drawn = started.elapsed();
    let published = png_origins::publish(
        &picture,
        &args.out,
        (protection, cancel),
        original,
        (assembly, &scene, &visible),
    )?;
    let bytes = published.bytes;
    let total = started.elapsed();
    let adapter = offscreen.adapter_name().to_owned();
    if let Released::Abandoned(stage) = offscreen.close() {
        writeln!(
            io::stderr().lock(),
            "warning: the graphics driver did not release the device in time ({stage}); the picture is written"
        )?;
    }
    Ok(Outcome {
        scene: Arc::into_inner(scene).context("the scene is still shared")?,
        picture,
        bytes,
        adapter,
        camera,
        published,
        times: TimesJson {
            assembly: millis(assembled),
            scene: millis(built.saturating_sub(assembled)),
            draw: millis(drawn.saturating_sub(built)),
            write: millis(total.saturating_sub(drawn)),
            total: millis(total),
        },
    })
}
/// Runs `p4kx ui hologram`.
pub(crate) fn run(args: &HologramArgs, as_json: bool) -> Result<ExitCode> {
    let started = Instant::now();
    if is_same_file(&args.out, &args.archive) {
        return Err(GlbOutputIsArchive(args.out.clone()).into());
    }
    let changes = args.changes.changes()?;
    let archive = crate::key::open(&args.archive)?;
    let cancel = crate::extract::cancel_on_interrupt()?;
    let protection = ArchiveWriteProtection::new(archive.path(), cancel.flag())?;
    crate::guard::ensure_writable(&args.out)?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let files = ArchiveFiles::new(&archive);
    let mut resolver = LoadoutResolver::new(&forge)
        .with_files(&files)
        .stopping_at_docked_vehicles();
    let record = resolver.entities().find(&args.entity)?;
    let applied = resolver.resolve_changed(&record, &changes)?;
    crate::entity_ignored::warn(&applied)?;
    let tree = applied.node();
    let models = ArchiveModels::new(&archive).with_model_node_sources();
    let options = AssemblyOptions::default().with_lod(args.lod);
    let palettes = ForgePalettes::new(&forge);
    let assembly = Assembly::build_with_palettes(tree, &models, &palettes, &options)?;
    let camera = HologramCamera::read_observed(&forge);
    let evidence = Projection::new(&entry)?;
    let settings = evidence.camera_inputs(&camera, args)?;
    let outcome = draw_picture(
        args,
        (&assembly, tree.class_name()),
        &HologramInputs {
            projection: &evidence,
            camera: &camera,
            settings: &settings,
        },
        (started, started.elapsed()),
        &protection,
        &cancel,
    )?;
    let summary = outcome.summary(args, &assembly, &applied);
    if as_json {
        let evidence = evidence.finish(
            record.index(),
            &camera,
            &settings,
            &summary,
            &outcome.published,
        )?;
        json::print(&summary.json(&archive, tree.guid().to_string(), evidence))?;
        return Ok(ExitCode::from(exit::SUCCESS));
    }
    match summary.print() {
        Err(error) if error.kind() != io::ErrorKind::BrokenPipe => Err(error.into()),
        _ => Ok(ExitCode::from(exit::SUCCESS)),
    }
}

struct Summary<'a> {
    args: &'a HologramArgs,
    class_name: &'a str,
    picture: &'a Picture,
    bytes: u64,
    assembly: &'a Assembly,
    applied: &'a p4kx_assembly::ChangedLoadout,
    scene: &'a AssemblyScene,
    adapter: &'a str,
    camera: (HologramView, &'static str),
    times: TimesJson,
}

impl<'a> Summary<'a> {
    fn json(
        &'a self,
        archive: &p4kx_extraction::Archive,
        guid: String,
        evidence: Evidence<'a>,
    ) -> HologramJson<'a> {
        let (view, source) = self.camera;
        HologramJson {
            archive: json::path(archive.path()),
            entity: self.class_name,
            guid,
            out: json::path(&self.args.out),
            width: self.picture.width(),
            height: self.picture.height(),
            bytes: self.bytes,
            parts: self.assembly.parts().len(),
            modelled_parts: self.assembly.modelled_parts(),
            drawn_parts: self.scene.drawn_parts(),
            meshes: self.scene.meshes(),
            triangles: self.scene.placed_triangles(),
            camera: CameraJson {
                source,
                fov_degrees: view.fov_degrees,
                view_angle_degrees: view.view_angle_degrees,
                elevation_degrees: view.elevation_degrees,
                distance_scaler: view.distance_scaler,
            },
            adapter: self.adapter,
            milliseconds: self.times,
            ignored: self.applied.ignored(),
            incompatible: crate::entity_ignored::incompatible(self.applied),
            issues: self.assembly.issues().iter().map(IssueJson::from).collect(),
            evidence,
        }
    }
}
