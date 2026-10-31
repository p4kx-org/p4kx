// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_dataforge::{Guid, StructValue, Value, ValueError};

use crate::character_path::is_rigged_path_outside_human_rigs;
use crate::read::{struct_field, struct_items, text};

/// The deepest nesting of sub-geometry nodes that is read.
const MAX_NESTED_SUB_GEOMETRY: usize = 4;

/// The tint palette an Entity's geometry names (`Palette.TintPaletteRef`): the `TintPaletteTree`
/// Record and the child of its tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaletteRef {
    pub(crate) guid: Guid,
    pub(crate) child_path: String,
}

impl PaletteRef {
    /// Returns the GUID of the `TintPaletteTree` Record.
    #[must_use]
    pub const fn guid(&self) -> Guid {
        self.guid
    }

    /// Returns the path of the child palette below the Record's root, or an empty string for the
    /// root.
    #[must_use]
    pub fn child_path(&self) -> &str {
        &self.child_path
    }
}

/// The geometry an Entity Record names (`SGeometryResourceParams`): the model file, the material
/// library that replaces the library the model names itself and the tint palette.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Geometry {
    pub(crate) path: String,
    pub(crate) material: Option<String>,
    pub(crate) palette: Option<PaletteRef>,
}

/// A geometry an Entity Record names for the tags of a sub-node (`SubGeometry`), e.g. `Male` or
/// `Female` for the body a piece of armour is worn on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeometryVariant {
    pub(crate) tags: Vec<String>,
    pub(crate) geometry: Geometry,
}

impl GeometryVariant {
    /// Returns the tags of the sub-node, split at white space.
    #[must_use]
    pub fn tags(&self) -> &[String] {
        &self.tags
    }

    /// Returns the geometry of the sub-node.
    #[must_use]
    pub const fn geometry(&self) -> &Geometry {
        &self.geometry
    }

    /// Returns whether the sub-node carries the tag `tag`, ignoring ASCII case.
    #[must_use]
    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|own| own.eq_ignore_ascii_case(tag))
    }
}

/// The deepest nesting of material variant nodes that is read.
const MAX_MATERIAL_DEPTH: usize = 6;

/// The most material variant nodes one Entity Record contributes.
const MAX_MATERIAL_NODES: usize = 4096;

/// A material library an Entity Record chooses for the tags of a node of its material tree
/// (`SMaterialNodeParams`), e.g. `hairVar_Brown` and below it `Male`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaterialVariant {
    pub(crate) tags: Vec<String>,
    pub(crate) material: Option<String>,
    pub(crate) children: Vec<Self>,
}

impl MaterialVariant {
    /// Returns the tags of the node, split at white space.
    #[must_use]
    pub fn tags(&self) -> &[String] {
        &self.tags
    }

    /// Returns the material library of the node, a path below `Data`.
    #[must_use]
    pub fn material(&self) -> Option<&str> {
        self.material.as_deref()
    }

    /// Returns the nodes nested in this one.
    #[must_use]
    pub fn children(&self) -> &[Self] {
        &self.children
    }

    /// Returns the material library of the deepest chain of nodes whose tags are all in `active`,
    /// ignoring ASCII case; a node without tags never matches.
    #[must_use]
    pub fn resolve<'v>(variants: &'v [Self], active: &[String]) -> Option<&'v str> {
        variants.iter().find_map(|node| {
            let matches = !node.tags.is_empty()
                && node
                    .tags
                    .iter()
                    .all(|tag| active.iter().any(|own| own.eq_ignore_ascii_case(tag)));
            if !matches {
                return None;
            }
            Self::resolve(&node.children, active).or(node.material.as_deref())
        })
    }

    /// Reads the variants below the material node of a `SGeometryResourceParams` component.
    pub(crate) fn read(component: &StructValue<'_>) -> Vec<Self> {
        let Some(node) = struct_field(component, "Material") else {
            return Vec::new();
        };
        let mut budget = MAX_MATERIAL_NODES;
        Self::read_children(node, 0, &mut budget)
    }

    fn read_children(node: &StructValue<'_>, depth: usize, budget: &mut usize) -> Vec<Self> {
        if depth >= MAX_MATERIAL_DEPTH {
            return Vec::new();
        }
        let mut found = Vec::new();
        for child in struct_items(node, "materialVariants") {
            if *budget == 0 {
                break;
            }
            *budget -= 1;
            let material = struct_field(child, "Material")
                .map(|resource| text(resource, "path"))
                .filter(|path| !path.is_empty())
                .map(str::to_owned);
            found.push(Self {
                tags: text(child, "Tags")
                    .split_whitespace()
                    .map(str::to_owned)
                    .collect(),
                material,
                children: Self::read_children(child, depth + 1, budget),
            });
        }
        found
    }
}

