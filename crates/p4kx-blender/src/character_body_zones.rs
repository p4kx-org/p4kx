// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;

use p4kx_assembly::{Armour, ArmourPart, BodyVariant, PartOrigin};
use p4kx_gltf::MeshVertexMap;

use crate::character_body_zone_origins::{BodyZoneOrigins, OriginalZoneWord};
use crate::manifest_body_zones::{
    BodyZoneData, BodyZoneGeometry, BodyZonePrimitive, ClothingBodyCoverage, PartBodyZones,
};

pub(super) struct ObserveBodyZones {
    variant: BodyVariant,
    body: BodyZoneGeometry,
    meshes_by_part: Vec<Vec<usize>>,
    body_words: Vec<OriginalZoneWord>,
}

impl ObserveBodyZones {
    pub(super) fn new(armour: &Armour, lod: u32) -> Self {
        let variant = armour.variant();
        let body = match armour.body_zone_layout(lod) {
            Ok(omitted) => BodyZoneGeometry::Verified {
                primitives: Vec::new(),
                omitted: omitted.to_vec(),
            },
            Err(reason) => BodyZoneGeometry::Unavailable {
                reason: reason.to_string(),
            },
        };
        Self {
            variant,
            body,
            meshes_by_part: vec![Vec::new(); armour.parts().len()],
            body_words: Vec::new(),
        }
    }

    pub(super) fn observe(
        &mut self,
        emitted: &MeshVertexMap<'_>,
        part: &ArmourPart,
    ) -> io::Result<()> {
        let meshes = self
            .meshes_by_part
            .get_mut(emitted.part)
            .ok_or_else(|| io::Error::other("emitted Character mesh lost its rig part"))?;
        meshes.push(emitted.mesh);
        if part.origin() == PartOrigin::Body {
            self.record_body(emitted, part);
        }
        Ok(())
    }

    fn record_body(&mut self, emitted: &MeshVertexMap<'_>, part: &ArmourPart) {
        let BodyZoneGeometry::Verified { primitives, .. } = &mut self.body else {
            return;
        };
        let names = self.variant.zone_names();
        let source = emitted.origin.and_then(|geometry| {
            part.model_archive_sources()
                .iter()
                .find(|source| source.file() == geometry.file)
                .and_then(p4kx_extraction::ModelArchiveSource::origin)
        });
        for primitive in emitted.primitives() {
            let Some((original, zone)) = primitive
                .original
                .and_then(|row| names.get(usize::from(row.node())).map(|name| (row, name)))
            else {
                self.body = BodyZoneGeometry::Unavailable {
                    reason: "an emitted body primitive has no verified original zone".to_owned(),
                };
                return;
            };
            self.body_words
                .push(source.cloned().map(|entry| (entry, original.node_range())));
            primitives.push(BodyZonePrimitive {
                zone: (*zone).to_owned(),
                mesh: emitted.mesh,
                primitive: primitive.primitive,
                model_lod: emitted.model_lod,
            });
        }
    }

    pub(super) fn finish(mut self, armour: &Armour) -> io::Result<BodyZoneData> {
        if let BodyZoneGeometry::Verified {
            primitives,
            omitted,
        } = &self.body
            && primitives.is_empty()
            && omitted.len() != self.variant.zone_names().len()
        {
            self.body = BodyZoneGeometry::Unavailable {
                reason: "the Character GLB contains no addressable body primitives".to_owned(),
            };
        }
        let mut origins = BodyZoneOrigins::new();
        if matches!(self.body, BodyZoneGeometry::Verified { .. }) {
            origins.body(self.body_words)?;
        }
        let mut parts = Vec::new();
        for (index, (part, meshes)) in armour
            .parts()
            .iter()
            .zip(self.meshes_by_part)
            .enumerate()
            .filter(|(_, (part, meshes))| part.origin() == PartOrigin::Piece && !meshes.is_empty())
        {
            let coverage = part
                .clothing_zones()
                .map(|original| {
                    let zones = original.coverage(self.variant);
                    let coverage = ClothingBodyCoverage {
                        guid: original.guid().to_string(),
                        record_index: original.record_index(),
                        record_path: original.record_path().to_owned(),
                        chunks_complete: original.chunks_complete(),
                        covers: zones.covered().iter().copied().map(str::to_owned).collect(),
                        conditional: zones
                            .undecided()
                            .iter()
                            .copied()
                            .map(str::to_owned)
                            .collect(),
                    };
                    origins.clothing(parts.len(), original, &coverage)?;
                    Ok::<_, io::Error>(coverage)
                })
                .transpose()?;
            parts.push(PartBodyZones {
                part: index,
                meshes,
                coverage,
            });
        }
        if let BodyZoneGeometry::Verified { omitted, .. } = &self.body {
            origins.omitted(omitted)?;
        }
        let (source_inventory, field_origins) = origins.finish()?;
        Ok(BodyZoneData {
            body: self.body,
            parts,
            source_inventory,
            field_origins,
        })
    }
}
