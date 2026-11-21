// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_model::{MaterialReadField, MaterialReadUse, MaterialSourceLimits, PaletteSlot};

use crate::material_observer_baseline::TestResult;
use crate::material_observer_support as support;

#[test]
fn material_observer_palette_retains_invalid_prior_and_chosen_original_layer_sites_only()
-> TestResult {
    let xml = b"<Material><Ignored/><SubMaterials><MatRef/><Material Shader='GlassPBR' StringGenMask='USE_TINT_PALETTE'><Ignored/><MatLayers><Unknown/><Layer PaletteTint='bad'/><Layer PaletteTint='2'/><Layer PaletteTint='3'/></MatLayers></Material></SubMaterials></Material>";
    let (library, events) = support::capture(xml, MaterialSourceLimits::default())?;
    assert_eq!(
        library.materials.get(1).ok_or("typed material")?.palette,
        Some(PaletteSlot::Secondary),
        "the first valid layer wins"
    );
    let layers: Vec<_> = support::palette(&events)
        .filter(|event| event.source_field == "PaletteTint")
        .collect();
    assert_eq!(
        layers.len(),
        2,
        "only the rejected prior layer and winner contribute"
    );
    let first = layers.first().ok_or("rejected layer")?;
    let chosen = layers.get(1).ok_or("chosen layer")?;
    assert_eq!(
        first.indices,
        [1, 1, 1, 1],
        "original ignored children do not compact layer ordinals"
    );
    assert_eq!(
        first.use_kind,
        MaterialReadUse::Selected,
        "invalid prior token selects continued search"
    );
    assert_eq!(
        chosen.indices,
        [1, 1, 1, 2],
        "the exact winning original occurrence contributes"
    );
    assert_eq!(
        chosen.use_kind,
        MaterialReadUse::Interpreted,
        "the chosen number is interpreted"
    );
    assert!(
        !support::palette(&events).any(|event| matches!(
            event.source_field.as_str(),
            "Shader" | "StringGenMask" | "Textures" | "PublicParams"
        )),
        "discarded fallback branches contribute nothing"
    );
    Ok(())
}

#[test]
fn material_observer_glass_excludes_evaluated_but_discarded_mask_inputs() -> TestResult {
    for (shader, expected, mask_used) in [
        ("Other", None, false),
        ("GlassPBR", Some(PaletteSlot::Glass), true),
    ] {
        let xml = format!("<Material Shader='{shader}' StringGenMask='USE_TINT_PALETTE'/>");
        let (library, events) = support::capture(xml.as_bytes(), MaterialSourceLimits::default())?;
        assert_eq!(
            library.materials.first().ok_or("material")?.palette,
            expected,
            "glass selection keeps its existing result"
        );
        assert_eq!(
            support::palette(&events).any(|event| event.source_field == "StringGenMask"),
            mask_used,
            "a non-glass result discards the eagerly evaluated mask boolean"
        );
        assert!(
            events
                .iter()
                .any(|event| event.field == MaterialReadField::StringGenMask
                    && event.source_field == "StringGenMask"
                    && event.use_kind == MaterialReadUse::Authored),
            "the mask's own copied output retains its source independently"
        );
    }
    Ok(())
}

#[test]
fn material_observer_stencil_uses_original_retained_texture_and_first_parameter_sites() -> TestResult
{
    let xml = b"<Material Shader='MeshDecal' StringGenMask='STENCIL_MAP'><Ignored/><Textures><Ignored/><Texture Map='TexSlot7' File=''/><Texture Map='TexSlot1' File='synthetic/detail_detail.tif'/><Texture Map='TexSlot7' File='synthetic/stencil.tif'/></Textures><PublicParams StencilDiffuseColor='1,0,0' StencilOpacity='0' stenciltintoverride='2'/><PublicParams StencilTintOverride='1'/></Material>";
    let (library, events) = support::capture(xml, MaterialSourceLimits::default())?;
    assert_eq!(
        library.materials.first().ok_or("material")?.palette,
        Some(PaletteSlot::Decal),
        "the first matching override determines tint"
    );
    let inputs: Vec<_> = support::palette(&events).collect();
    assert!(
        inputs
            .iter()
            .any(|event| event.indices == [1, 3] && event.source_field == "File"),
        "the retained stencil map uses its original slot"
    );
    assert!(
        !inputs
            .iter()
            .any(|event| event.indices == [1, 0] || event.indices == [1, 1]),
        "ignored and empty-file slots are not compacted into consulted typed slots"
    );
    assert!(
        inputs.iter().any(|event| event.indices == [2]
            && event.source_field == "stenciltintoverride"
            && event.use_kind == MaterialReadUse::Interpreted),
        "the original first parameter spelling and site survive"
    );
    assert!(
        !inputs.iter().any(|event| event.indices == [3]),
        "a later duplicate override contributes nothing"
    );
    assert!(
        !inputs.iter().any(|event| matches!(
            event.source_field.as_str(),
            "StencilDiffuseColor" | "StencilOpacity"
        )),
        "computed but unused stencil colour and opacity do not choose the palette"
    );
    Ok(())
}

