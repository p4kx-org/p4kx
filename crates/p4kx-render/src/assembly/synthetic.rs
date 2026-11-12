// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! A small ship of boxes for the tests that draw: the nose points along +X, Z is up.

use super::{AssemblyPieceSource, AssemblyScene, Group, Item, Left, PartDraw, PartInfo};
use crate::scene::{Bounds, Rows, Vertex, next_serial};

const FACES: [([f32; 3], [f32; 3], [f32; 3]); 6] = [
    ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]),
    ([-1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, 1.0, 0.0]),
    ([0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0]),
    ([0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
    ([0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
    ([0.0, 0.0, -1.0], [0.0, 1.0, 0.0], [1.0, 0.0, 0.0]),
];

struct Box {
    size: [f32; 3],
    at: [f32; 3],
    turn: f32,
}

const BOXES: [Box; 7] = [
    Box {
        size: [7.0, 1.6, 1.2],
        at: [0.0, 0.0, 0.0],
        turn: 0.0,
    },
    Box {
        size: [2.2, 1.0, 0.7],
        at: [1.6, 0.0, 0.85],
        turn: 0.0,
    },
    Box {
        size: [3.4, 4.2, 0.18],
        at: [-0.6, 2.9, -0.1],
        turn: -0.45,
    },
    Box {
        size: [3.4, 4.2, 0.18],
        at: [-0.6, -2.9, -0.1],
        turn: 0.45,
    },
    Box {
        size: [1.6, 0.18, 1.7],
        at: [-3.0, 0.0, 1.3],
        turn: 0.0,
    },
    Box {
        size: [1.4, 0.7, 0.7],
        at: [-3.6, 1.0, 0.0],
        turn: 0.0,
    },
    Box {
        size: [1.4, 0.7, 0.7],
        at: [-3.6, -1.0, 0.0],
        turn: 0.0,
    },
];

fn cube() -> (Vec<Vertex>, Vec<u32>) {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    for (normal, right, up) in FACES {
        let base = u32::try_from(vertices.len()).unwrap_or_default();
        for (u, v) in [(-1.0_f32, -1.0_f32), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
            let at: [f32; 3] = std::array::from_fn(|axis| {
                0.5 * v.mul_add(up[axis], u.mul_add(right[axis], normal[axis]))
            });
            vertices.push([
                at[0],
                at[1],
                at[2],
                normal[0],
                normal[1],
                normal[2],
                0.5_f32.mul_add(u, 0.5),
                0.5_f32.mul_add(v, 0.5),
            ]);
        }
        indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    (vertices, indices)
}

const RINGS: u32 = 12;
const SEGMENTS: u32 = 20;

fn sphere(base: u32) -> (Vec<Vertex>, Vec<u32>) {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    for ring in 0..=RINGS {
        let polar = std::f32::consts::PI * f32::from(u8::try_from(ring).unwrap_or_default())
            / f32::from(u8::try_from(RINGS).unwrap_or(1));
        for segment in 0..=SEGMENTS {
            let azimuth = std::f32::consts::TAU
                * f32::from(u8::try_from(segment).unwrap_or_default())
                / f32::from(u8::try_from(SEGMENTS).unwrap_or(1));
            let direction = [
                polar.sin() * azimuth.cos(),
                polar.sin() * azimuth.sin(),
                polar.cos(),
            ];
            vertices.push([
                0.5 * direction[0],
                0.5 * direction[1],
                0.5 * direction[2],
                direction[0],
                direction[1],
                direction[2],
                0.0,
                0.0,
            ]);
        }
    }
    for ring in 0..RINGS {
        for segment in 0..SEGMENTS {
            let first = base + ring * (SEGMENTS + 1) + segment;
            let below = first + SEGMENTS + 1;
            indices.extend([first, below, first + 1, first + 1, below, below + 1]);
        }
    }
    (vertices, indices)
}

fn rows(part: &Box) -> Rows {
    let (sin, cos) = part.turn.sin_cos();
    let [sx, sy, sz] = part.size;
    let [x, y, z] = part.at;
    [
        [cos * sx, -sin * sy, 0.0, x],
        [sin * sx, cos * sy, 0.0, y],
        [0.0, 0.0, sz, z],
    ]
}

fn groups(first_sphere: u32, sphere_indices: u32) -> Vec<Group> {
    [(0, 36), (first_sphere, sphere_indices)]
        .map(|(first_index, index_count)| Group {
            first_index,
            index_count,
            texture: None,
            translucent: false,
        })
        .to_vec()
}

/// Returns the ship: an ellipsoid hull of a smooth sphere mesh, and a canopy, two wings, a fin and
/// two engines of one cube mesh.
pub(crate) fn ship() -> AssemblyScene {
    p4kx_testkit::watchdog::start();
    let (mut vertices, mut indices) = cube();
    let first_sphere = u32::try_from(indices.len()).unwrap_or_default();
    let (round, round_indices) = sphere(u32::try_from(vertices.len()).unwrap_or_default());
    let sphere_indices = u32::try_from(round_indices.len()).unwrap_or_default();
    vertices.extend(round);
    indices.extend(round_indices);
    let mut bounds = Bounds::EMPTY;
    let mut draws = Vec::new();
    let mut parts = Vec::new();
    let mut items = Vec::new();
    for (at, part) in BOXES.iter().enumerate() {
        let world = rows(part);
        let radius = 0.5 * part.size.iter().map(|side| side * side).sum::<f32>().sqrt();
        let centre = part.at;
        bounds.include(centre.map(|value| value - radius));
        bounds.include(centre.map(|value| value + radius));
        let mut placed = Bounds::EMPTY;
        placed.include(centre.map(|value| value - radius));
        placed.include(centre.map(|value| value + radius));
        let number = u32::try_from(at).unwrap_or_default();
        items.push(Item {
            library_slot: None,
            group: u32::from(at == 0),
            colour: [0.55, 0.60, 0.65, 1.0],
        });
        draws.push(PartDraw {
            part: at,
            mesh: usize::from(at == 0),
            bounds: placed,
            world,
            centre,
            radius,
            items: number..number + 1,
        });
        parts.push(PartInfo {
            source: AssemblyPieceSource::Part(at),
            name: format!("port{at}/part"),
            path: format!("port{at}"),
            parent: at.checked_sub(1).map(|_| 0),
            triangles: 12,
            interior: false,
        });
    }
    AssemblyScene {
        vertices,
        indices,
        groups: groups(first_sphere, sphere_indices),
        items,
        draws,
        parts,
        textures: Vec::new(),
        meshes: 2,
        bounds,
        left: Left::default(),
        serial: next_serial(),
    }
}

/// Returns the ship with every draw repeated `copies` times in the same place, so that many
/// layers lie over each other.
pub(crate) fn stacked(copies: usize) -> AssemblyScene {
    let mut scene = ship();
    let draws = scene.draws.clone();
    let items = scene.items.clone();
    let step = u32::try_from(items.len()).unwrap_or_default();
    for copy in 1..u32::try_from(copies).unwrap_or(1) {
        scene.items.extend(items.iter().copied());
        scene.draws.extend(draws.iter().cloned().map(|mut draw| {
            draw.items = draw.items.start + copy * step..draw.items.end + copy * step;
            draw
        }));
    }
    scene
}
