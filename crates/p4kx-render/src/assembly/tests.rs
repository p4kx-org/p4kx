// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;

use super::{
    AssemblyPieceSource, AssemblyScene, DrawList, Group, Item, Left, MIN_PART_PIXELS, PartDraw,
    PartInfo, hidden_mask,
};
use crate::scene::{Bounds, Rows};
use crate::{Camera, Matrix};

const ASPECT: f32 = 1.5;
const HEIGHT: f32 = 600.0;

fn at(x: f32, y: f32, z: f32) -> Rows {
    [[1.0, 0.0, 0.0, x], [0.0, 1.0, 0.0, y], [0.0, 0.0, 1.0, z]]
}

fn info(path: &str, parent: Option<usize>) -> PartInfo {
    PartInfo {
        source: AssemblyPieceSource::Part(0),
        name: path.to_owned(),
        path: path.to_owned(),
        parent,
        triangles: 12,
        interior: false,
    }
}

fn draw(part: usize, world: Rows, radius: f32, items: std::ops::Range<u32>) -> PartDraw {
    let centre = [world[0][3], world[1][3], world[2][3]];
    PartDraw {
        part,
        mesh: 0,
        bounds: Bounds {
            min: centre.map(|value| value - radius),
            max: centre.map(|value| value + radius),
        },
        centre,
        world,
        radius,
        items,
    }
}

fn with_interior_part(mut scene: AssemblyScene) -> AssemblyScene {
    let mut part = info("room", Some(0));
    part.interior = true;
    scene.parts.push(part);
    let mut room = draw(4, at(10.0, 0.0, 0.0), 1.0, 4..5);
    room.mesh = 1;
    scene.draws.push(room);
    scene.meshes = 2;
    scene.bounds.max[0] = 11.0;
    scene
}

#[test]
fn the_facts_leave_the_interior_out_while_it_is_hidden() {
    let scene = with_interior_part(scene());

    assert_eq!(scene.meshes_of(true), 2);
    assert_eq!(scene.meshes_of(false), 1, "the room's mesh is not counted");
    assert_eq!(scene.bounds_of(true), *scene.bounds());
    let hull = scene.bounds_of(false);
    assert!((hull.max[0] - 3.0).abs() < f32::EPSILON, "{hull:?}");
    assert!((hull.min[0] + 1.0).abs() < f32::EPSILON, "{hull:?}");
}

#[test]
fn a_scene_of_the_interior_alone_keeps_its_box_when_it_is_hidden() {
    let mut scene = with_interior_part(scene());
    for part in &mut scene.parts {
        part.interior = true;
    }

    assert_eq!(scene.bounds_of(false), *scene.bounds());
}

fn group(translucent: bool) -> Group {
    Group {
        first_index: 0,
        index_count: 36,
        texture: None,
        translucent,
    }
}

fn scene() -> AssemblyScene {
    let item = |group| Item {
        library_slot: Some(0),
        group,
        colour: [0.5, 0.5, 0.5, 1.0],
    };
    AssemblyScene {
        vertices: Vec::new(),
        indices: vec![0; 36],
        groups: vec![group(false), group(true)],
        items: vec![item(0), item(0), item(0), item(1), item(0)],
        draws: vec![
            draw(0, at(0.0, 0.0, 0.0), 1.0, 0..1),
            draw(1, at(2.0, 0.0, 0.0), 1.0, 1..2),
            draw(2, at(0.0, 2.0, 0.0), 1.0, 2..4),
            draw(3, at(0.0, 0.0, 2.0), 1.0, 4..5),
        ],
        parts: vec![
            info("", None),
            info("a", Some(0)),
            info("a/b", Some(1)),
            info("c", Some(0)),
        ],
        material_suppressions: None,
        textures: Vec::new(),
        meshes: 1,
        bounds: Bounds {
            min: [-1.0; 3],
            max: [3.0; 3],
        },
        left: Left::default(),
        serial: 1,
    }
}

fn view(scene: &AssemblyScene) -> Matrix {
    let mut camera = Camera::framing(scene.bounds());
    camera.fit(ASPECT);
    camera.uniforms(ASPECT).view_proj
}

#[test]
fn parts_sharing_a_mesh_range_become_one_run_of_instances() {
    let scene = scene();

    let list = DrawList::of(&scene, &[false; 4], &view(&scene), HEIGHT);

    assert_eq!(list.visible_parts(), 4, "every part is in view");
    assert_eq!(list.instances().len(), 5, "one instance per item");
    let runs: Vec<(u32, u32)> = list
        .draws()
        .iter()
        .map(|run| (run.group, run.count))
        .collect();
    assert_eq!(runs, vec![(0, 4), (1, 1)], "four share the opaque group");
    assert_eq!(list.draws().first().map(|run| run.first), Some(0), "start");
    assert_eq!(list.draws().get(1).map(|run| run.first), Some(4), "next");
}