impl Geometry {
    /// Returns the model file as the Record spells it, a path below `Data`, e.g.
    /// `Objects\Spaceships\Ships\AEGS\Gladius\AEGS_Gladius.cga`.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Returns the material library the Entity chooses, a path below `Data`, or `None` when the
    /// Record names none and the model's own library applies.
    #[must_use]
    pub fn material(&self) -> Option<&str> {
        self.material.as_deref()
    }

    /// Returns the tint palette the Entity chooses, or `None` when the Record names none.
    #[must_use]
    pub const fn palette(&self) -> Option<&PaletteRef> {
        self.palette.as_ref()
    }

    /// Returns whether the model file is a `.cdf`, `.chr` or `.skin` file outside
    /// `Objects/Characters/Human`, the folder of the human body rigs.
    #[must_use]
    pub fn is_rigged_file_outside_human_rigs(&self) -> bool {
        is_rigged_path_outside_human_rigs(&self.path)
    }

    /// Reads the main geometry of a `SGeometryResourceParams` component.
    pub(crate) fn read(component: &StructValue<'_>) -> Option<Self> {
        Self::of_node(struct_field(component, "Geometry")?)
    }

    /// Reads the geometry of the sub-nodes of the main node of a `SGeometryResourceParams`
    /// component that name a model, with their tags: the children of the main node first, then
    /// the sub-nodes nested in them, which carry the tags of the nodes they are nested in as well
    /// as their own.
    pub(crate) fn read_variants(component: &StructValue<'_>) -> Vec<GeometryVariant> {
        let Some(node) = struct_field(component, "Geometry") else {
            return Vec::new();
        };
        let mut variants = Vec::new();
        let mut level = vec![(node, Vec::<String>::new())];
        for _ in 0..MAX_NESTED_SUB_GEOMETRY {
            let mut next = Vec::new();
            for (parent, inherited) in &level {
                for sub in struct_items(parent, "SubGeometry") {
                    let mut tags = inherited.clone();
                    tags.extend(text(sub, "Tags").split_whitespace().map(str::to_owned));
                    if let Some(geometry) = Self::of_node(sub) {
                        variants.push(GeometryVariant {
                            tags: tags.clone(),
                            geometry,
                        });
                    }
                    next.push((sub, tags));
                }
            }
            if next.is_empty() {
                break;
            }
            level = next;
        }
        variants
    }

    fn of_node(node: &StructValue<'_>) -> Option<Self> {
        let data = struct_field(node, "Geometry")?;
        let path = struct_field(data, "Geometry").map(|resource| text(resource, "path"))?;
        if path.is_empty() {
            return None;
        }
        let material = struct_field(data, "Material")
            .map(|resource| text(resource, "path"))
            .filter(|path| !path.is_empty());
        Some(Self {
            path: path.to_owned(),
            material: material.map(str::to_owned),
            palette: read_palette(data),
        })
    }
}

fn read_palette(data: &StructValue<'_>) -> Option<PaletteRef> {
    let reference = struct_field(data, "Palette")?;
    let guid = match reference.field("RootRecord")? {
        Value::Reference(link) => link.guid(),
        Value::Error(ValueError::DanglingReference { guid }) => *guid,
        _ => return None,
    };
    Some(PaletteRef {
        guid,
        child_path: text(reference, "ChildPath").trim().to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::Geometry;
    use crate::BodyVariant;

    fn geometry(path: &str) -> Geometry {
        Geometry {
            path: path.to_owned(),
            material: None,
            palette: None,
        }
    }

    #[test]
    fn the_human_skeleton_and_skin_are_no_rigged_files_outside_the_human_rigs() {
        for body in BodyVariant::ALL {
            assert!(!geometry(body.skeleton()).is_rigged_file_outside_human_rigs());
            assert!(!geometry(body.skin()).is_rigged_file_outside_human_rigs());
        }
    }

    #[test]
    fn character_files_outside_the_human_rigs_are_told_by_extension_and_folder() {
        for path in [
            "Objects/Characters/Creatures/x/x_skel.chr",
            "objects\\characters\\creatures\\x\\x.CDF",
            "/Objects/Characters/Vanduul/drone/body.skin",
            "Objects/Spaceships/Thrusters/nozzle.cdf",
        ] {
            assert!(geometry(path).is_rigged_file_outside_human_rigs(), "{path}");
        }
        for path in [
            "Data/Objects/Characters/Human/male_v7/export/bhm_skeleton_v7.chr",
            "Objects/Characters/Human/heads/male/head.skin",
            "Objects/Spaceships/Ships/AEGS/Gladius/gladius.cga",
            "Objects/Characters/Creatures/x/x.cgf",
            "Objects/Characters/Creatures/x/skin",
        ] {
            assert!(
                !geometry(path).is_rigged_file_outside_human_rigs(),
                "{path}"
            );
        }
    }
}
