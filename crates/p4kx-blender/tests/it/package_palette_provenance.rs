// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{ArchiveModels, Assembly, AssemblyOptions, ForgePalettes, LoadoutResolver};
use p4kx_blender::{Manifest, PackageInfo, manifest_schema, write_package};
use p4kx_extraction::{Archive, DataForgeEntry};
use p4kx_testkit::{livery_entries, test_key};
use serde_json::Value;

use crate::support::{TestResult, fixture, glb_options, never};

#[path = "package_palette_ancestry.rs"]
mod ancestry;

#[test]
fn package_palette_fields_keep_their_source_separate_from_selection_and_inheritance() -> TestResult
{
    let entity_fixture = fixture(livery_entries())?;
    let mut palette_entries = livery_entries();
    let dcb = palette_entries.remove(0).encrypted();
    palette_entries.insert(0, dcb);
    let palette_fixture = fixture(palette_entries)?;
    let entities = Archive::open(&entity_fixture.path)?;
    let palettes = Archive::open_with_key(&palette_fixture.path, Some(test_key()))?;
    let scratch = tempfile::tempdir()?;

    let livery = package(&entities, &palettes, "TEST_Livery", scratch.path())?;
    let provenance = livery
        .get("palette_provenance")
        .ok_or("palette provenance")?;
    let root = binding(&livery, "")?;
    let child = binding(&livery, "hardpoint_default")?;
    assert_eq!(
        child.get("inheritedFrom"),
        root.get("node"),
        "the supplying ancestor"
    );
    for key in ["payload", "selection"] {
        assert_eq!(
            root.get(key),
            child.get(key),
            "same {key} as the supplying ancestor"
        );
    }
    let twin = binding(&livery, "hardpoint_twin")?;
    let painted = binding(&livery, "hardpoint_painted")?;
    for key in ["payload", "selection"] {
        assert_eq!(
            twin.get(key),
            painted.get(key),
            "shared render input and original {key}"
        );
    }
    verify_payload(&livery, provenance, root)?;
    verify_named(provenance, root)?;

    let bare = package(&entities, &palettes, "TEST_Bare", scratch.path())?;
    verify_default(&bare)?;
    let typed: Manifest = serde_json::from_value(livery.clone())?;
    assert_eq!(
        typed.palette_provenance.as_ref(),
        Some(provenance),
        "palette provenance round-trips"
    );
    let mut historical = livery;
    historical
        .as_object_mut()
        .ok_or("manifest object")?
        .remove("palette_provenance");
    assert!(
        serde_json::from_value::<Manifest>(historical)?
            .palette_provenance
            .is_none(),
        "packages without palette evidence remain readable"
    );
    Ok(())
}

fn package(
    entities: &Archive,
    palettes: &Archive,
    class: &str,
    scratch: &std::path::Path,
) -> Result<Value, Box<dyn std::error::Error>> {
    let entity = DataForgeEntry::read(entities)?;
    let palette = DataForgeEntry::read(palettes)?;
    let entity_forge = entity.open()?;
    let palette_forge = palette.open()?;
    let mut resolver = LoadoutResolver::new(&entity_forge).with_forge_entry(&entity);
    let selected = resolver.entities().find(class)?;
    let loadout = resolver.resolve(&selected)?;
    let models = ArchiveModels::new(entities);
    let assembly = Assembly::build_with_palettes(
        &loadout,
        &models,
        &ForgePalettes::new(&palette_forge).with_entry(&palette),
        &AssemblyOptions::default(),
    )?;
    let out = scratch.join(class);
    write_package(
        &assembly,
        &models,
        &models.textures(),
        &glb_options(0),
        &PackageInfo::default(),
        &out,
        &never,
    )?;
    let manifest = serde_json::from_slice(&std::fs::read(out.join("scene.json"))?)?;
    jsonschema::validator_for(&manifest_schema())?
        .validate(&manifest)
        .map_err(|error| error.to_string())?;
    Ok(manifest)
}

fn binding<'a>(manifest: &'a Value, path: &str) -> Result<&'a Value, Box<dyn std::error::Error>> {
    let nodes = manifest
        .get("nodes")
        .and_then(Value::as_array)
        .ok_or("package nodes")?;
    let index = nodes
        .iter()
        .position(|node| node.get("path").and_then(Value::as_str) == Some(path))
        .ok_or("named node")?;
    let rows = manifest
        .pointer("/palette_provenance/bindings")
        .and_then(Value::as_array)
        .ok_or("palette bindings")?;
    rows.iter()
        .find(|row| row.get("node").and_then(Value::as_u64) == Some(index as u64))
        .ok_or_else(|| "bound node".into())
}

fn row<'a>(
    provenance: &'a Value,
    table: &str,
    index: &Value,
) -> Result<&'a Value, Box<dyn std::error::Error>> {
    let path = format!("/{table}/{}", index.as_u64().ok_or("table index")?);
    provenance
        .pointer(&path)
        .ok_or_else(|| "provenance row".into())
}

fn source_use<'a>(
    provenance: &'a Value,
    origin: &Value,
) -> Result<&'a Value, Box<dyn std::error::Error>> {
    let reference = origin
        .pointer("/sourceUsesRefs/0")
        .ok_or("field contributor")?;
    let uses = provenance
        .pointer("/sourceInventory/uses")
        .and_then(Value::as_array)
        .ok_or("uses")?;
    uses.iter()
        .find(|used| used.get("id") == Some(reference))
        .ok_or_else(|| "registered source use".into())
}

