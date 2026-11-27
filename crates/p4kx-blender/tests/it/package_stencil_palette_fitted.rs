// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::path::Path;

use p4kx_assembly::{
    ArchiveFiles, ArchiveModels, Assembly, AssemblyOptions, ForgePalettes, LoadoutResolver,
};
use p4kx_blender::Manifest;
use p4kx_extraction::{Archive, DataForge, DataForgeEntry, Entry, Record};
use p4kx_testkit::{ArchiveBuilder, DataForgeBuilder, TestEntity, TestEntry, TestPort};
use serde_json::Value;

use super::{COLOUR_GUID, DECALS, MATERIAL_PATH, MODEL};
use crate::package_palette_provenance as origins;
use crate::support::TestResult;

const HULL: &str = "TEST_FittedStencilHull";
const PAINT: &str = "TEST_FittedStencilPaint";
const PAINT_GUID: u128 = 0x9704;
const BASE_PALETTE_GUID: u128 = 0x9705;
const HULL_GUID: u128 = 0x9706;
const SELECTED_PALETTE: &str = "stencil_fitted";
const APPEARANCE: &str = "Stencil";
const VEHICLE: &str = "Scripts/Entities/Vehicles/Implementations/Xml/stencil.xml";
const IMPLEMENTATION: &[u8] = br#"<Vehicle><Parts><Part name="livery" class="ItemPort">
    <ItemPort><Types><Type type="Paints"/></Types></ItemPort>
</Part></Parts></Vehicle>"#;

#[test]
fn fitted_paint_supplies_the_drawn_stencil_and_its_physical_colour_origins() -> TestResult {
    let scratch = tempfile::tempdir()?;
    let archive_path = scratch.path().join("Data.p4k");
    archive_fixture(&archive_path)?;
    let archive = Archive::open_with_key(&archive_path, Some(p4kx_testkit::test_key()))?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let files = ArchiveFiles::new(&archive);
    let mut resolver = LoadoutResolver::new(&forge)
        .with_forge_entry(&entry)
        .with_files(&files)
        .with_vehicle_appearance();
    let hull = resolver.entities().find(HULL)?;
    let paint = resolver.entities().find(PAINT)?;
    let palette = original_palette(&forge)?;
    let loadout = resolver.resolve(&hull)?;
    let models = ArchiveModels::new(&archive);
    let assembly = Assembly::build_with_palettes(
        &loadout,
        &models,
        &ForgePalettes::new(&forge).with_entry(&entry),
        &AssemblyOptions::default(),
    )?;
    let manifest = super::export_case(&assembly, &models, &scratch.path().join("fitted"), true)?;
    super::verify_package(&manifest, true)?;
    verify_fitted_choice(&manifest, &paint, &palette)?;
    let dcb = DataForgeEntry::locate(&archive).ok_or("the original DataForge Entry")?;
    verify_palette_origins(&manifest, hull.index(), palette.index(), dcb)
}

fn original_palette<'source>(
    forge: &'source DataForge<'source>,
) -> Result<Record<'source>, Box<dyn Error>> {
    let mut candidates =
        forge.records_at_path("libs/foundry/records/tintpalettes/brand/test/stencil_fitted.xml");
    let palette = candidates
        .next()
        .ok_or("the selected physical palette Record")?;
    assert!(
        candidates.next().is_none(),
        "the fixture defines one physical selected palette"
    );
    Ok(palette)
}

fn archive_fixture(path: &Path) -> TestResult {
    let hull = TestEntity::new(HULL_GUID, HULL, "entities/fixture_stencil_hull.xml")
        .vehicle(VEHICLE, "")
        .port(TestPort::new("livery").default_item(PAINT_GUID))
        .geometry(MODEL, Some(MATERIAL_PATH))
        .palette(BASE_PALETTE_GUID, "")
        .geometry_variant(APPEARANCE, MODEL, Some(MATERIAL_PATH))
        .variant_palette(COLOUR_GUID, "");
    let paint = TestEntity::new(PAINT_GUID, PAINT, "entities/fixture_stencil_paint.xml")
        .attach_type("Paints")
        .tags(format!("@{APPEARANCE}"));
    let database = p4kx_testkit::entity_schema(DataForgeBuilder::new())
        .record(hull.record())
        .record(paint.record())
        .record(p4kx_testkit::tint_palette_record_with_decal(
            BASE_PALETTE_GUID,
            "stencil_unfitted",
            [[255; 3]; 3],
            [255; 3],
            [128; 3],
            [[255; 3]; 3],
        ))
        .record(p4kx_testkit::tint_palette_record_with_decal(
            COLOUR_GUID,
            SELECTED_PALETTE,
            [[255; 3]; 3],
            [255; 3],
            [128; 3],
            DECALS,
        ))
        .build();
    super::stencil_assets()?
        .into_iter()
        .fold(ArchiveBuilder::v2(), ArchiveBuilder::entry)
        .entry(TestEntry::stored(format!("Data/{VEHICLE}"), IMPLEMENTATION))
        .entry(TestEntry::zstd("Data/Game2.dcb", database).encrypted())
        .write_to(path)?;
    Ok(())
}

