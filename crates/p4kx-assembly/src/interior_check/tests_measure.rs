// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Tests of the world box of a model and of the measure of one container.

use std::error::Error;
use std::sync::Arc;

use p4kx_chunks::ChunkFile;
use p4kx_model::{CompanionError, CompanionPath, MaterialStatus, Model, Transform};
use p4kx_testkit::{TestModel, TestNode};

use super::fit::ContainerFit;
use super::tests::hardpoint;
use super::*;
use crate::ObjectContainerRef;
use crate::{ContainerSource, EntityContainer, Interior, InteriorPart, InteriorPartKind};

type TestResult = Result<(), Box<dyn Error>>;

const IDENTITY: [[f32; 4]; 3] = [
    [1.0, 0.0, 0.0, 0.0],
    [0.0, 1.0, 0.0, 0.0],
    [0.0, 0.0, 1.0, 0.0],
];

fn moved(x: f32, y: f32, z: f32) -> Transform {
    Transform {
        rows: [[1.0, 0.0, 0.0, x], [0.0, 1.0, 0.0, y], [0.0, 0.0, 1.0, z]],
    }
}

/// A cube of 4 m edge whose mesh node sits at the model's origin: x and y from -2 to 2, z from 0
/// to 4.
fn cube() -> Result<Model, Box<dyn Error>> {
    let node = |name: &str, parent, kind| TestNode {
        name: name.to_owned(),
        parent,
        local: IDENTITY,
        kind,
    };
    let cube =
        TestModel::cube().with_nodes(vec![node("cube", None, 2), node("cube_mesh", Some(0), 0)]);
    let cgfm = cube.cgfm();
    let mut companions = move |path: CompanionPath<'_>| -> Result<Option<Vec<u8>>, CompanionError> {
        Ok(match path {
            CompanionPath::Sibling("cube.cgfm") => Some(cgfm.clone()),
            _ => None,
        })
    };
    Ok(Model::read(
        "cube.cgf",
        &ChunkFile::parse(cube.cgf())?,
        &mut companions,
    )?)
}

fn box_of(model: &Model, placement: &Transform) -> Option<WorldBox> {
    let mut bounds = None;
    WorldBox::grow_with_model(&mut bounds, model, placement);
    bounds
}

#[test]
fn a_model_grows_the_box_by_its_vertices_at_the_placement() -> TestResult {
    let bounds = box_of(&cube()?, &moved(10.0, 0.0, 1.0)).ok_or("no box")?;

    assert_eq!(bounds.min(), [8.0, -2.0, 1.0]);
    assert_eq!(bounds.max(), [12.0, 2.0, 5.0]);
    Ok(())
}

#[test]
fn a_model_without_levels_or_with_a_lost_node_adds_nothing() -> TestResult {
    let mut empty = cube()?;
    empty.lods.clear();
    let mut lost = cube()?;
    lost.lods[0].meshes[0].node = 99;

    assert_eq!(box_of(&empty, &Transform::IDENTITY), None);
    assert_eq!(box_of(&lost, &Transform::IDENTITY), None);
    Ok(())
}

#[test]
fn a_parent_loop_in_the_nodes_adds_nothing_and_ends() -> TestResult {
    let mut looped = cube()?;
    looped.nodes[1].parent = Some(1);
    let mut cycle = cube()?;
    cycle.nodes[0].parent = Some(1);

    assert_eq!(box_of(&looped, &Transform::IDENTITY), None);
    assert_eq!(box_of(&cycle, &Transform::IDENTITY), None);
    Ok(())
}

#[test]
fn a_placement_that_overflows_leaves_the_box_untouched() -> TestResult {
    let mut bounds = WorldBox::point([1.0; 3]);
    WorldBox::grow_with_model(&mut bounds, &cube()?, &moved(f32::NAN, 0.0, 0.0));

    assert_eq!(bounds, WorldBox::point([1.0; 3]));
    Ok(())
}

fn part(
    kind: InteriorPartKind,
    path: Option<&str>,
    model: Option<Arc<Model>>,
    at: Transform,
) -> InteriorPart {
    InteriorPart {
        name: path.unwrap_or("root").to_owned(),
        kind,
        parent: Some(0),
        local: at,
        world: at,
        model_path: path.map(str::to_owned),
        model,
        materials: Arc::new(MaterialStatus::Unnamed),
        palette: None,
        light: None,
        light_placement: None,
    }
}

fn container_of(parts: Vec<InteriorPart>) -> EntityContainer {
    EntityContainer {
        part: 0,
        reference: ObjectContainerRef {
            file: "room.socpak".to_owned(),
            bone: String::new(),
            position: [0.0; 3],
            rotation: [0.0; 3],
            source: ContainerSource::Vehicle,
            coordinate: None,
        },
        placement: moved(0.0, 0.0, 5.0),
        interior: Interior {
            parts,
            issues: Vec::new(),
            lod: 0,
            containers: 1,
            default_material: None,
        },
    }
}

