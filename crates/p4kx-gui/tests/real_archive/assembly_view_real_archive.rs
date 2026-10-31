// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The 3D view of the Entities tab over capital ships of a real Archive: the Assembly is built,
//! culled and drawn within bounded work.

use std::error::Error;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::software_wgpu::software_wgpu_setup;
use eframe::egui_wgpu::RendererOptions;
use egui_kittest::Harness;
use egui_kittest::wgpu::{WgpuTestRenderer, create_render_state};
use p4kx_gui::{
    AssemblyView, EntitiesStatus, GpuReport, Language, LoadoutSession, Model, ViewerSupport,
    WorkTab, install_viewer, ui,
};
use p4kx_settings::ModelMaterials;
use p4kx_testkit::{real_archive_key, real_archive_path};

type TestResult = Result<(), Box<dyn Error>>;

const SHIPS: [&str; 2] = ["RSI_Polaris", "AEGS_Idris_P"];
const BUILD_LIMIT: Duration = Duration::from_secs(300);
const FRAME_BUDGET: Duration = Duration::from_millis(8);

#[expect(
    clippy::print_stderr,
    reason = "tells the test run what a capital ship costs"
)]
fn note(text: &str) {
    eprintln!("{text}");
}

fn opened(path: PathBuf) -> Result<Model, Box<dyn Error>> {
    let mut model = Model::new(Language::BritishEnglish);
    model.set_archive_key(Some(real_archive_key()?));
    model.open(path, || {});
    let deadline = Instant::now() + Duration::from_secs(600);
    while model.browser().is_none() {
        if Instant::now() > deadline {
            return Err("the Archive did not open in time".into());
        }
        std::thread::sleep(Duration::from_millis(20));
        model.poll();
    }
    Ok(model)
}

fn view(model: &Model) -> Option<&AssemblyView> {
    model.entities().loadout().map(LoadoutSession::assembly)
}

fn pump(model: &mut Model) {
    model.poll();
    model.entities_mut().poll();
    std::thread::sleep(Duration::from_millis(10));
}