fn verify_fitted_choice(
    manifest: &Manifest,
    paint: &Record<'_>,
    palette: &Record<'_>,
) -> TestResult {
    let proof = manifest
        .paint_provenance
        .as_ref()
        .ok_or("original paint evidence")?;
    assert_eq!(
        origins::field(proof, "/appearance/paintGuid")?.as_str(),
        Some(paint.guid().to_string().as_str()),
        "the fitted paint came from its actual Record"
    );
    assert_eq!(
        origins::field(proof, "/appearance/appearanceTag")?.as_str(),
        Some(APPEARANCE),
        "the original paint tag selects the hull variant"
    );
    assert_eq!(
        origins::field(proof, "/appearance/paletteGuid")?.as_str(),
        Some(palette.guid().to_string().as_str()),
        "the fitted palette is the variant's authored choice"
    );
    Ok(())
}

fn verify_palette_origins(manifest: &Manifest, hull: u32, palette: u32, dcb: &Entry) -> TestResult {
    let provenance = manifest
        .palette_provenance
        .as_ref()
        .ok_or("palette evidence")?;
    let binding = root_palette_binding(manifest, provenance)?;
    let payload = origins::row(provenance, "payloads", origins::field(binding, "/payload")?)?;
    assert_eq!(
        origins::field(payload, "/record/index")?.as_u64(),
        Some(u64::from(palette)),
        "the drawn mesh uses the selected palette's actual physical Record"
    );
    let selection = origins::row(
        provenance,
        "selections",
        origins::field(binding, "/selection")?,
    )?;
    let selector = origins::source_use(provenance, origins::field(selection, "/selector")?)?;
    assert_eq!(
        origins::field(selector, "/location/recordIndex")?.as_u64(),
        Some(u64::from(hull)),
        "the original hull Record supplies the palette selector"
    );
    assert!(
        origins::field(selector, "/location/field")?
            .as_str()
            .is_some_and(|path| {
                path.ends_with(".Geometry.SubGeometry[0].Geometry.Palette.RootRecord")
            }),
        "the effective variant's exact original RootRecord is the selector"
    );
    let source = physical_dataforge(provenance, dcb)?;
    for (mask, colour) in ["R", "G", "B"].into_iter().enumerate() {
        for (channel, component) in ["r", "g", "b"].into_iter().enumerate() {
            let origin = origins::field(
                payload,
                &format!("/fieldOrigins/~1decal~1{mask}~1{channel}"),
            )?;
            verify_colour_origin(
                provenance,
                origin,
                source,
                palette,
                &format!("root.decalColor{colour}.{component}"),
            )?;
        }
    }
    Ok(())
}

fn root_palette_binding<'a>(
    manifest: &Manifest,
    provenance: &'a Value,
) -> Result<&'a Value, Box<dyn Error>> {
    let (node, root) = manifest
        .nodes
        .iter()
        .enumerate()
        .find(|(_, node)| node.path.is_empty())
        .ok_or("the drawn hull node")?;
    let mesh = manifest
        .meshes
        .iter()
        .find(|mesh| root.mesh.as_ref() == Some(&mesh.id))
        .ok_or("the hull uses a package mesh")?;
    let bindings = origins::field(provenance, "/bindings")?
        .as_array()
        .ok_or("palette bindings")?;
    let binding = bindings
        .iter()
        .find(|binding| binding.get("node").and_then(Value::as_u64) == Some(node as u64))
        .ok_or("palette binding for the drawn hull")?;
    let payload = origins::row(provenance, "payloads", origins::field(binding, "/payload")?)?;
    let index = origins::field(payload, "/palette")?
        .as_u64()
        .ok_or("manifest palette index")?;
    let palette = manifest
        .palettes
        .get(usize::try_from(index)?)
        .ok_or("the emitted palette")?;
    assert_eq!(
        mesh.palette_id.as_ref(),
        Some(&palette.id),
        "evidence binds the drawn palette"
    );
    assert_eq!(
        palette.name, SELECTED_PALETTE,
        "fitting supersedes the base palette"
    );
    Ok(binding)
}

fn physical_dataforge<'a>(provenance: &'a Value, dcb: &Entry) -> Result<&'a Value, Box<dyn Error>> {
    let sources = origins::field(provenance, "/sourceInventory/sources")?
        .as_array()
        .ok_or("physical inputs")?;
    let source = sources
        .iter()
        .find(|source| {
            source.pointer("/path/value").and_then(Value::as_str) == Some(dcb.path())
                && source.get("entryIndex").and_then(Value::as_u64) == Some(dcb.index() as u64)
        })
        .ok_or("the original palette DataForge Entry in the inventory")?;
    origins::field(source, "/id")
}

fn verify_colour_origin(
    provenance: &Value,
    origin: &Value,
    source: &Value,
    palette: u32,
    path: &str,
) -> TestResult {
    assert_eq!(
        origins::field(origin, "/containsEncryptedSource")?,
        true,
        "encrypted palette value at {path}"
    );
    assert_eq!(
        origins::field(origin, "/sourceSetComplete")?,
        true,
        "complete original leaf at {path}"
    );
    assert_eq!(
        origins::field(origin, "/sourceUsesRefs")?
            .as_array()
            .map(Vec::len),
        Some(1),
        "one physical leaf supplies the converted value at {path}",
    );
    let used = origins::source_use(provenance, origin)?;
    assert_eq!(
        origins::field(used, "/source")?,
        source,
        "the actual DataForge Entry at {path}"
    );
    assert_eq!(
        origins::field(used, "/role")?,
        "conversion",
        "sRGB decoding at {path}"
    );
    assert_eq!(
        origins::field(used, "/location/field")?,
        path,
        "the exact original colour component"
    );
    assert_eq!(
        origins::field(used, "/location/recordIndex")?.as_u64(),
        Some(u64::from(palette)),
        "the physical palette Record at {path}"
    );
    Ok(())
}
