// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The state of the 3D view of the Entities tab: its Assembly, choices, hidden branches, changes.

use crate::entity_picking::{TestResult, opened, select, settle};
use crate::tagging::wait_until;

use egui_kittest::Harness;
use p4kx_gui::{AssemblyView, Camera, DrawList, Language, LoadoutSession, Model, VIEW_LOD};
use p4kx_settings::{AssemblyExportSettings, ModelMaterials};

pub(crate) fn view(model: &Model) -> Option<&AssemblyView> {
    model.entities().loadout().map(LoadoutSession::assembly)
}

pub(crate) fn view_mut<'a>(
    harness: &'a mut Harness<'_, Model>,
) -> Result<&'a mut AssemblyView, &'static str> {
    harness
        .state_mut()
        .entities_mut()
        .loadout_mut()
        .map(LoadoutSession::assembly_mut)
        .ok_or("no Loadout")
}

pub(crate) fn serial(model: &Model) -> Option<u64> {
    view(model)
        .and_then(AssemblyView::scene)
        .map(|scene| scene.serial())
}

pub(crate) fn built(harness: &mut Harness<'_, Model>, before: Option<u64>) -> TestResult {
    wait_until(harness, |model| {
        view(model).is_some_and(|view| !view.is_building())
            && serial(model).is_some()
            && serial(model) != before
    })
}

fn paths(model: &Model) -> Vec<String> {
    view(model)
        .and_then(AssemblyView::scene)
        .map_or_default(|scene| {
            scene
                .parts()
                .iter()
                .map(|part| part.path().to_owned())
                .collect()
        })
}

pub(crate) fn draw(
    harness: &mut Harness<'_, Model>,
) -> Result<std::sync::Arc<DrawList>, Box<dyn std::error::Error>> {
    let view = view_mut(harness)?;
    let mut camera: Camera = view.camera().cloned().ok_or("no camera")?;
    camera.fit(1.5);
    let matrix = camera.uniforms(1.5).view_proj;
    Ok(view.draw_list(&matrix, 800.0).ok_or("no scene")?)
}

#[test]
fn the_view_shows_the_assembly_the_export_writes() -> TestResult {
    let (_dir, mut harness) = opened(Language::BritishEnglish)?;
    select(&mut harness, "TEST_Ship")?;
    built(&mut harness, None)?;

    let model = harness.state();
    let view = view(model).ok_or("no view")?;
    let scene = view.scene().ok_or("no scene")?;
    assert_eq!(
        paths(model),
        vec!["", "hardpoint_weapon_nose", "hardpoint_relay"],
        "the root and a part per port, in tree order"
    );
    assert_eq!(scene.drawn_parts(), 3, "every part has a model");
    assert_eq!(
        scene.meshes(),
        2,
        "the two parts with the gun model share it"
    );
    assert!(scene.triangle_count() > 0, "triangles are stored");
    assert_eq!(view.lod(), VIEW_LOD, "the level the view opens with");
    assert!(view.camera().is_some(), "framed");
    assert_eq!(view.issues(), 0, "everything resolves");
    Ok(())
}

#[test]
fn parts_sharing_a_model_are_one_run_of_instances() -> TestResult {
    let (_dir, mut harness) = opened(Language::BritishEnglish)?;
    select(&mut harness, "TEST_Ship")?;
    built(&mut harness, None)?;

    let list = draw(&mut harness)?;

    assert_eq!(list.visible_parts(), 3, "all in view");
    let scene = view(harness.state())
        .and_then(AssemblyView::scene)
        .ok_or("no scene")?;
    assert!(
        list.draws().len() < scene.item_count(),
        "fewer draw calls ({}) than draw items ({})",
        list.draws().len(),
        scene.item_count()
    );
    assert!(
        list.draws().iter().any(|run| run.count == 2),
        "the two guns are one instanced draw"
    );
    Ok(())
}

#[test]
fn leaving_a_port_out_updates_the_view() -> TestResult {
    let (_dir, mut harness) = opened(Language::BritishEnglish)?;
    select(&mut harness, "TEST_Ship")?;
    built(&mut harness, None)?;
    let before = serial(harness.state());

    harness
        .state_mut()
        .entities_mut()
        .loadout_mut()
        .ok_or("no Loadout")?
        .set_excluded("hardpoint_weapon_nose", true)?;
    settle(&mut harness)?;
    built(&mut harness, before)?;

    assert_eq!(
        paths(harness.state()),
        vec!["", "hardpoint_relay"],
        "the left-out port has no part"
    );
    Ok(())
}

#[test]
fn quick_changes_end_with_the_last_one() -> TestResult {
    let (_dir, mut harness) = opened(Language::BritishEnglish)?;
    select(&mut harness, "TEST_Ship")?;
    built(&mut harness, None)?;
    let before = serial(harness.state());

    let session = harness
        .state_mut()
        .entities_mut()
        .loadout_mut()
        .ok_or("no Loadout")?;
    session.set_excluded("hardpoint_weapon_nose", true)?;
    session.set_excluded("hardpoint_relay", true)?;
    session.restore("hardpoint_weapon_nose");
    session.set_excluded("hardpoint_weapon_nose", true)?;
    session.restore("hardpoint_weapon_nose");
    settle(&mut harness)?;
    built(&mut harness, before)?;

    assert_eq!(
        paths(harness.state()),
        vec!["", "hardpoint_weapon_nose"],
        "only the last state of the changes is shown"
    );
    Ok(())
}