#[test]
fn material_observer_stencil_stops_at_diffuse_winner_and_retains_no_unused_tint() -> TestResult {
    let xml = b"<Material Shader='MeshDecal' StringGenMask='STENCIL_MAP'><Textures><Texture Map='TexSlot1' File='synthetic/own_diff.tif'/><Texture Map='TexSlot7' File='synthetic/stencil.tif'/></Textures><PublicParams StencilTintOverride='2'/></Material>";
    let (library, events) = support::capture(xml, MaterialSourceLimits::default())?;
    assert_eq!(
        library.materials.first().ok_or("material")?.palette,
        None,
        "the material's own diffuse map excludes stencil tinting"
    );
    assert!(
        support::palette(&events)
            .any(|event| event.indices == [0, 0] && event.source_field == "File"),
        "the selected diffuse input contributes"
    );
    assert!(
        !support::palette(&events).any(|event| event.indices == [0, 1]
            || matches!(
                event.source_field.as_str(),
                "PublicParams" | "StencilTintOverride"
            )),
        "later stencil and parameter inputs are unused"
    );
    Ok(())
}

#[test]
fn material_observer_stencil_invalid_first_override_retains_default_selector() -> TestResult {
    let xml = b"<Material Shader='MeshDecal' StringGenMask='STENCIL_MAP'><Textures><Texture Map='TexSlot7' File='synthetic/stencil.tif'/></Textures><PublicParams StencilTintOverride='NaN'/><PublicParams StencilTintOverride='2'/></Material>";
    let (library, events) = support::capture(xml, MaterialSourceLimits::default())?;
    assert_eq!(
        library.materials.first().ok_or("material")?.palette,
        None,
        "an invalid first declaration remains the existing no-tint result"
    );
    assert!(
        support::palette(&events).any(|event| event.indices == [1]
            && event.source_field == "StencilTintOverride"
            && event.use_kind == MaterialReadUse::Selected),
        "the original invalid first parameter selects its default"
    );
    assert!(
        !support::palette(&events).any(|event| event.indices == [2]),
        "the later valid duplicate is never substituted"
    );
    Ok(())
}

#[test]
fn material_observer_stencil_distinguishes_missing_texture_map_from_explicit_empty_map()
-> TestResult {
    for (map, expected) in [
        ("", MaterialReadUse::Selected),
        ("Map=''", MaterialReadUse::Interpreted),
    ] {
        let xml = format!(
            "<Material Shader='MeshDecal' StringGenMask='STENCIL_MAP'><Textures><Texture {map} File='synthetic/motif.tif'/></Textures></Material>"
        );
        let (library, events) = support::capture(xml.as_bytes(), MaterialSourceLimits::default())?;
        assert_eq!(
            library.materials.first().ok_or("material")?.palette,
            None,
            "both original branches preserve their no-stencil result"
        );
        let maps: Vec<_> = support::palette(&events)
            .filter(|event| event.source_field == "Map")
            .collect();
        assert!(
            !maps.is_empty(),
            "the actual slot classification consults Map"
        );
        for event in maps {
            assert_eq!(
                event.indices,
                [0, 0],
                "the query keeps its original Texture site"
            );
            assert_eq!(
                event.use_kind, expected,
                "an absent Map selects its default while explicit empty text is interpreted"
            );
        }
    }
    Ok(())
}