#[test]
fn a_container_is_measured_at_its_owner_and_placement() -> TestResult {
    let model = Arc::new(cube()?);
    let container = container_of(vec![
        part(InteriorPartKind::Container, None, None, Transform::IDENTITY),
        part(
            InteriorPartKind::Model,
            Some("room.cgf"),
            Some(model),
            moved(1.0, 0.0, 0.0),
        ),
    ]);
    let inside = hardpoint(HardpointKind::Seat, [0.0, 10.0, 6.0]);
    let outside = hardpoint(HardpointKind::Seat, [0.0, 10.0, 20.0]);

    let fit = ContainerFit::measure(&moved(0.0, 10.0, 0.0), &container, &[inside, outside]);

    let room = fit.room().ok_or("no room")?;
    assert_eq!(room.min(), [-1.0, 8.0, 5.0]);
    assert_eq!(room.max(), [3.0, 12.0, 9.0]);
    assert_eq!((fit.inside(), fit.outside()), (&[0][..], &[1][..]));
    assert_eq!(fit.file(), "room.socpak");
    Ok(())
}

#[test]
fn props_do_not_bound_the_room_and_animated_parts_are_measured_from_it() -> TestResult {
    let model = Arc::new(cube()?);
    let container = container_of(vec![
        part(InteriorPartKind::Container, None, None, Transform::IDENTITY),
        part(
            InteriorPartKind::Model,
            Some("room.cgf"),
            Some(Arc::clone(&model)),
            Transform::IDENTITY,
        ),
        part(
            InteriorPartKind::Prop,
            Some("far_cabinet.cgf"),
            Some(Arc::clone(&model)),
            moved(100.0, 0.0, 0.0),
        ),
        part(
            InteriorPartKind::Prop,
            Some("DOOR.CGA"),
            Some(Arc::clone(&model)),
            moved(0.0, 7.0, 0.0),
        ),
        part(
            InteriorPartKind::Model,
            Some("ramp.cga"),
            None,
            moved(0.0, 0.0, 1.0),
        ),
    ]);

    let fit = ContainerFit::measure(&Transform::IDENTITY, &container, &[]);

    let room = fit.room().ok_or("no room")?;
    assert_eq!(room.max(), [2.0, 2.0, 9.0]);
    let distances: Vec<Option<f32>> = fit.animated().iter().map(AnimatedPart::distance).collect();
    assert_eq!(distances, [Some(5.0), Some(0.0)]);
    assert_eq!(fit.unreadable_models(), 0);
    Ok(())
}

#[test]
fn a_hardpoint_between_two_room_models_is_in_the_union_box_but_in_no_room() -> TestResult {
    let model = Arc::new(cube()?);
    let container = container_of(vec![
        part(InteriorPartKind::Container, None, None, Transform::IDENTITY),
        part(
            InteriorPartKind::Model,
            Some("cockpit.cgf"),
            Some(Arc::clone(&model)),
            Transform::IDENTITY,
        ),
        part(
            InteriorPartKind::Model,
            Some("body.cgf"),
            Some(model),
            moved(0.0, 10.0, 0.0),
        ),
    ]);
    let gap = hardpoint(HardpointKind::Seat, [0.0, 5.0, 7.0]);

    let fit = ContainerFit::measure(&Transform::IDENTITY, &container, &[gap]);

    assert!(fit.room().ok_or("no room")?.contains([0.0, 5.0, 7.0]));
    assert_eq!((fit.inside(), fit.outside()), (&[][..], &[0][..]));
    assert_eq!(fit.rooms().len(), 2);
    assert!(!fit.holds([0.0, 5.0, 7.0], 0.5));
    assert!(fit.holds([0.0, 10.0, 7.0], 0.5));
    assert!(fit.holds([0.0, 0.0, 7.0], 0.5));
    Ok(())
}

#[test]
fn a_model_much_smaller_than_the_largest_holds_hardpoints_but_is_no_room_for_the_hull_check()
-> TestResult {
    let model = Arc::new(cube()?);
    let container = container_of(vec![
        part(InteriorPartKind::Container, None, None, Transform::IDENTITY),
        part(
            InteriorPartKind::Model,
            Some("room.cgf"),
            Some(Arc::clone(&model)),
            Transform::IDENTITY,
        ),
        part(
            InteriorPartKind::Model,
            Some("trim.cgf"),
            Some(model),
            Transform {
                rows: [
                    [0.1, 0.0, 0.0, 0.0],
                    [0.0, 0.1, 0.0, 30.0],
                    [0.0, 0.0, 0.1, 0.0],
                ],
            },
        ),
    ]);

    let fit = ContainerFit::measure(&Transform::IDENTITY, &container, &[]);

    let hull = WorldBox::around([[-3.0, -3.0, -3.0], [3.0, 3.0, 7.0]]).ok_or("no hull")?;
    assert_eq!(fit.rooms().len(), 1);
    assert_eq!(fit.models().len(), 2);
    assert!(fit.holds([0.0, 30.0, 5.2], 0.5));
    assert_eq!(fit.overshoot(&hull), Some(2.0));
    Ok(())
}

#[test]
fn a_static_model_that_cannot_be_read_is_counted_not_boxed() {
    let container = container_of(vec![
        part(InteriorPartKind::Container, None, None, Transform::IDENTITY),
        part(
            InteriorPartKind::Model,
            Some("lost.cgf"),
            None,
            Transform::IDENTITY,
        ),
    ]);

    let fit = ContainerFit::measure(&Transform::IDENTITY, &container, &[]);

    assert_eq!(fit.room(), None);
    assert_eq!(fit.unreadable_models(), 1);
    assert!(!fit.holds([0.0; 3], f32::MAX));
}