#[test]
fn a_replacement_below_a_left_out_port_is_refused() -> TestResult {
    let (_dir, mut harness) = opened(Language::BritishEnglish)?;
    select(&mut harness, "TEST_Ship")?;
    built(&mut harness, None)?;

    let session = harness
        .state_mut()
        .entities_mut()
        .loadout_mut()
        .ok_or("no Loadout")?;
    session.set_excluded("hardpoint_weapon_nose", true)?;
    let refused = session.set_replacement("hardpoint_weapon_nose/mount", "TEST_Gun_S1");

    assert!(
        matches!(refused, Err(p4kx_assembly::ChangeError::Conflict { .. })),
        "{refused:?}"
    );
    assert_eq!(session.change_counts(), (1, 0), "nothing was added");
    settle(&mut harness)?;
    assert!(
        harness
            .state()
            .entities()
            .loadout()
            .is_some_and(|session| session.failure().is_none()),
        "the Loadout still resolves"
    );
    Ok(())
}

#[test]
fn replacing_an_entity_updates_the_view() -> TestResult {
    let (_dir, mut harness) = opened(Language::BritishEnglish)?;
    select(&mut harness, "TEST_Ship")?;
    built(&mut harness, None)?;
    let before = serial(harness.state());

    harness
        .state_mut()
        .entities_mut()
        .loadout_mut()
        .ok_or("no Loadout")?
        .set_replacement("hardpoint_weapon_nose", "TEST_Gun_S1")?;
    settle(&mut harness)?;
    built(&mut harness, before)?;

    let model = harness.state();
    let scene = view(model)
        .and_then(AssemblyView::scene)
        .ok_or("no scene")?;
    assert!(
        scene
            .parts()
            .iter()
            .any(|part| part.name().contains("TEST_Gun_S1")),
        "the replacement is a part"
    );
    assert!(
        !scene
            .parts()
            .iter()
            .any(|part| part.name().contains("TEST_Gun_S2")),
        "the replaced Entity is gone"
    );
    Ok(())
}

#[test]
fn another_detail_or_material_mode_builds_the_view_again() -> TestResult {
    let (_dir, mut harness) = opened(Language::BritishEnglish)?;
    select(&mut harness, "TEST_Ship")?;
    built(&mut harness, None)?;
    let first = serial(harness.state());

    view_mut(&mut harness)?.set_lod(7);
    assert!(
        !view(harness.state()).is_some_and(AssemblyView::is_building),
        "ignored"
    );

    view_mut(&mut harness)?.set_lod(0);
    built(&mut harness, first)?;
    let second = serial(harness.state());
    assert_eq!(
        view(harness.state()).map(AssemblyView::lod),
        Some(0),
        "level"
    );

    view_mut(&mut harness)?.set_materials(ModelMaterials::Textures);
    built(&mut harness, second)?;
    assert_eq!(
        view(harness.state()).map(AssemblyView::materials),
        Some(ModelMaterials::Textures),
        "materials"
    );
    assert!(
        view(harness.state()).is_some_and(|view| view.failure().is_none()),
        "no failure"
    );
    Ok(())
}

#[test]
fn the_export_keeps_its_remembered_choices_until_the_view_is_changed() -> TestResult {
    let (_dir, mut harness) = opened(Language::BritishEnglish)?;
    harness
        .state_mut()
        .set_assembly_export_settings(AssemblyExportSettings {
            lod: 0,
            materials: ModelMaterials::Textures,
            ..AssemblyExportSettings::default()
        });
    select(&mut harness, "TEST_Ship")?;
    built(&mut harness, None)?;
    let view = view(harness.state()).ok_or("no view")?;
    assert_eq!(view.materials(), ModelMaterials::Textures, "starts there");
    assert_eq!(view.lod(), VIEW_LOD, "but no heavier than the default");

    harness.state_mut().begin_assembly_export();

    let setup = harness.state().assembly_export_setup().ok_or("no setup")?;
    assert_eq!(setup.lod(), 0, "the remembered level");
    assert_eq!(setup.materials(), ModelMaterials::Textures, "materials");
    Ok(())
}

#[test]
fn stopping_a_build_keeps_the_view_built_before() -> TestResult {
    let (_dir, mut harness) = opened(Language::BritishEnglish)?;
    select(&mut harness, "TEST_Ship")?;
    built(&mut harness, None)?;
    let first = serial(harness.state());

    let view = view_mut(&mut harness)?;
    view.set_lod(0);
    assert!(view.is_building(), "building");
    assert!(view.progress().is_some(), "progress is reported");
    view.cancel();

    assert!(!view.is_building(), "stopped");
    assert!(view.is_cancelled(), "by the user");
    harness.run_steps(3);
    assert_eq!(serial(harness.state()), first, "the old scene stays");
    Ok(())
}

#[test]
fn the_export_starts_with_the_detail_and_materials_of_the_view() -> TestResult {
    let (_dir, mut harness) = opened(Language::BritishEnglish)?;
    select(&mut harness, "TEST_Ship")?;
    built(&mut harness, None)?;
    view_mut(&mut harness)?.set_lod(2);
    view_mut(&mut harness)?.set_materials(ModelMaterials::Textures);

    harness.state_mut().begin_assembly_export();

    let setup = harness.state().assembly_export_setup().ok_or("no setup")?;
    assert_eq!(setup.lod(), 2, "level");
    assert_eq!(setup.materials(), ModelMaterials::Textures, "materials");
    Ok(())
}
