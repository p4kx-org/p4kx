// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::Read;

use p4kx_extraction::{self as extraction, Archive, cryxml};
use p4kx_testkit::{TestMtl, TestMtlMaterial};

use crate::exact_material_fixture::{self as fixture, Encoding};
use crate::support::{Install, TestResult};

pub(crate) const COUNT: usize = 65;

#[derive(Clone, Copy)]
pub(crate) enum Kind {
    Textures,
    Layers,
}

impl Kind {
    pub(crate) const fn field(self) -> &'static str {
        match self {
            Self::Textures => "textures",
            Self::Layers => "layers",
        }
    }

    pub(crate) const fn count_field(self) -> &'static str {
        match self {
            Self::Textures => "texture_count",
            Self::Layers => "layer_count",
        }
    }

    const fn list(self) -> &'static str {
        match self {
            Self::Textures => "Textures",
            Self::Layers => "MatLayers",
        }
    }

    pub(crate) fn attributes(self, ordinal: usize) -> [(&'static str, &'static str, String); 2] {
        match self {
            Self::Textures => [
                ("Map", "map", format!("TexSlot{ordinal:02}")),
                ("File", "file", format!("objects/texture_{ordinal:02}.dds")),
            ],
            Self::Layers => [
                ("Name", "name", format!("Layer{ordinal:02}")),
                ("Path", "path", format!("objects/layer_{ordinal:02}.mtl")),
            ],
        }
    }
}

pub(crate) fn install(kind: Kind) -> TestResult<Install> {
    let mut material = TestMtlMaterial::new("complete");
    for ordinal in 0..COUNT {
        let [(_, _, name), (_, _, path)] = kind.attributes(ordinal);
        material = match kind {
            Kind::Textures => material.texture(&name, &path),
            Kind::Layers => material.layer_with(&[
                ("Name", name.as_str()),
                ("Path", path.as_str()),
                ("PaletteTint", "1"),
            ]),
        };
    }
    fixture::install(&TestMtl::single(&material), Encoding::CryXml, Vec::new())
}

pub(crate) fn original(install: &Install, kind: Kind) -> TestResult {
    with_original(install, |_, root| {
        assert_eq!(root.name(), "Material", "actual original material root");
        let list = root
            .children()
            .find(|child| child.name() == kind.list())
            .ok_or("original list")?;
        let children: Vec<_> = list.children().collect();
        assert_eq!(
            children.len(),
            COUNT,
            "all original XML declarations before Model admission"
        );
        for (ordinal, child) in children.into_iter().enumerate() {
            let tag = match kind {
                Kind::Textures => "Texture",
                Kind::Layers => "Layer",
            };
            assert_eq!(child.name(), tag, "original typed XML occurrence");
            for (field, _, value) in kind.attributes(ordinal) {
                assert_eq!(
                    child.attribute(field),
                    Some(value.as_str()),
                    "exact original attribute/order before public producer"
                );
            }
            if matches!(kind, Kind::Layers) {
                assert_eq!(
                    child.attribute("PaletteTint"),
                    Some("1"),
                    "original layer tint"
                );
            }
        }
        Ok(())
    })
}

pub(crate) fn with_original(
    install: &Install,
    inspect: impl FnOnce(&[u8], cryxml::Element<'_>) -> TestResult,
) -> TestResult {
    let archive = Archive::open(install.archive())?;
    let entry =
        extraction::find_entry(&archive, fixture::MATERIAL).ok_or("original material Entry")?;
    let mut bytes = Vec::new();
    archive.open_entry(entry)?.read_to_end(&mut bytes)?;
    let document = cryxml::parse(&bytes)?;
    let root = document.root();
    inspect(&bytes, root)
}
