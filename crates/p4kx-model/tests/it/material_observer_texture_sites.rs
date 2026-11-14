// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_model::{MaterialReadUse, MaterialSourceLimits, PaletteSlot};

use crate::material_observer_baseline::TestResult;
use crate::material_observer_support::{self as support, Observed};

const TEXTURES: usize = 65;
const TEXTURE_PARENT: usize = 1;

struct Site {
    indices: Vec<usize>,
    field: &'static str,
    use_kind: MaterialReadUse,
}

impl Site {
    fn matches(&self, actual: &Observed) -> bool {
        actual.material == 0
            && actual.indices == self.indices
            && actual.source_field == self.field
            && actual.use_kind == self.use_kind
    }
}

pub(crate) fn check() -> TestResult {
    let prefix = "<Texture Map='TexSlot2' File='synthetic/prefix_detail.tif'/>";
    let last = "<Texture Map='TexSlot7' File='synthetic/stencil.tif'/>";
    let xml = format!(
        "<Material Shader='MeshDecal' StringGenMask='STENCIL_MAP'><Unrelated/>\
         <Textures><Ignored/>{}{last}</Textures>\
         <PublicParams StencilTintOverride='1'/></Material>",
        prefix.repeat(TEXTURES - 1)
    );
    let (library, events) = support::capture(xml.as_bytes(), MaterialSourceLimits::default())?;
    let material = library.materials.first().ok_or("original material")?;
    assert_eq!(
        material.textures.len(),
        TEXTURES,
        "all original slots retained"
    );
    for (ordinal, texture) in material.textures.iter().enumerate() {
        let (map, file) = if ordinal == TEXTURES - 1 {
            ("TexSlot7", "synthetic/stencil.tif")
        } else {
            ("TexSlot2", "synthetic/prefix_detail.tif")
        };
        assert_eq!(texture.map, map, "original ordered map value");
        assert_eq!(texture.file, file, "original ordered filename");
    }
    assert_eq!(
        material.palette,
        Some(PaletteSlot::Primary),
        "the actual65th stencil selects tint"
    );
    assert_sites(&events);
    Ok(())
}

fn expected() -> Vec<Site> {
    use MaterialReadUse::{Interpreted, Selected};

    let mut sites: Vec<_> = [
        ("SubMaterials", Selected),
        ("Material", Selected),
        ("MatLayers", Selected),
        ("Shader", Interpreted),
        ("StringGenMask", Interpreted),
        ("Textures", Selected),
        ("PublicParams", Selected),
    ]
    .into_iter()
    .map(|(field, use_kind)| Site {
        indices: Vec::new(),
        field,
        use_kind,
    })
    .collect();
    sites.push(Site {
        indices: vec![2],
        field: "StencilTintOverride",
        use_kind: Interpreted,
    });
    for ordinal in 1..=TEXTURES {
        for field in ["Map", "File"] {
            sites.push(Site {
                indices: vec![TEXTURE_PARENT, ordinal],
                field,
                use_kind: Interpreted,
            });
        }
    }
    sites
}

fn assert_sites(events: &[Observed]) {
    let palette: Vec<_> = support::palette(events).collect();
    let expected = expected();
    for event in &palette {
        assert!(
            expected.iter().any(|site| site.matches(event)),
            "no unused/foreign source site or role contributes to the actual palette: {event:?}"
        );
    }
    for site in &expected {
        assert!(
            palette.iter().any(|event| site.matches(event)),
            "the actual consulted site is retained: {:?}/{} {:?}",
            site.indices,
            site.field,
            site.use_kind
        );
    }
    assert!(
        palette.iter().any(
            |event| event.indices == [TEXTURE_PARENT, TEXTURES] && event.source_field == "File"
        ),
        "the late original XML ordinal65 survives the skipped XML sibling and typed index64"
    );
}
