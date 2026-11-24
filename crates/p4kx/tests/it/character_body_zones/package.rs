// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;
use std::fs;
use std::io::{self, Write as _};
use std::path::Path;

use p4kx_testkit::{CheckedGlb, check_glb};
use serde_json::{Value, json};

use super::{GLB, TestResult, body_map, read_manifest};

pub(super) struct Package {
    glb: CheckedGlb,
    pub(super) manifest: Value,
}

impl Package {
    pub(super) fn read(directory: &Path) -> TestResult<Self> {
        let bytes = fs::read(directory.join(GLB))?;
        let manifest = read_manifest(directory)?;
        let mut measured = p4kx_extraction::OriginDigestWriter::new(io::sink());
        measured.write_all(&bytes)?;
        let digest = measured.finish()?;
        assert_eq!(
            body_map(&manifest)?
                .get("glb_sha256")
                .and_then(Value::as_str),
            Some(digest.sha256()),
            "map addresses bind to the exact emitted GLB bytes"
        );
        Ok(Self {
            glb: check_glb(&bytes)?,
            manifest,
        })
    }

    pub(super) fn body_primitives(&self) -> TestResult<BTreeMap<String, Vec<u32>>> {
        let body = body_map(&self.manifest)?
            .pointer("/body/primitives")
            .and_then(Value::as_array)
            .ok_or("verified body primitive list")?;
        let mut zones = BTreeMap::new();
        for primitive in body {
            let mesh = index(primitive, "mesh")?;
            let position = index(primitive, "primitive")?;
            let zone = primitive
                .get("zone")
                .and_then(Value::as_str)
                .ok_or("body zone name")?;
            let bits = self.primitive(mesh, position)?;
            if zones.insert(zone.to_owned(), bits).is_some() {
                return Err("the synthetic body has exactly one primitive per zone".into());
            }
        }
        Ok(zones)
    }

    pub(super) fn part_primitives(&self, name: &str) -> TestResult<Vec<Vec<u32>>> {
        let rig = self
            .manifest
            .pointer("/meshes/0/rig")
            .ok_or("Character rig")?;
        let parts = rig
            .get("parts")
            .and_then(Value::as_array)
            .ok_or("rig parts")?;
        let at = parts
            .iter()
            .position(|part| part.get("name").and_then(Value::as_str) == Some(name))
            .ok_or("nominated item in manifest")?;
        let worn = body_map(&self.manifest)?
            .get("parts")
            .and_then(Value::as_array)
            .ok_or("worn map")?;
        let mapped = worn
            .iter()
            .find(|row| row.get("part") == Some(&json!(at)))
            .ok_or("mapped item")?;
        let meshes = mapped
            .get("meshes")
            .and_then(Value::as_array)
            .ok_or("item mesh indices")?;
        let mut output = Vec::new();
        for mesh in meshes {
            let mesh = usize::try_from(mesh.as_u64().ok_or("item mesh index")?)?;
            let emitted = self
                .glb
                .document
                .meshes()
                .nth(mesh)
                .ok_or("emitted item mesh")?;
            for row in emitted.primitives() {
                output.push(self.primitive(mesh, row.index())?);
            }
        }
        if output.is_empty() {
            return Err("expected nominated item triangles".into());
        }
        Ok(output)
    }

    fn primitive(&self, mesh: usize, primitive: usize) -> TestResult<Vec<u32>> {
        let primitive = self
            .glb
            .document
            .meshes()
            .nth(mesh)
            .and_then(|mesh| mesh.primitives().nth(primitive))
            .ok_or("addressed GLB primitive")?;
        let indices = primitive.indices().ok_or("primitive index accessor")?;
        let positions = primitive
            .attributes()
            .find(|(semantic, _)| semantic.to_string() == "POSITION")
            .map(|(_, accessor)| accessor)
            .ok_or("primitive POSITION accessor")?;
        let floats = self.glb.floats(&positions);
        let vertices = floats.as_chunks::<3>().0;
        let result = self
            .glb
            .indices(&indices)
            .into_iter()
            .map(|at| {
                vertices
                    .get(at as usize)
                    .copied()
                    .ok_or("addressed triangle vertex")
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(result.into_iter().flatten().map(f32::to_bits).collect())
    }

    pub(super) fn bind_pose(&self) -> TestResult<(Vec<String>, Vec<u8>)> {
        let skin = self.glb.document.skins().next().ok_or("Character skin")?;
        let joints = skin
            .joints()
            .map(|joint| joint.name().unwrap_or_default().to_owned())
            .collect();
        let matrices = skin
            .inverse_bind_matrices()
            .ok_or("inverse bind accessor")?;
        Ok((joints, self.glb.bytes(&matrices)))
    }
}

fn index(value: &Value, key: &str) -> TestResult<usize> {
    Ok(usize::try_from(
        value
            .get(key)
            .and_then(Value::as_u64)
            .ok_or("GLB address")?,
    )?)
}