#[test]
fn translucent_items_are_drawn_from_the_far_part_to_the_near_one() {
    let mut scene = scene();
    scene.groups.push(group(true));
    scene.draws.clear();
    let item = |group| Item {
        library_slot: Some(0),
        group,
        colour: [0.5; 4],
    };
    scene.items = vec![item(1), item(2), item(1), item(0)];
    for (part, (x, depth)) in [(0.1, 10.0), (0.2, 20.0), (0.3, 5.0)]
        .into_iter()
        .enumerate()
    {
        let first = u32::try_from(part).unwrap_or(0);
        scene
            .draws
            .push(draw(part, at(x, 0.0, depth), 1.0, first..first + 1));
    }
    scene.draws.push(draw(3, at(0.4, 0.0, 8.0), 1.0, 3..4));
    let perspective: Matrix = [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 0.01, 1.0],
        [0.0, 0.0, 0.0, 0.0],
    ];

    let list = DrawList::of(&scene, &[false; 4], &perspective, HEIGHT);

    let xs: Vec<f32> = list.instances().iter().map(|rows| rows[0][3]).collect();
    assert_eq!(xs, vec![0.4, 0.2, 0.1, 0.3], "opaque, then far to near");
    let runs: Vec<(u32, u32)> = list
        .draws()
        .iter()
        .map(|run| (run.group, run.count))
        .collect();
    assert_eq!(
        runs,
        vec![(0, 1), (2, 1), (1, 2)],
        "neighbours of a group merge"
    );
}

#[test]
fn a_hidden_branch_hides_its_parts_and_their_descendants() {
    let scene = scene();
    let hidden: HashSet<String> = ["a".to_owned()].into();

    let mask = hidden_mask(scene.parts(), &hidden, true);
    let list = DrawList::of(&scene, &mask, &view(&scene), HEIGHT);

    assert_eq!(mask, vec![false, true, true, false], "a and a/b");
    assert_eq!(list.visible_parts(), 2, "the root and c remain");
    assert_eq!(list.hidden_parts(), 2, "two were hidden");
    assert_eq!(list.instances().len(), 2, "their instances are gone");
}

#[test]
fn the_root_is_never_hidden_by_an_empty_path() {
    let scene = scene();
    let hidden: HashSet<String> = [String::new()].into();

    assert_eq!(
        hidden_mask(scene.parts(), &hidden, true),
        vec![false; 4],
        "an empty path names no port"
    );
}

#[test]
fn the_interior_is_hidden_with_its_descendants_unless_it_is_shown() {
    let mut scene = scene();
    scene.parts[1].interior = true;
    let none = HashSet::new();

    assert_eq!(
        hidden_mask(scene.parts(), &none, false),
        vec![false, true, true, false],
        "the interior part and what hangs below it"
    );
    assert_eq!(
        hidden_mask(scene.parts(), &none, true),
        vec![false; 4],
        "shown"
    );
}

#[test]
fn a_part_outside_the_view_is_culled() {
    let mut scene = scene();
    scene.draws.push(draw(3, at(1.0e6, 0.0, 0.0), 1.0, 4..5));
    let matrix = view(&scene);

    let list = DrawList::of(&scene, &[false; 4], &matrix, HEIGHT);

    assert_eq!(list.outside_parts(), 1, "the far part");
    assert_eq!(list.visible_parts(), 4, "the others");
}

#[test]
fn a_part_smaller_than_a_pixel_is_not_drawn() {
    let mut scene = scene();
    scene.draws.push(draw(3, at(1.0, 1.0, 1.0), 1.0e-4, 4..5));
    let matrix = view(&scene);

    let list = DrawList::of(&scene, &[false; 4], &matrix, HEIGHT);

    assert_eq!(list.small_parts(), 1, "below {MIN_PART_PIXELS} px");
    assert_eq!(list.instances().len(), 5, "only the visible items");
}

#[test]
fn the_work_of_a_frame_follows_the_parts_not_the_triangles() {
    let mut scene = scene();
    scene.indices = vec![0; 3 * 5_000_000];
    scene.draws.clear();
    scene.parts = std::iter::once(info("", None))
        .chain((0..4000).map(|_| info("p", Some(0))))
        .collect();
    for part in 0..4000_u16 {
        scene.draws.push(draw(
            usize::from(part) + 1,
            at(f32::from(part) * 0.001, 0.0, 0.0),
            1.0,
            0..1,
        ));
    }
    let matrix = view(&scene);

    let list = DrawList::of(&scene, &vec![false; 4001], &matrix, HEIGHT);

    assert_eq!(list.draws().len(), 1, "one draw call for 4000 instances");
    assert_eq!(list.instances().len(), 4000, "one instance each");
}

#[test]
fn observed_frame_records_the_accepted_pieces_and_bound_material_ranges() {
    let scene = scene();
    let view = view(&scene);
    let observed = DrawList::of_observed(&scene, &[false, true, false, true], &view, HEIGHT);
    assert_eq!(observed.parts(), [0, 2]);
    let (list, selected) = observed.into_parts();
    assert_eq!(list.visible_parts(), selected.len());
    let inputs: Vec<_> = scene
        .placed_inputs()
        .map(|piece| (piece.ordinal(), piece.material_slots().collect::<Vec<_>>()))
        .collect();
    assert_eq!(inputs.len(), 4);
    assert_eq!(inputs.first(), Some(&(0, vec![0])));
    assert_eq!(inputs.get(2), Some(&(2, vec![0, 0])));
}
