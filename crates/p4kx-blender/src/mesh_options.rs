// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_gltf::GlbOptions;

/// Returns the GLB options of a package mesh: `options` with the vertex colours of layered
/// surfaces, which the add-on reads as the shares of their layers. Every writer of a mesh file
/// takes its options from here; vertex colours switched off stay off.
pub(crate) const fn package_mesh_options(options: &GlbOptions) -> GlbOptions {
    options.with_layer_vertex_colours()
}

#[cfg(test)]
mod tests {
    use p4kx_gltf::{GlbOptions, VertexColours};

    use super::package_mesh_options;

    #[test]
    fn a_package_mesh_carries_the_layer_shares_unless_colours_are_off() {
        let on = package_mesh_options(&GlbOptions::default());
        assert_eq!(on.vertex_colours, VertexColours::TintsAndShares);
        let off = package_mesh_options(&GlbOptions::default().without_vertex_colours());
        assert_eq!(off.vertex_colours, VertexColours::Off);
    }
}
