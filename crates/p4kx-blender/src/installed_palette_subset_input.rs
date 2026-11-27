// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use p4kx_assembly::{ForgePalettes, PaletteProvider};
use p4kx_extraction::{
    Archive, DataForge, ModelArchiveSource, ModelSource, OriginDigestWriter, PathIndex,
    TextureVerdicts,
};
use p4kx_model::{LodSelection, MaterialReading, Model, PositionBox, TintPalette};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::{Check, sources};

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct SelectedEntry {
    pub(super) entry_index: usize,
    pub(super) path: String,
    pub(super) decoded_bytes: u64,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Build {
    branch: String,
    version: String,
    change: String,
    id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Layer {
    pub(super) path: String,
    pub(super) entry: SelectedEntry,
    pub(super) source_ordinal: usize,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Request {
    pub(super) build: Build,
    archive_bytes: u64,
    archive_mtime_ns: u128,
    manifest_sha256: String,
    pub(super) source_receipt: Pin,
    pub(super) input_receipt: Pin,
    pub(super) directory_receipt: Pin,
    pub(super) geometry: Vec<SelectedEntry>,
    pub(super) wrapper: SelectedEntry,
    pub(super) layers: Vec<Layer>,
    pub(super) dataforge: SelectedEntry,
    pub(super) palette_record: u32,
    pub(super) palette_guid: String,
    palette_glossiness: f32,
    pub(super) material_ordinal: usize,
    pub(super) material_name: String,
    pub(super) shader: String,
    #[serde(rename = "genMask")]
    pub(super) string_gen_mask: String,
    pub(super) public_params: Vec<(String, String)>,
    pub(super) layer_attributes: Vec<Vec<(String, String)>>,
    pub(super) textures: Vec<String>,
    pub(super) allowed_sources: Vec<SelectedEntry>,
    pub(super) closure_bytes: u64,
    pub(super) lod: u32,
    pub(super) mesh: usize,
    pub(super) subset: usize,
    pub(super) triangles: usize,
    pub(super) mip: u32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Pin {
    path: PathBuf,
    sha256: String,
}

pub(super) fn load() -> Check<(Request, Value)> {
    let path = std::env::var_os("P4KX_INSTALLED_PALETTE_SUBSET_SELECTION")
        .ok_or("P4KX_INSTALLED_PALETTE_SUBSET_SELECTION is required")?;
    let bytes = fs::read(path)?;
    let request: Request = serde_json::from_slice(&bytes)?;
    for pin in [
        &request.source_receipt,
        &request.input_receipt,
        &request.directory_receipt,
    ] {
        if digest(&fs::read(&pin.path)?)? != pin.sha256 {
            return Err("an original source receipt pin differs".into());
        }
    }
    let pins = json!({"selectionSha256":digest(&bytes)?,
        "subset":request.source_receipt.sha256,"inputs":request.input_receipt.sha256,
        "directory":request.directory_receipt.sha256});
    Ok((request, pins))
}

fn digest(bytes: &[u8]) -> Check<String> {
    let mut writer = OriginDigestWriter::new(io::sink());
    writer.write_all(bytes)?;
    Ok(writer.finish()?.sha256().to_owned())
}

impl Request {
    pub(super) fn check_archive(&self, archive: &Archive) -> Check {
        let metadata = fs::metadata(archive.path())?;
        if metadata.len() != self.archive_bytes
            || metadata.modified()?.duration_since(UNIX_EPOCH)?.as_nanos() != self.archive_mtime_ns
        {
            return Err("the Archive differs from its original before/after binding".into());
        }
        let manifest = archive.path().with_file_name("build_manifest.id");
        if digest(&fs::read(manifest)?)? != self.manifest_sha256 {
            return Err("the original Build manifest differs".into());
        }
        let actual = archive.build().ok_or("installed Build identity")?;
        if actual.branch() != self.build.branch
            || actual.version() != self.build.version
            || actual.change_number() != self.build.change
            || actual.build_id() != self.build.id
        {
            return Err("the original Build identity differs".into());
        }
        Ok(())
    }
}

pub(super) fn palette(
    index: &PathIndex<'_>,
    request: &Request,
    ledger: &mut sources::Ledger,
) -> Check<TintPalette> {
    let bytes = ledger.read(
        index,
        &request.dataforge,
        sources::ReadRole::PaletteDatabase,
    )?;
    let forge = DataForge::open(&bytes)?;
    let guid = request.palette_guid.parse()?;
    let record = forge
        .record(request.palette_record)
        .ok_or("original physical palette Record")?;
    if record.guid() != guid
        || record.type_name()? != "TintPaletteTree"
        || forge.records_by_guid(guid).take(2).count() != 1
    {
        return Err("the palette Record is absent, ambiguous or changed".into());
    }
    let read = ForgePalettes::new(&forge).by_guid_observed(guid)?;
    if read
        .record
        .is_none_or(|source| source.index != request.palette_record)
        || read.value.primary.glossiness.to_bits() != request.palette_glossiness.to_bits()
    {
        return Err("the normal palette parser selected a different physical value".into());
    }
    Ok(read.value)
}

pub(super) fn model(
    index: &PathIndex<'_>,
    request: &Request,
    ledger: &mut sources::Ledger,
) -> Check<(Model, Vec<ModelArchiveSource>, Vec<[f32; 2]>)> {
    let primary = request
        .geometry
        .iter()
        .find(|entry| {
            Path::new(&entry.path).extension().is_some_and(|extension| {
                extension.eq_ignore_ascii_case("cga") || extension.eq_ignore_ascii_case("cgf")
            })
        })
        .ok_or("one original primary geometry Entry")?;
    let source = ModelSource::from_index_observing_model_sources(
        index,
        &primary.path,
        LodSelection::Only(request.lod),
        PositionBox::default(),
        MaterialReading::Skipped,
        &TextureVerdicts::new(),
    )?;
    let mut read = source.into_model_with_archive_sources();
    let actual: Vec<_> = read
        .sources
        .iter()
        .filter_map(|row| row.origin())
        .map(|row| u64::from(row.entry().ordinal()))
        .collect();
    let expected: Vec<_> = request
        .geometry
        .iter()
        .map(|row| u64::try_from(row.entry_index))
        .collect::<Result<_, _>>()?;
    if actual != expected {
        return Err("the model's physical source closure differs".into());
    }
    for entry in &request.geometry {
        ledger.verify(index, entry, sources::ReadRole::GeometryChecksumReplay)?;
    }
    let uvs = select(&mut read.model, request)?;
    Ok((read.model, read.sources, uvs))
}

fn select(model: &mut Model, request: &Request) -> Check<Vec<[f32; 2]>> {
    let [lod] = model.lods.as_mut_slice() else {
        return Err("one original LOD".into());
    };
    if lod.level != request.lod {
        return Err("the original LOD differs".into());
    }
    let mesh = lod.meshes.get(request.mesh).ok_or("original mesh")?;
    if mesh.non_finite_uvs != 0 {
        return Err("the original source UVs require normalization".into());
    }
    let subset = mesh
        .submeshes
        .get(request.subset)
        .ok_or("original subset")?;
    if usize::from(subset.material_id) != request.material_ordinal {
        return Err("the original subset material assignment differs".into());
    }
    let indices = mesh
        .submesh_indices(subset)
        .ok_or("the original subset index span")?;
    if indices.len() / 3 != request.triangles || indices.len() % 3 != 0 {
        return Err("the original triangle count differs".into());
    }
    let mut probes = Vec::new();
    for triangle in [
        0,
        request.triangles / 3,
        2 * request.triangles / 3,
        request.triangles - 1,
    ] {
        let mut uv = [0.0; 2];
        for vertex in &indices[triangle * 3..triangle * 3 + 3] {
            let source = mesh
                .uvs
                .get(usize::try_from(*vertex)?)
                .ok_or("original source UV")?;
            uv[0] += source[0] / 3.0;
            uv[1] += source[1] / 3.0;
        }
        if uv.iter().any(|value| !value.is_finite()) {
            return Err("non-finite source UV probe".into());
        }
        probes.push(uv);
    }
    for (at, mesh) in lod.meshes.iter_mut().enumerate() {
        mesh.invalidate_vertex_origin_order();
        if at == request.mesh {
            let mut slot = 0;
            mesh.submeshes.retain(|_| {
                let keep = slot == request.subset;
                slot += 1;
                keep
            });
        } else {
            mesh.submeshes.clear();
        }
    }
    Ok(probes)
}

pub(super) fn canonical(path: &str) -> String {
    path.replace('\\', "/").to_ascii_lowercase()
}
