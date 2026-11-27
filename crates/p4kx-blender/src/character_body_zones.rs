// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;
use std::path::Path;

use p4kx_assembly::{Armour, ArmourPart, BodyVariant, PartOrigin};
use p4kx_extraction::{EntryOrigin, ModelArchiveSource};
use p4kx_gltf::MeshVertexMap;
use p4kx_model::{MeshExtraWord, MeshSubmeshSource};

use crate::character_body_zone_origins::{BodyZoneOrigins, OriginalZoneWord, SkinZoneEvidence};
use crate::manifest_body_zones::{
    BodyZoneData, BodyZoneGeometry, BodyZonePrimitive, ClothingBodyCoverage, PartBodyZones,
};

pub(super) struct ObserveBodyZones {
    variant: BodyVariant,
    body: BodyZoneGeometry,
    worn: Vec<WornGeometry>,
    body_words: Vec<OriginalZoneWord>,
}

#[derive(Clone, Default)]
struct WornGeometry {
    meshes: Vec<usize>,
    zones: Vec<WornZone>,
}

#[derive(Clone)]
struct WornZone {
    primitive: BodyZonePrimitive,
    entry: Option<EntryOrigin>,
    descriptor: MeshSubmeshSource,
    word: MeshExtraWord,
}

impl WornGeometry {
    fn observe(&mut self, emitted: &MeshVertexMap<'_>, part: &ArmourPart) {
        self.meshes.push(emitted.mesh);
        let Some(clothing) = part.clothing_zones() else {
            return;
        };
        if !Path::new(part.file())
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("skin"))
        {
            return;
        }
        let source = geometry_source(emitted, part);
        for primitive in emitted.primitives() {
            let Some(descriptor) = primitive.original else {
                continue;
            };
            let Some(word) = descriptor.extra_word() else {
                continue;
            };
            let Some(zone) = clothing.name_for_skin_word(word.value()) else {
                continue;
            };
            self.zones.push(WornZone {
                primitive: BodyZonePrimitive {
                    zone: zone.to_owned(),
                    mesh: emitted.mesh,
                    primitive: primitive.primitive,
                    model_lod: emitted
                        .origin
                        .map_or(emitted.model_lod, |origin| origin.lod),
                },
                entry: source.cloned(),
                descriptor,
                word,
            });
        }
    }

    fn manifest_part(
        self,
        rig_index: usize,
        output_index: usize,
        part: &ArmourPart,
        variant: BodyVariant,
        origins: &mut BodyZoneOrigins,
    ) -> io::Result<PartBodyZones> {
        let coverage = part
            .clothing_zones()
            .map(|original| {
                let zones = original.coverage(variant);
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
                origins.clothing(output_index, original, &coverage)?;
                for (index, zone) in self.zones.iter().enumerate() {
                    let observed = zone.entry.as_ref().map(|entry| SkinZoneEvidence {
                        entry,
                        node: zone.descriptor.node_range(),
                        word: zone.word.range(),
                    });
                    let path = format!("/parts/{output_index}/primitives/{index}/zone");
                    origins.worn_primitive(&path, original, &zone.primitive.zone, observed)?;
                }
                Ok::<_, io::Error>(coverage)
            })
            .transpose()?;
        Ok(PartBodyZones {
            part: rig_index,
            meshes: self.meshes,
            primitives: self.zones.into_iter().map(|zone| zone.primitive).collect(),
            coverage,
        })
    }
}

fn geometry_source<'a>(
    emitted: &MeshVertexMap<'_>,
    part: &'a ArmourPart,
) -> Option<&'a EntryOrigin> {
    let geometry = emitted.origin?;
    part.model_archive_sources()
        .iter()
        .find(|source| source.file() == geometry.file)
        .and_then(ModelArchiveSource::origin)
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
            worn: vec![WornGeometry::default(); armour.parts().len()],
            body_words: Vec::new(),
        }
    }

    pub(super) fn observe(
        &mut self,
        emitted: &MeshVertexMap<'_>,
        part: &ArmourPart,
    ) -> io::Result<()> {
        let worn = self
            .worn
            .get_mut(emitted.part)
            .ok_or_else(|| io::Error::other("emitted Character mesh lost its rig part"))?;
        match part.origin() {
            PartOrigin::Body => self.record_body(emitted, part),
            PartOrigin::Piece => worn.observe(emitted, part),
            _ => {}
        }
        Ok(())
    }

    fn record_body(&mut self, emitted: &MeshVertexMap<'_>, part: &ArmourPart) {
        let BodyZoneGeometry::Verified { primitives, .. } = &mut self.body else {
            return;
        };
        let names = self.variant.zone_names();
        let source = geometry_source(emitted, part);
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
                model_lod: emitted
                    .origin
                    .map_or(emitted.model_lod, |origin| origin.lod),
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
        for (index, (part, worn)) in
            armour
                .parts()
                .iter()
                .zip(self.worn)
                .enumerate()
                .filter(|(_, (part, worn))| {
                    part.origin() == PartOrigin::Piece && !worn.meshes.is_empty()
                })
        {
            parts.push(worn.manifest_part(index, parts.len(), part, self.variant, &mut origins)?);
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