fn field<'a>(value: &'a Value, address: &str) -> Result<&'a Value, Box<dyn std::error::Error>> {
    value
        .pointer(address)
        .ok_or_else(|| format!("missing expected field {address}").into())
}

fn verify_payload(manifest: &Value, provenance: &Value, binding: &Value) -> TestResult {
    let payload = row(provenance, "payloads", field(binding, "/payload")?)?;
    let palette = row(manifest, "palettes", field(payload, "/palette")?)?;
    let origins = field(payload, "/fieldOrigins")?
        .as_object()
        .ok_or("palette field origins")?;
    assert_eq!(
        origins.len(),
        48,
        "name, each aggregate and each sourced RGB/gloss field"
    );
    assert!(
        origins
            .keys()
            .all(|address| palette.pointer(address).is_some()),
        "every origin points to an emitted value"
    );
    assert!(
        origins.values().all(|origin| {
            origin
                .get("containsEncryptedSource")
                .and_then(Value::as_bool)
                == Some(true)
                && origin.get("sourceSetComplete").and_then(Value::as_bool) == Some(true)
        }),
        "the independent palette DCB is encrypted for every emitted value"
    );
    verify_channels(provenance, payload, origins)?;
    let guid = source_use(provenance, field(payload, "/record/guidOrigin")?)?;
    assert_eq!(
        field(guid, "/location/field")?,
        "guid",
        "physical Record identity"
    );
    Ok(())
}

fn verify_channels(
    provenance: &Value,
    payload: &Value,
    origins: &serde_json::Map<String, Value>,
) -> TestResult {
    for (address, expected) in [
        ("/primary/tint/0", "root.entryA.tintColor.r"),
        ("/primary/specular/1", "root.entryA.specColor.g"),
        ("/tertiary/glossiness", "root.entryC.glossiness"),
        ("/decal/2/2", "root.decalColorB.b"),
    ] {
        let used = source_use(provenance, origins.get(address).ok_or("missing channel")?)?;
        assert_eq!(
            field(used, "/location/field")?,
            expected,
            "the exact converted field"
        );
        assert_eq!(
            field(used, "/role")?,
            "conversion",
            "numeric colour conversion"
        );
        assert_eq!(
            field(used, "/location/recordIndex")?,
            field(payload, "/record/index")?,
            "the physical Record owning each channel"
        );
    }
    Ok(())
}

fn verify_named(provenance: &Value, binding: &Value) -> TestResult {
    let selection = row(provenance, "selections", field(binding, "/selection")?)?;
    let payload = row(provenance, "payloads", field(binding, "/payload")?)?;
    assert_eq!(
        field(selection, "/kind")?,
        "named",
        "authored geometry lookup"
    );
    assert_eq!(
        field(selection, "/selector/containsEncryptedSource")?,
        false,
        "independent geometry DCB is clear"
    );
    let selector = source_use(provenance, field(selection, "/selector")?)?;
    let colour = source_use(provenance, field(payload, "/fieldOrigins/~1glass~12")?)?;
    assert!(
        field(selector, "/location/field")?
            .as_str()
            .is_some_and(|field| field.ends_with(".Geometry.Palette.RootRecord")),
        "the geometry chooses the original GUID"
    );
    assert_ne!(
        field(selector, "/source")?,
        field(colour, "/source")?,
        "equal DCB bytes retain physical inputs"
    );
    assert_eq!(
        field(selector, "/role")?,
        "selector",
        "the selector does not supply RGB"
    );
    Ok(())
}

fn verify_default(manifest: &Value) -> TestResult {
    let selected = binding(manifest, "hardpoint_default")?;
    let provenance = field(manifest, "/palette_provenance")?;
    let selection = row(provenance, "selections", field(selected, "/selection")?)?;
    let payload = row(provenance, "payloads", field(selected, "/payload")?)?;
    assert_eq!(
        field(selection, "/kind")?,
        "materialDefault",
        "the material root chose this palette"
    );
    assert_eq!(
        field(selection, "/selector/containsEncryptedSource")?,
        false,
        "actual material selector is clear"
    );
    let selector = source_use(provenance, field(selection, "/selector")?)?;
    assert_eq!(
        field(selector, "/location/field")?,
        "DefaultPalette",
        "the original attribute"
    );
    assert_eq!(
        field(selector, "/location/namespace")?,
        "materialXmlDeclaration",
        "material declaration coordinate"
    );
    let lookup = source_use(provenance, field(selection, "/lookup/origin")?)?;
    assert_eq!(
        field(lookup, "/location/field")?,
        "fileName",
        "the actual path lookup"
    );
    assert_eq!(
        field(lookup, "/location/recordIndex")?,
        field(selection, "/lookup/record/index")?,
        "physical Record supplying the path result"
    );
    let colour = source_use(provenance, field(payload, "/fieldOrigins/~1glass~12")?)?;
    assert_eq!(
        field(lookup, "/source")?,
        field(colour, "/source")?,
        "lookup and values use the same observed DCB"
    );
    assert_ne!(
        field(selector, "/source")?,
        field(lookup, "/source")?,
        "MTL selector and DataForge input are separate"
    );
    Ok(())
}
