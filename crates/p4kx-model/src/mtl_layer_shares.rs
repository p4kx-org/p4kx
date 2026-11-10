// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The one rule for the share each layer of a material has without a blend mask.

use crate::Material;

impl Material {
    /// Returns the share by position of each layer of the material, in layer order, or `None`
    /// when the material is shaded by its own colours.
    ///
    /// The first layer weighs `1 - WearBlendBase`, the others `WearBlendBase` between them; a
    /// layer for which `resolved` is false (its file is missing) has no weight, and the shares of
    /// the others are normalised to sum to 1. When no resolved layer has weight, the first layer
    /// takes everything if it is resolved, else the material keeps its own colours.
    ///
    /// The GLB writer shades from it and the package states it as `layer_shares`.
    #[must_use]
    pub fn layer_shares(&self, resolved: impl Fn(usize) -> bool) -> Option<Vec<f32>> {
        let count = self.layers.len();
        let wear = self.wear_blend().clamp(0.0, 1.0);
        #[expect(
            clippy::cast_precision_loss,
            reason = "layer shares use approximate f32 weights"
        )]
        let others = count.saturating_sub(1).max(1) as f32;
        let weight = |at: usize| if at == 0 { 1.0 - wear } else { wear / others };
        let first = (0..count).find(|&at| resolved(at))?;
        let total: f32 = (0..count).filter(|&at| resolved(at)).map(weight).sum();
        let only = total <= f32::EPSILON;
        if only && first != 0 {
            return None;
        }
        Some(
            (0..count)
                .map(|at| match (resolved(at), only) {
                    (false, _) => 0.0,
                    (true, true) => f32::from(u8::from(at == first)),
                    (true, false) => weight(at) / total,
                })
                .collect(),
        )
    }
}

#[cfg(test)]
mod tests {
    use std::fmt::Write as _;

    use crate::MaterialLibrary;

    type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

    fn shares(wear: &str, layers: usize, resolved: &[usize]) -> Result<Option<Vec<f32>>> {
        let mut rows = String::new();
        for at in 0..layers {
            write!(rows, r#"<Layer Name="l{at}" Path="a/l{at}.mtl"/>"#)?;
        }
        let xml = format!(
            r#"<Material><SubMaterials><Material Name="m" Shader="HardSurface">
            <PublicParams WearBlendBase="{wear}"/><MatLayers>{rows}</MatLayers></Material></SubMaterials></Material>"#
        );
        let library = MaterialLibrary::parse("a", xml.as_bytes())?;
        let material = library.materials.first().ok_or("a material")?;
        Ok(material.layer_shares(|at| resolved.contains(&at)))
    }

    fn assert_shares(found: Option<Vec<f32>>, expected: &[f32]) {
        let found = found.expect("shares");
        assert_eq!(found.len(), expected.len());
        for (found, expected) in found.iter().zip(expected) {
            assert!((found - expected).abs() < 1e-6, "{found} vs {expected}");
        }
    }

    #[test]
    fn the_first_layer_takes_the_rest_of_the_wear_share() -> Result {
        assert_shares(shares("0.25", 2, &[0, 1])?, &[0.75, 0.25]);
        assert_shares(shares("0", 2, &[0, 1])?, &[1.0, 0.0]);
        Ok(())
    }

    #[test]
    fn a_missing_layer_has_no_weight_and_the_rest_are_renormalised() -> Result {
        assert_shares(shares("0.4", 3, &[0, 2])?, &[0.75, 0.0, 0.25]);
        Ok(())
    }

    #[test]
    fn without_any_weight_the_first_layer_takes_everything_if_it_exists() -> Result {
        assert_shares(shares("1", 1, &[0])?, &[1.0]);
        assert_shares(shares("1", 2, &[0])?, &[1.0, 0.0]);
        Ok(())
    }

    #[test]
    fn all_layer_positions_keep_their_shares_beyond64() -> Result {
        const LAYERS: usize = 65;
        let resolved: Vec<_> = (0..LAYERS).collect();
        let mut expected = vec![1.0 / 128.0; LAYERS];
        *expected.first_mut().ok_or("first expected layer")? = 0.5;
        assert_shares(shares("0.5", LAYERS, &resolved)?, &expected);
        Ok(())
    }

    #[test]
    fn without_a_first_layer_or_any_weight_the_material_keeps_its_colours() -> Result {
        assert_eq!(shares("1", 2, &[])?, None);
        assert_eq!(shares("0", 2, &[1])?, None);
        assert_shares(shares("1", 2, &[1])?, &[0.0, 1.0]);
        Ok(())
    }
}
