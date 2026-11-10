// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::model::MaterialLibrary;
use p4kx_extraction::{self as extraction, Archive};
use p4kx_testkit::{TestEntry, TestMtl, TestMtlMaterial};

use crate::support::{Install, TestResult};

pub(crate) const MATERIAL: &str = "Data\\Objects\\exact.mtl";
pub(crate) const PAYLOAD: &[u8] = b"the exact requested texture source";

#[derive(Clone, Copy)]
pub(crate) enum Encoding {
    Text,
    CryXml,
}

#[derive(Clone, Copy)]
pub(crate) enum TexturePath {
    Reusable,
    OverInputLimit,
}

pub(crate) struct Texts {
    pub(crate) name: String,
    pub(crate) shader: String,
    pub(crate) surface: String,
    pub(crate) mask: String,
    pub(crate) palette: String,
    pub(crate) map: String,
    pub(crate) file: String,
    pub(crate) parameter: String,
    pub(crate) value: String,
    pub(crate) layer: String,
    pub(crate) layer_path: String,
    pub(crate) tint: String,
}

pub(crate) fn texts(path: TexturePath) -> Texts {
    let count = match path {
        TexturePath::Reusable => 300,
        TexturePath::OverInputLimit => 1100,
    };
    Texts {
        name: format!("{}tail", "é".repeat(280)),
        shader: format!("Shader_{}tail", "s".repeat(280)),
        surface: format!("Surface_{}tail", "u".repeat(280)),
        mask: format!("{}tail", "m".repeat(280)),
        palette: format!(
            "libs/foundry/records/tintpalette/{}tail.xml",
            "a".repeat(280)
        ),
        map: format!("TexSlot_{}tail", "t".repeat(280)),
        file: format!("objects/{}_B.dds", "f".repeat(count)),
        parameter: format!("Parameter{}Tail", "p".repeat(280)),
        value: format!("  {}\\n%%_尾  ", "v".repeat(280)),
        layer: format!("Layer{}Tail", "l".repeat(280)),
        layer_path: format!("objects/{}tail.mtl", "r".repeat(280)),
        tint: "1".repeat(280),
    }
}

pub(crate) fn exact(texts: &Texts, encoding: Encoding) -> TestResult<Install> {
    let material = TestMtlMaterial::new(&texts.name)
        .shader(&texts.shader)
        .surface_type(&texts.surface)
        .attribute("StringGenMask", &texts.mask)
        .texture(&texts.map, &texts.file)
        .public_param(&texts.parameter, &texts.value)
        .layer_with(&[
            ("Name", &texts.layer),
            ("Path", &texts.layer_path),
            ("PaletteTint", &texts.tint),
        ]);
    let library = TestMtl::single(&material).with_default_palette(&texts.palette);
    let file = entry_path(&texts.file);
    let other = file.replace("_B.dds", "_A.dds");
    let install = install(
        &library,
        encoding,
        vec![
            TestEntry::stored(file, PAYLOAD),
            TestEntry::stored(other, b"the other genuine same-prefix texture"),
        ],
    )?;
    check_original(&original(&install)?, texts)?;
    let archive = Archive::open_with_key(install.archive(), None)?;
    let entry = extraction::find_entry(&archive, &entry_path(&texts.file))
        .ok_or("actual original full texture path")?;
    let mut reader = archive.open_entry(entry)?;
    let mut bytes = Vec::new();
    std::io::Read::read_to_end(&mut reader, &mut bytes)?;
    assert_eq!(
        bytes, PAYLOAD,
        "the real complete path reaches its exact original payload"
    );
    Ok(install)
}

pub(crate) fn entry_path(file: &str) -> String {
    format!("Data\\{}", file.replace('/', "\\"))
}

pub(crate) fn install(
    library: &TestMtl,
    encoding: Encoding,
    mut extra: Vec<TestEntry>,
) -> TestResult<Install> {
    let bytes = match encoding {
        Encoding::Text => library.text_xml(),
        Encoding::CryXml => library.cryxml(),
    };
    extra.push(TestEntry::stored(MATERIAL, bytes));
    Install::new(extra)
}

pub(crate) fn original(install: &Install) -> TestResult<MaterialLibrary> {
    let archive = Archive::open_with_key(install.archive(), None)?;
    let entry =
        extraction::find_entry(&archive, MATERIAL).ok_or("actual original material Entry")?;
    let mut reader = archive.open_entry(entry)?;
    let mut bytes = Vec::new();
    std::io::Read::read_to_end(&mut reader, &mut bytes)?;
    Ok(MaterialLibrary::parse(entry.path(), &bytes)?)
}

fn check_original(library: &MaterialLibrary, texts: &Texts) -> TestResult {
    assert_eq!(
        library.default_palette.as_deref(),
        Some(texts.palette.as_str()),
        "actual original palette path is exact"
    );
    assert_eq!(library.materials.len(), 1, "one actual original material");
    let material = library
        .materials
        .first()
        .ok_or("actual original material")?;
    assert_eq!(
        material.name, texts.name,
        "original decoded name before MCP"
    );
    assert_eq!(
        material.shader, texts.shader,
        "original decoded shader before MCP"
    );
    assert_eq!(
        material.surface_type, texts.surface,
        "original decoded surface before MCP"
    );
    assert_eq!(
        material.string_gen_mask, texts.mask,
        "original decoded mask before MCP"
    );
    let texture = material
        .textures
        .first()
        .ok_or("original texture declaration")?;
    assert_eq!(
        (&texture.map, &texture.file),
        (&texts.map, &texts.file),
        "exact original texture names/path"
    );
    let (name, value) = material
        .public_params
        .first()
        .ok_or("original parameter declaration")?;
    assert_eq!(
        (name, value),
        (&texts.parameter, &texts.value),
        "original parameter spelling/value"
    );
    let layer = material.layers.first().ok_or("actual original layer")?;
    for (name, value) in [
        ("Name", &texts.layer),
        ("Path", &texts.layer_path),
        ("PaletteTint", &texts.tint),
    ] {
        assert_eq!(
            layer.attribute(name),
            Some(value.as_str()),
            "actual original layer attribute {name}"
        );
    }
    Ok(())
}

pub(crate) fn parameter_library(values: &[(&str, &str)]) -> TestMtl {
    let materials: Vec<_> = values
        .iter()
        .map(|(name, value)| TestMtlMaterial::new(*name).public_param("Value", value))
        .collect();
    TestMtl::multi(&materials)
}