fn select_ship(model: &mut Model, class: &str) -> Result<bool, Box<dyn Error>> {
    model.set_work_tab(WorkTab::Entities);
    model.load_entities(|| {});
    let deadline = Instant::now() + Duration::from_secs(300);
    while !matches!(model.entities().status(), EntitiesStatus::Ready(_)) {
        if Instant::now() > deadline {
            return Err("the Entities were not listed in time".into());
        }
        pump(model);
    }
    model.entities_mut().set_query(class);
    let EntitiesStatus::Ready(list) = model.entities().status() else {
        return Err("no list".into());
    };
    let Some(at) = (0..list.shown_len()).find(|&at| {
        list.get(at)
            .is_some_and(|entity| entity.class_name() == class)
    }) else {
        return Ok(false);
    };
    model.select_entity(at, || {});
    let deadline = Instant::now() + BUILD_LIMIT;
    loop {
        if Instant::now() > deadline {
            return Err("the view was not built in time".into());
        }
        pump(model);
        if view(model).is_some_and(|view| !view.is_building() && view.scene().is_some())
            && model
                .entities()
                .loadout()
                .is_some_and(|session| !session.is_resolving())
        {
            return Ok(true);
        }
        if let Some(failure) = view(model).and_then(AssemblyView::failure) {
            return Err(format!("the view failed: {failure:?}").into());
        }
    }
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
#[expect(clippy::too_many_lines, reason = "one scenario per ship")]
fn a_capital_ship_is_built_culled_and_instanced_within_bounded_work() -> TestResult {
    let mut model = opened(real_archive_path()?)?;
    let mut measured = 0;
    for class in SHIPS {
        let started = Instant::now();
        if !select_ship(&mut model, class)? {
            note(&format!("{class}: not in this Archive"));
            continue;
        }
        let built = started.elapsed();
        let view = view(&model).ok_or("no view")?;
        let scene = view.scene().cloned().ok_or("no scene")?;
        let issues = view.issues();
        let mut camera = view.camera().cloned().ok_or("no camera")?;
        camera.fit(1.6);
        let view = model
            .entities_mut()
            .loadout_mut()
            .ok_or("no loadout")?
            .assembly_mut();

        let mut worst = Duration::ZERO;
        let mut visible = 0;
        let mut calls = 0;
        for step in 0..200_u16 {
            camera.orbit(f32::from(step % 7) * 11.0, f32::from(step % 5) * 3.0);
            if step % 10 == 0 {
                camera.reset(1.6);
            }
            let matrix = camera.uniforms(1.6).view_proj;
            let taken = Instant::now();
            let list = view.draw_list(&matrix, 900.0).ok_or("no list")?;
            worst = worst.max(taken.elapsed());
            visible = list.visible_parts();
            calls = list.draws().len();
            assert!(
                list.instances().len() <= scene.item_count(),
                "no more instances than draw items"
            );
        }

        note(&format!(
            "{class}: built in {built:.1?}; {parts} parts ({drawn} drawn, {issues} not resolved), \
             {meshes} shared meshes, {stored} triangles stored of {placed} placed; \
             culling worst case {worst:.2?} for {visible} visible parts in {calls} draws",
            parts = scene.parts().len(),
            drawn = scene.drawn_parts(),
            meshes = scene.meshes(),
            stored = scene.triangle_count(),
            placed = scene.placed_triangles(),
        ));
        assert!(scene.drawn_parts() > 50, "a capital ship has many parts");
        assert!(scene.meshes() < scene.drawn_parts(), "parts share meshes");
        assert!(
            scene.placed_triangles() > scene.triangle_count(),
            "instancing stores fewer triangles than the ship has"
        );
        assert!(issues < scene.parts().len(), "most parts resolve");
        assert!(
            worst < FRAME_BUDGET,
            "picking the parts of a frame: {worst:?}"
        );
        measured += 1;
    }
    assert!(measured > 0, "at least one capital ship is in the Archive");
    Ok(())
}

fn save_png(
    image: &[u8],
    (width, height): (u32, u32),
    name: &str,
) -> Result<PathBuf, Box<dyn Error>> {
    let folder = std::env::var_os("P4KX_RENDER_DIR")
        .map_or_else(|| std::env::temp_dir().join("p4kx-render"), PathBuf::from);
    std::fs::create_dir_all(&folder)?;
    let path = folder.join(name);
    let file = std::io::BufWriter::new(std::fs::File::create(&path)?);
    let mut encoder = png::Encoder::new(file, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(image)?;
    Ok(path)
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
#[expect(
    clippy::too_many_lines,
    reason = "one scenario from selecting to rendering"
)]
fn a_capital_ship_is_drawn_in_the_window() -> TestResult {
    let Ok(state) = std::panic::catch_unwind(|| {
        create_render_state(software_wgpu_setup(), RendererOptions::PREDICTABLE)
    }) else {
        note("no graphics adapter: the test checks nothing");
        return Ok(());
    };
    let report = install_viewer(&state)?;
    let mut model = opened(real_archive_path()?)?;
    model.set_viewer_support(ViewerSupport::Ready(Arc::clone(&report)));
    let mut chosen = None;
    for class in SHIPS {
        if select_ship(&mut model, class)? {
            chosen = Some(class);
            break;
        }
    }
    let class = chosen.ok_or("no capital ship in the Archive")?;
    let lightest = *p4kx_gui::ASSEMBLY_LODS.last().ok_or("no levels")?;
    model
        .entities_mut()
        .loadout_mut()
        .ok_or("no loadout")?
        .assembly_mut()
        .set_lod(lightest);
    let deadline = Instant::now() + BUILD_LIMIT;
    while view(&model).is_some_and(AssemblyView::is_building) {
        if Instant::now() > deadline {
            return Err("the view was not built in time".into());
        }
        pump(&mut model);
    }
    let mut harness = Harness::builder()
        .with_context_setup(p4kx_gui::theme::apply)
        .with_size([2400.0, 1500.0])
        .renderer(WgpuTestRenderer::from_render_state(state))
        .build_ui_state(
            |ui, model: &mut Model| {
                ui::show(ui, model);
            },
            model,
        );
    harness.run_steps(5);

    let started = Instant::now();
    let image = harness.render()?;
    let first = started.elapsed();
    harness.run_steps(2);
    let started = Instant::now();
    let _again = harness.render()?;
    let repeat = started.elapsed();

    assert_eq!(report.uploads(), 1, "uploaded once");
    assert_eq!(report.buffers(), 3, "one vertex, index and instance buffer");
    assert!(report.instances() > report.draw_calls(), "instanced");
    assert!(!report.is_lost(), "the device is fine");
    let path = save_png(
        image.as_raw(),
        (image.width(), image.height()),
        &format!("real-{class}.png"),
    )?;
    note(&format!(
        "{class}: {instances} instances in {calls} draw calls, first render {first:.1?}, \
         repeat {repeat:.1?}, {path}",
        instances = report.instances(),
        calls = report.draw_calls(),
        path = path.display()
    ));
    textured(&mut harness, &report, class)
}

fn textured(harness: &mut Harness<'_, Model>, report: &GpuReport, class: &str) -> TestResult {
    harness
        .state_mut()
        .entities_mut()
        .loadout_mut()
        .ok_or("no loadout")?
        .assembly_mut()
        .set_materials(ModelMaterials::Textures);
    let deadline = Instant::now() + BUILD_LIMIT;
    while view(harness.state()).is_some_and(AssemblyView::is_building) {
        if Instant::now() > deadline {
            return Err("the textured view was not built in time".into());
        }
        harness.step();
        std::thread::sleep(Duration::from_millis(20));
    }
    harness.run_steps(3);
    let started = Instant::now();
    let picture = harness.render()?;
    let first = started.elapsed();
    let images = view(harness.state())
        .and_then(AssemblyView::scene)
        .map_or(0, |scene| scene.texture_count());
    save_png(
        picture.as_raw(),
        (picture.width(), picture.height()),
        &format!("real-{class}-textures.png"),
    )?;
    harness
        .state_mut()
        .entities_mut()
        .loadout_mut()
        .and_then(|session| {
            session
                .assembly_mut()
                .camera_mut()
                .map(|camera| camera.zoom(6.0))
        })
        .ok_or("no camera")?;
    harness.run_steps(3);
    let started = Instant::now();
    let close = harness.render()?;
    let closer = started.elapsed();
    save_png(
        close.as_raw(),
        (close.width(), close.height()),
        &format!("real-{class}-textures-close.png"),
    )?;
    note(&format!(
        "{class}: {images} textures, {resident} on the device; first textured render {first:.1?}, \
         close-up render {closer:.1?}",
        resident = report.textures()
    ));
    assert!(images > 0, "the materials of a ship have textures");
    assert_eq!(report.textures(), images, "each uploaded once");
    Ok(())
}
