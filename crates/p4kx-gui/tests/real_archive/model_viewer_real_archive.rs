// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The 3D viewer over models of a real Archive: they are read, flattened and drawn.

use std::error::Error;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use eframe::egui_wgpu::RendererOptions;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use egui_kittest::wgpu::{WgpuTestRenderer, create_render_state};
use p4kx_gui::{
    Browser, Language, MAX_SCENE_TRIANGLES, MAX_SCENE_VERTICES, Model, ModelPreview, ModelStatus,
    Preview, PreviewTab, ViewerSupport, install_viewer, is_model_path, ui,
};
use p4kx_testkit::real_archive_path;
use p4kx_testkit::wgpu_setup::software_wgpu_setup;

type TestResult = Result<(), Box<dyn Error>>;

#[expect(
    clippy::print_stderr,
    reason = "tells the test run why a test checked nothing"
)]
fn no_adapter() {
    eprintln!("no graphics adapter: the test checks nothing");
}

const SAMPLE: usize = 24;
const LOAD_LIMIT: Duration = Duration::from_secs(60);

fn opened(path: PathBuf) -> Result<Model, Box<dyn Error>> {
    let mut model = Model::new(Language::BritishEnglish);
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

fn model_entries(browser: &Browser) -> Vec<u32> {
    let index = browser.index();
    let all: Vec<u32> = (0..u32::try_from(index.len()).unwrap_or(u32::MAX))
        .filter(|&id| {
            index.entry(id).is_some_and(|entry| {
                let path = entry.path().to_ascii_lowercase();
                is_model_path(&path)
                    && path.contains("objects")
                    && entry.uncompressed_size() < 8 * 1024 * 1024
            })
        })
        .collect();
    let stride = (all.len() / SAMPLE).max(1);
    all.into_iter().step_by(stride).take(SAMPLE).collect()
}

fn load(browser: &mut Browser, id: u32) -> Result<(), Box<dyn Error>> {
    browser.select_entry(id, || {});
    let deadline = Instant::now() + LOAD_LIMIT;
    while browser
        .preview()
        .and_then(Preview::model)
        .is_some_and(ModelPreview::is_loading)
    {
        if Instant::now() > deadline {
            return Err("a model did not load in time".into());
        }
        std::thread::sleep(Duration::from_millis(5));
        browser.poll();
    }
    Ok(())
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn real_models_are_flattened_into_finite_scenes_within_the_caps() -> TestResult {
    let mut model = opened(real_archive_path()?)?;
    let browser = model.browser_mut().ok_or("not opened")?;
    let entries = model_entries(browser);
    assert!(entries.len() >= SAMPLE / 2, "models were found");

    let (mut ready, mut triangles) = (0, 0);
    for id in entries {
        load(browser, id)?;
        let preview = browser
            .preview()
            .and_then(Preview::model)
            .ok_or("no model preview")?;
        if let ModelStatus::Ready(scene) = preview.status() {
            let bounds = scene.bounds();
            assert!(
                bounds
                    .min
                    .iter()
                    .chain(&bounds.max)
                    .all(|value| value.is_finite()),
                "finite bounds"
            );
            assert!(scene.triangle_count() > 0, "triangles");
            assert!(
                scene
                    .indices()
                    .iter()
                    .all(|&index| (index as usize) < scene.vertices().len()),
                "every index is inside the vertex list"
            );
            assert!(preview.camera().is_some(), "framed");
            ready += 1;
            triangles += scene.triangle_count();
        }
    }

    assert!(
        ready >= SAMPLE / 2,
        "most sampled models are shown: {ready}"
    );
    assert!(triangles > 0, "triangles in total");
    Ok(())
}

fn entry_named(browser: &Browser, file: &str) -> Option<u32> {
    let index = browser.index();
    (0..u32::try_from(index.len()).ok()?).find(|&id| {
        index
            .entry(id)
            .is_some_and(|entry| entry.path().to_ascii_lowercase().ends_with(file))
    })
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn a_large_ship_is_flattened_within_the_caps_and_has_finite_bounds() -> TestResult {
    let mut model = opened(real_archive_path()?)?;
    let browser = model.browser_mut().ok_or("not opened")?;
    let id =
        entry_named(browser, "\\rsi_polaris.cga").ok_or("the Polaris is not in the Archive")?;

    load(browser, id)?;

    let preview = browser
        .preview()
        .and_then(Preview::model)
        .ok_or("no model preview")?;
    let ModelStatus::Ready(scene) = preview.status() else {
        return Err("the Polaris was not flattened into a scene".into());
    };
    assert!(scene.triangle_count() > 10_000, "a ship is large");
    assert!(
        scene.vertices().len() <= MAX_SCENE_VERTICES,
        "vertices within the cap"
    );
    assert!(
        scene.triangle_count() <= MAX_SCENE_TRIANGLES,
        "triangles within the cap"
    );
    let bounds = scene.bounds();
    assert!(
        bounds
            .min
            .iter()
            .chain(&bounds.max)
            .all(|value| value.is_finite()),
        "finite bounds"
    );
    assert!(
        scene
            .indices()
            .iter()
            .all(|&index| (index as usize) < scene.vertices().len()),
        "every index is inside the vertex list"
    );
    Ok(())
}

fn largest_model(browser: &mut Browser) -> Result<u32, Box<dyn Error>> {
    let mut best: Option<(u32, usize)> = None;
    for id in model_entries(browser) {
        load(browser, id)?;
        let count = browser
            .preview()
            .and_then(Preview::model)
            .and_then(ModelPreview::scene)
            .map(|scene| scene.triangle_count());
        if let Some(count) = count
            && best.is_none_or(|(_, top)| count > top)
        {
            best = Some((id, count));
        }
    }
    best.map(|(id, _)| id)
        .ok_or_else(|| "no model could be shown".into())
}

fn save_png(image: &[u8], (width, height): (u32, u32)) -> TestResult {
    let Some(folder) = std::env::var_os("P4KX_RENDER_DIR").map(PathBuf::from) else {
        return Ok(());
    };
    std::fs::create_dir_all(&folder)?;
    let file = std::io::BufWriter::new(std::fs::File::create(folder.join("real-model.png"))?);
    let mut encoder = png::Encoder::new(file, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(image)?;
    Ok(())
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn a_real_model_is_drawn_in_the_window() -> TestResult {
    let Ok(state) = std::panic::catch_unwind(|| {
        create_render_state(software_wgpu_setup(), RendererOptions::PREDICTABLE)
    }) else {
        no_adapter();
        return Ok(());
    };
    let report = install_viewer(&state)?;
    let mut model = opened(real_archive_path()?)?;
    model.set_viewer_support(ViewerSupport::Ready(Arc::clone(&report)));
    let id = largest_model(model.browser_mut().ok_or("not opened")?)?;
    model.set_preview_tab(PreviewTab::Model);
    let mut harness = Harness::builder()
        .with_context_setup(p4kx_gui::theme::apply)
        .with_size([1600.0, 900.0])
        .renderer(WgpuTestRenderer::from_render_state(state))
        .build_ui_state(
            |ui, model: &mut Model| {
                ui::show(ui, model);
            },
            model,
        );
    if let Some(browser) = harness.state_mut().browser_mut() {
        load(browser, id)?;
    }
    harness.run_steps(5);

    let image = harness.render()?;

    assert_eq!(report.uploads(), 1, "uploaded once");
    assert!(
        harness
            .get_by_label("Reset view")
            .accesskit_node()
            .is_disabled(),
        "the view starts framed"
    );
    assert!(!report.is_lost(), "the device is fine");
    assert!(report.resident().is_some(), "the model is on the device");
    save_png(image.as_raw(), (image.width(), image.height()))
}

fn animated_parts(browser: &mut Browser) -> Result<Vec<(u32, String)>, Box<dyn Error>> {
    let index = browser.index();
    let wanted: Vec<(u32, String)> = (0..u32::try_from(index.len()).unwrap_or(u32::MAX))
        .filter_map(|id| {
            let path = index.entry(id)?.path();
            let lower = path.to_ascii_lowercase();
            (is_model_path(&lower)
                && lower.contains("objects")
                && ["landing", "gear", "door", "turret"]
                    .iter()
                    .any(|word| lower.contains(word)))
            .then(|| (id, path.to_owned()))
        })
        .collect();
    let mut found = Vec::new();
    for (id, path) in wanted.into_iter().take(150) {
        load(browser, id)?;
        let deadline = Instant::now() + LOAD_LIMIT;
        while browser
            .preview()
            .and_then(Preview::model)
            .is_some_and(ModelPreview::is_reading_animations)
            && Instant::now() < deadline
        {
            std::thread::sleep(Duration::from_millis(5));
            browser.poll();
        }
        let animated = browser
            .preview()
            .and_then(Preview::model)
            .is_some_and(|preview| {
                preview.scene().is_some_and(|scene| {
                    let bounds = scene.bounds();
                    let size = bounds
                        .max
                        .iter()
                        .zip(&bounds.min)
                        .map(|(max, min)| max - min)
                        .fold(0.0_f32, f32::max);
                    scene.skin().is_some() && scene.triangle_count() >= 100 && size >= 1.0
                }) && preview
                    .playback()
                    .is_some_and(|playback| !playback.animations().is_empty())
            });
        if animated {
            found.push((id, path));
            if found.len() == 2 {
                break;
            }
        }
    }
    Ok(found)
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn real_doors_and_gear_are_drawn_at_rest_and_in_motion() -> TestResult {
    let Ok(state) = std::panic::catch_unwind(|| {
        create_render_state(software_wgpu_setup(), RendererOptions::PREDICTABLE)
    }) else {
        no_adapter();
        return Ok(());
    };
    let report = install_viewer(&state)?;
    let mut model = opened(real_archive_path()?)?;
    model.set_viewer_support(ViewerSupport::Ready(Arc::clone(&report)));
    let parts = animated_parts(model.browser_mut().ok_or("not opened")?)?;
    assert!(!parts.is_empty(), "a door or gear with animations exists");
    model.set_preview_tab(PreviewTab::Model);
    let mut harness = Harness::builder()
        .with_context_setup(p4kx_gui::theme::apply)
        .with_size([1600.0, 900.0])
        .renderer(WgpuTestRenderer::from_render_state(state))
        .build_ui_state(
            |ui, model: &mut Model| {
                ui::show(ui, model);
            },
            model,
        );
    for (at, (id, path)) in parts.iter().enumerate() {
        if let Some(browser) = harness.state_mut().browser_mut() {
            load(browser, *id)?;
        }
        harness.run_steps(5);
        let rest = harness.render()?;
        save_named(
            rest.as_raw(),
            (rest.width(), rest.height()),
            &format!("real-part-{at}-rest.png"),
        )?;
        let playback = harness
            .state_mut()
            .viewer_playback_mut()
            .ok_or("no playback")?;
        playback.select(Some(0));
        playback.apply(p4kx_gui::Control::Toggle);
        playback.apply(p4kx_gui::Control::Seek(0.5));
        harness.run_steps(5);
        let moved = harness.render()?;
        save_named(
            moved.as_raw(),
            (moved.width(), moved.height()),
            &format!("real-part-{at}-moved.png"),
        )?;
        assert_ne!(rest.as_raw(), moved.as_raw(), "{path} moves");
    }
    Ok(())
}

fn save_named(image: &[u8], (width, height): (u32, u32), name: &str) -> TestResult {
    let folder = std::env::var_os("P4KX_RENDER_DIR")
        .map_or_else(|| std::env::temp_dir().join("p4kx-render"), PathBuf::from);
    std::fs::create_dir_all(&folder)?;
    let file = std::io::BufWriter::new(std::fs::File::create(folder.join(name))?);
    let mut encoder = png::Encoder::new(file, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(image)?;
    Ok(())
}
