// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The hull of the RSI Ursa Medivac under the Respite livery is drawn in the greys of the palette:
//! the vertex colours of its layered materials are the shares of their layers, no tint
//! (ADR-0318).

use std::collections::BTreeSet;
use std::error::Error;

use p4kx_assembly::{
    ArchiveModels, Assembly, AssemblyOptions, ForgePalettes, LoadoutResolver, PaletteFailure,
    PaletteProvider,
};
use p4kx_dataforge::{DataForge, Guid};
use p4kx_extraction::{Archive, DataForgeEntry};
use p4kx_gltf::{GlbOptions, MaterialMode};
use p4kx_model::TintPalette;
use p4kx_testkit::gltf::Semantic;
use p4kx_testkit::{check_glb, real_archive_key, real_archive_path};

type TestResult = Result<(), Box<dyn Error>>;

const URSA: &str = "RSI_Ursa_Medivac";
const RESPITE: &str = "rsi_ursa_medivac_black_white_grey_vip";
const TINT_PALETTE_TREE: &str = "TintPaletteTree";
const HULL_MATERIAL: &str = "Painted_Metal_Tiled";
const GREY_TOLERANCE: f32 = 0.02;
const TINTED_COLOUR_SPREAD: u8 = 64;

/// Gives every part the one palette, as a paint item of the livery would.
struct OnePalette {
    palette: TintPalette,
    guid: Guid,
}

impl PaletteProvider for OnePalette {
    fn by_guid(&self, _guid: Guid) -> Result<TintPalette, PaletteFailure> {
        Ok(self.palette.clone())
    }

    fn guid_at(&self, _path: &str) -> Result<Guid, PaletteFailure> {
        Ok(self.guid)
    }
}

fn respite(forge: &DataForge<'_>, palettes: ForgePalettes<'_>) -> Result<OnePalette, String> {
    forge
        .records()
        .filter(|record| {
            record
                .type_name()
                .is_ok_and(|name| name == TINT_PALETTE_TREE)
        })
        .find_map(|record| {
            let palette = palettes.by_guid(record.guid()).ok()?;
            (palette.name == RESPITE).then(|| OnePalette {
                palette,
                guid: record.guid(),
            })
        })
        .ok_or_else(|| format!("the palette {RESPITE}"))
}

fn has_coloured_vertices(assembly: &Assembly) -> bool {
    assembly
        .parts()
        .iter()
        .filter_map(|part| part.model())
        .filter_map(|model| model.lods.first())
        .flat_map(|lod| lod.meshes.iter())
        .filter_map(|mesh| mesh.colours.as_ref())
        .flatten()
        .any(|&[red, green, blue, _]| {
            red.max(green).max(blue) - red.min(green).min(blue) >= TINTED_COLOUR_SPREAD
        })
}

/// Returns the names of the layered materials of `assembly`, whose vertex colours are shares.
fn layer_share_materials(assembly: &Assembly) -> BTreeSet<&str> {
    assembly
        .parts()
        .iter()
        .filter_map(|part| part.materials().library())
        .flat_map(|library| library.materials.iter())
        .filter(|material| material.shares_layers_by_vertex_colours())
        .map(|material| material.name.as_str())
        .collect()
}

#[test]
#[expect(
    clippy::print_stdout,
    reason = "the names of the layered materials are the measurement; the test prints them"
)]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn the_hull_of_the_ursa_medivac_is_grey_under_the_respite_livery() -> TestResult {
    let archive = Archive::open_with_key(real_archive_path()?, Some(real_archive_key()?))?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let models = ArchiveModels::new(&archive);
    let respite = respite(&forge, ForgePalettes::new(&forge))?;
    let mut resolver = LoadoutResolver::new(&forge);
    let record = resolver.entities().find(URSA)?;
    let tree = resolver.resolve(&record)?;
    let assembly =
        Assembly::build_with_palettes(&tree, &models, &respite, &AssemblyOptions::default())?;

    let shared = layer_share_materials(&assembly);
    assert!(shared.contains(HULL_MATERIAL), "the hull blends layers");
    println!("{} layered materials: {shared:?}", shared.len());
    assert!(
        has_coloured_vertices(&assembly),
        "the vertex colours of the hull are coloured, which is why they must not tint it"
    );

    let options = GlbOptions::default().with_materials(MaterialMode::Colours);
    let mut bytes = Vec::new();
    assembly.write_glb_with(&options, &models.textures(), &mut bytes)?;
    let glb = check_glb(&bytes)?;

    let (mut hull, mut coloured_layered) = (0_usize, Vec::new());
    for primitive in glb.document.meshes().flat_map(|mesh| mesh.primitives()) {
        let material = primitive.material();
        let name = material.name().unwrap_or_default();
        if shared.contains(name) && primitive.get(&Semantic::Colors(0)).is_some() {
            coloured_layered.push(name.to_owned());
        }
        if name == HULL_MATERIAL {
            let [red, green, blue, _] = material.pbr_metallic_roughness().base_color_factor();
            assert!(
                red.max(green).max(blue) - red.min(green).min(blue) < GREY_TOLERANCE,
                "the hull is drawn in the greys of the palette: {red} {green} {blue}"
            );
            hull += 1;
        }
    }
    assert!(hull > 0, "the GLB draws the hull");
    assert!(
        coloured_layered.is_empty(),
        "a viewer would multiply the layer shares into the colour: {coloured_layered:?}"
    );
    Ok(())
}
