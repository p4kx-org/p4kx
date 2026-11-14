// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::starmap_xml::{Document, Node, Root};
use p4kx_extraction::Guid;

use super::TestResult;

const IDENTITY: [[f64; 3]; 3] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

#[derive(Clone)]
pub(super) struct AuthoredChild {
    pub(super) index: u32,
    pub(super) position: [f64; 3],
    pub(super) rotation: [f64; 4],
    pub(super) xml_encrypted: Option<bool>,
    pub(super) position_present: bool,
    pub(super) rotation_present: bool,
}

pub(super) struct ReferenceRoute<'descriptor> {
    pub(super) target: &'descriptor str,
    pub(super) record: Option<Guid>,
    pub(super) nodes: Vec<AuthoredChild>,
}

impl ReferenceRoute<'_> {
    pub(super) fn has_pose(&self) -> bool {
        self.nodes
            .last()
            .is_some_and(|node| node.position_present && node.rotation_present)
    }
}

pub(super) fn reference_routes<'descriptor>(
    document: &'descriptor Document<'_>,
    encrypted: Option<bool>,
) -> TestResult<Vec<ReferenceRoute<'descriptor>>> {
    let mut routes = Vec::new();
    let mut path = Vec::new();
    read_direct_lists(document, document.root(), encrypted, &mut path, &mut routes)?;
    Ok(routes)
}

fn read_direct_lists<'descriptor>(
    document: &Document<'_>,
    parent: Root<'descriptor>,
    encrypted: Option<bool>,
    path: &mut Vec<AuthoredChild>,
    routes: &mut Vec<ReferenceRoute<'descriptor>>,
) -> TestResult {
    for section in parent.children() {
        if !section.name().eq_ignore_ascii_case("ChildObjectContainers") {
            continue;
        }
        for child in section.children() {
            if child.name().eq_ignore_ascii_case("Child") {
                read_child(document, child, encrypted, path, routes)?;
            }
        }
    }
    Ok(())
}

fn read_child<'descriptor>(
    document: &Document<'_>,
    child: Root<'descriptor>,
    encrypted: Option<bool>,
    path: &mut Vec<AuthoredChild>,
    routes: &mut Vec<ReferenceRoute<'descriptor>>,
) -> TestResult {
    let position = attr(child, "pos");
    let rotation = attr(child, "rot");
    let local = AuthoredChild {
        index: document
            .original_index(child)
            .ok_or("matching original Child coordinate")?,
        position: coordinates(position, [0.0; 3])?,
        rotation: coordinates(rotation, [1.0, 0.0, 0.0, 0.0])?,
        xml_encrypted: encrypted,
        position_present: position.is_some(),
        rotation_present: rotation.is_some(),
    };
    path.push(local);
    if matches!(attr(child, "external"), Some("1" | "true"))
        && let Some(target) = attr(child, "name")
    {
        let record = attr(child, "starMapRecord")
            .and_then(|text| text.trim().parse().ok())
            .filter(|id| id != &Guid::default());
        routes.push(ReferenceRoute {
            target,
            record,
            nodes: path.clone(),
        });
    }
    read_direct_lists(document, child, encrypted, path, routes)?;
    path.pop();
    Ok(())
}

fn attr<'descriptor>(element: Root<'descriptor>, field: &str) -> Option<&'descriptor str> {
    element
        .attributes()
        .find_map(|(name, value)| name.eq_ignore_ascii_case(field).then_some(value))
}

fn coordinates<const N: usize>(text: Option<&str>, default: [f64; N]) -> TestResult<[f64; N]> {
    let Some(text) = text else {
        return Ok(default);
    };
    let mut original = text.split(',');
    let mut coordinates: [f64; N] = [0.0; N];
    for coordinate in &mut coordinates {
        *coordinate = original
            .next()
            .ok_or("an original Child coordinate is missing")?
            .trim()
            .parse()?;
        if !coordinate.is_finite() {
            return Err("a selected original Child coordinate is nonfinite".into());
        }
    }
    if original.next().is_some() {
        return Err("unexpected extra original Child coordinate".into());
    }
    Ok(coordinates)
}

pub(super) struct World {
    pub(super) position: [f64; 3],
    pub(super) rotation: [[f64; 3]; 3],
}

pub(super) fn world<'route>(
    route: impl IntoIterator<Item = &'route AuthoredChild>,
) -> TestResult<World> {
    let mut position = [0.0; 3];
    let mut rotation = IDENTITY;
    for child in route {
        let [x, y, z] = rotation.map(|row: [f64; 3]| dot(row, child.position));
        let [px, py, pz] = position;
        position = [px + x, py + y, pz + z];
        let [[ax, ay, az], [bx, by, bz], [cx, cy, cz]] = rotation_matrix(child.rotation)?;
        let columns = [[ax, bx, cx], [ay, by, cy], [az, bz, cz]];
        rotation = rotation.map(|row| columns.map(|column| dot(row, column)));
    }
    Ok(World { position, rotation })
}

fn dot([ax, ay, az]: [f64; 3], [bx, by, bz]: [f64; 3]) -> f64 {
    az.mul_add(bz, ay.mul_add(by, ax * bx))
}

pub(super) fn rotation_matrix([w, x, y, z]: [f64; 4]) -> TestResult<[[f64; 3]; 3]> {
    let norm = z.mul_add(z, y.mul_add(y, x.mul_add(x, w * w)));
    if !norm.is_finite() || norm <= f64::EPSILON {
        return Err("a selected original quaternion cannot be normalized".into());
    }
    let scaled = 2.0 / norm;
    Ok([
        [
            scaled.mul_add(-z.mul_add(z, y * y), 1.0),
            scaled * w.mul_add(-z, x * y),
            scaled * w.mul_add(y, x * z),
        ],
        [
            scaled * w.mul_add(z, x * y),
            scaled.mul_add(-z.mul_add(z, x * x), 1.0),
            scaled * w.mul_add(-x, y * z),
        ],
        [
            scaled * w.mul_add(-y, x * z),
            scaled * w.mul_add(x, y * z),
            scaled.mul_add(-y.mul_add(y, x * x), 1.0),
        ],
    ])
}
