// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::manifest::{Issue, IssueOrigin, Manifest};

const KIND: &str = "value_not_finite";

const fn clean(value: &mut f32, fallback: f32) -> bool {
    let bad = !value.is_finite();
    if bad {
        *value = fallback;
    }
    bad
}

fn clean_all(values: &mut [f32], fallback: &[f32]) -> bool {
    let bad = values.iter().any(|value| !value.is_finite());
    if bad {
        values.copy_from_slice(fallback);
    }
    bad
}

fn clean_each<const N: usize>(values: &mut [f32; N]) -> bool {
    values
        .iter_mut()
        .fold(false, |bad, value| clean(value, 0.0) | bad)
}

fn clean_option<const N: usize>(values: &mut Option<[f32; N]>) -> bool {
    let bad = values.is_some_and(|array| array.iter().any(|value| !value.is_finite()));
    if bad {
        *values = None;
    }
    bad
}

fn issue(path: &str, name: &str, what: &str) -> Issue {
    Issue {
        origin: IssueOrigin::Assembly,
        kind: KIND.to_owned(),
        path: path.to_owned(),
        name: name.to_owned(),
        skipped: false,
        reason: format!("{what} is not a finite number in the source data and was replaced"),
    }
}

/// Replaces every number of `manifest` that is NaN or infinite, which JSON cannot hold, and lists
/// each replacement among the issues.
///
/// Only the non-finite part of a node transform (translation, rotation or scale) becomes the
/// identity. Colours and factors become 0, except `opacity`, which becomes 1; optional values are
/// left out.
pub(crate) fn sanitise(manifest: &mut Manifest) {
    let mut found = Vec::new();
    for node in &mut manifest.nodes {
        let bad = clean_all(&mut node.translation, &[0.0; 3])
            | clean_all(&mut node.rotation, &[0.0, 0.0, 0.0, 1.0])
            | clean_all(&mut node.scale, &[1.0; 3]);
        if bad {
            found.push(issue(&node.path, &node.name, "the transform of the node"));
        }
    }
    materials(manifest, &mut found);
    palettes(manifest, &mut found);
    clips(manifest, &mut found);
    manifest.issues.extend(found);
}

fn materials(manifest: &mut Manifest, found: &mut Vec<Issue>) {
    for material in &mut manifest.materials {
        let mut bad = clean_each(&mut material.diffuse)
            | clean_each(&mut material.specular)
            | clean_each(&mut material.emissive);
        bad |= clean(&mut material.opacity, 1.0);
        for value in [
            &mut material.shininess,
            &mut material.alpha_test,
            &mut material.glow,
        ] {
            bad |= clean(value, 0.0);
        }
        for layer in &mut material.layers {
            bad |= clean_option(&mut layer.tint_colour)
                | clean_option(&mut layer.wear_tint)
                | clean_option_value(&mut layer.uv_tiling)
                | clean_option_value(&mut layer.gloss_mult);
        }
        if bad {
            found.push(issue(
                &material.library,
                &material.name,
                "a value of the material",
            ));
        }
    }
}

fn palettes(manifest: &mut Manifest, found: &mut Vec<Issue>) {
    for palette in &mut manifest.palettes {
        let mut bad = clean_each(&mut palette.glass);
        if let Some(decal) = &mut palette.decal {
            bad |= decal
                .iter_mut()
                .fold(false, |bad, colour| clean_each(colour) | bad);
        }
        for entry in [
            &mut palette.primary,
            &mut palette.secondary,
            &mut palette.tertiary,
        ] {
            bad |= clean_each(&mut entry.tint)
                | clean_each(&mut entry.specular)
                | clean(&mut entry.glossiness, 0.0);
        }
        if bad {
            found.push(issue("", &palette.name, "a colour of the palette"));
        }
    }
}

fn clips(manifest: &mut Manifest, found: &mut Vec<Issue>) {
    for mesh in &mut manifest.meshes {
        let Some(rig) = &mut mesh.rig else {
            continue;
        };
        for clip in &mut rig.animations {
            if clean(&mut clip.duration_seconds, 0.0) {
                found.push(issue("", &clip.name, "the length of the animation"));
            }
        }
    }
}

fn clean_option_value(value: &mut Option<f32>) -> bool {
    let bad = value.is_some_and(|number| !number.is_finite());
    if bad {
        *value = None;
    }
    bad
}

#[cfg(test)]
mod tests {
    use super::{clean_all, clean_each, clean_option, sanitise};
    use crate::build::{coordinates, generator};
    use crate::manifest::{
        EntityRef, FORMAT_VERSION, Format, IssueOrigin, Manifest, Node, NodeKind, Options, Source,
    };

    fn node(translation: [f32; 3]) -> Node {
        Node {
            kind: NodeKind::Entity,
            name: "Gun".to_owned(),
            path: "hardpoint/Gun".to_owned(),
            parent: None,
            entity: None,
            light: None,
            mesh: None,
            translation,
            rotation: [0.0, 0.0, 0.0, 1.0],
            scale: [1.0; 3],
        }
    }

    fn manifest(nodes: Vec<Node>) -> Manifest {
        Manifest {
            format: Format::P4kxBlenderPackage,
            version: FORMAT_VERSION.to_owned(),
            generator: generator(),
            source: Source {
                channel: None,
                build: None,
                entity: EntityRef {
                    class_name: String::new(),
                    guid: String::new(),
                },
                character: None,
            },
            coordinates: coordinates(),
            options: Options {
                lod: 0,
                texture_mip: 0,
                vertex_colours: true,
                exclude: Vec::new(),
                replace: Vec::new(),
            },
            nodes,
            meshes: Vec::new(),
            textures: Vec::new(),
            materials: Vec::new(),
            palettes: Vec::new(),
            issues: Vec::new(),
        }
    }

    #[test]
    fn a_node_with_a_nan_becomes_an_issue_and_the_manifest_serialises_without_null() {
        let mut manifest = manifest(vec![node([1.0, f32::NAN, 3.0]), node([1.0, 2.0, 3.0])]);

        sanitise(&mut manifest);

        assert_eq!(manifest.nodes[0].translation, [0.0; 3]);
        assert_eq!(manifest.nodes[1].translation, [1.0, 2.0, 3.0]);
        assert_eq!(manifest.issues.len(), 1);
        assert_eq!(manifest.issues[0].origin, IssueOrigin::Assembly);
        assert_eq!(manifest.issues[0].kind, "value_not_finite");
        assert_eq!(manifest.issues[0].path, "hardpoint/Gun");
        assert!(!manifest.issues[0].skipped);
        let json = serde_json::to_string(&manifest).unwrap_or_default();
        assert!(!json.contains("null"), "{json}");
    }

    #[test]
    fn a_transform_with_a_nan_becomes_the_identity() {
        let mut rotation = [0.0, f32::NAN, 0.0, 1.0];

        assert!(clean_all(&mut rotation, &[0.0, 0.0, 0.0, 1.0]));
        assert_eq!(rotation, [0.0, 0.0, 0.0, 1.0]);
    }

    #[test]
    fn finite_values_are_left_alone() {
        let mut colour = [0.1, 0.2, 0.3];
        let mut optional = Some([1.0, 2.0, 3.0]);

        assert!(!clean_each(&mut colour));
        assert!(!clean_option(&mut optional));
        assert_eq!(colour, [0.1, 0.2, 0.3]);
        assert_eq!(optional, Some([1.0, 2.0, 3.0]));
    }

    #[test]
    fn a_colour_with_an_infinity_is_zeroed_and_an_optional_one_dropped() {
        let mut colour = [f32::INFINITY, 0.5, 0.5];
        let mut optional = Some([0.0, f32::NAN, 0.0]);

        assert!(clean_each(&mut colour));
        assert!(clean_option(&mut optional));
        assert_eq!(colour, [0.0, 0.5, 0.5]);
        assert_eq!(optional, None);
    }
}
