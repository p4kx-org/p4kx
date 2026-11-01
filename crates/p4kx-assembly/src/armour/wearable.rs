// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_dataforge::Guid;

use super::{BodyVariant, ZoneChunk, ZoneList};
use crate::node_geometry::select_variant;
use crate::spec::EntitySpec;
use crate::{Geometry, GeometryVariant, MaterialVariant};

/// Why a piece has no geometry for a body variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Unfit {
    /// The Entity's Record names no geometry at all.
    NoGeometry,
    /// The Record names geometry per body, but none for this variant.
    NoVariant,
}

/// The geometry worn on a body and whether it is the Record's main geometry, not a sub-geometry.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Worn<'a> {
    pub(crate) geometry: &'a Geometry,
    pub(crate) main: bool,
}

/// An Entity worn on a body, with the geometry its Record names: the main geometry and the
/// sub-geometry per body tag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Wearable {
    pub(crate) guid: Guid,
    pub(crate) record_path: String,
    pub(crate) class_name: String,
    pub(crate) item_type: String,
    pub(crate) geometry: Option<Geometry>,
    pub(crate) variants: Vec<GeometryVariant>,
    pub(crate) tags: Vec<String>,
    pub(crate) material_variants: Vec<MaterialVariant>,
    pub(crate) zones: ZoneList,
}

impl Wearable {
    pub(crate) fn from_spec(spec: &EntitySpec) -> Self {
        Self {
            guid: spec.guid,
            record_path: spec.record_path.clone(),
            class_name: crate::entities::class_name_of(&spec.name)
                .unwrap_or(&spec.name)
                .to_owned(),
            item_type: spec.attach_type.clone(),
            geometry: spec.geometry.clone(),
            variants: spec.variants.clone(),
            tags: spec.attach_tags.clone(),
            material_variants: spec.material_variants.clone(),
            zones: spec.zones.clone(),
        }
    }

    /// Returns the zones of the body the Entity covers when it is worn, from the Record's
    /// `SCItemClothingParams`.
    #[must_use]
    pub fn zones(&self) -> &[ZoneChunk] {
        &self.zones.chunks
    }

    /// Returns the names of the ports, in lower case, whose items are not drawn when the Entity is
    /// worn, from the `HiddenParts` of the Record's `SCItemClothingParams`: a hat hides the hair.
    #[must_use]
    pub fn hidden_ports(&self) -> &[String] {
        &self.zones.hidden_ports
    }

    /// Returns the tags of the Entity's `AttachDef`, e.g. `hairVar_Brown` for a hair colour.
    #[must_use]
    pub fn tags(&self) -> &[String] {
        &self.tags
    }

    /// Returns the material libraries the Record chooses by tags.
    #[must_use]
    pub fn material_variants(&self) -> &[MaterialVariant] {
        &self.material_variants
    }

    /// Returns the geometry worn on `body` like [`Self::geometry_for`], but a sub-geometry whose
    /// tags all are among `selecting` comes first: a port's flags select the sub-geometry of the
    /// item attached to it, as in the Entity tree. A flag that covers only some tags of a
    /// sub-geometry, e.g. the head's flag next to `Male` / `Female`, leaves the body to decide.
    pub(crate) fn geometry_selected(
        &self,
        body: BodyVariant,
        selecting: &[String],
    ) -> Result<Worn<'_>, Unfit> {
        select_variant(&self.variants, selecting).map_or_else(
            || self.geometry_for(body),
            |found| {
                Ok(Worn {
                    geometry: &found.geometry,
                    main: false,
                })
            },
        )
    }

    /// Returns the GUID of the Entity's Record.
    #[must_use]
    pub const fn guid(&self) -> Guid {
        self.guid
    }

    /// Returns the record path of the Entity.
    #[must_use]
    pub fn record_path(&self) -> &str {
        &self.record_path
    }

    /// Returns the class name of the Entity.
    #[must_use]
    pub fn class_name(&self) -> &str {
        &self.class_name
    }

    /// Returns the Entity's `AttachDef.Type`, e.g. `Char_Armor_Torso`; empty when it has none.
    #[must_use]
    pub fn item_type(&self) -> &str {
        &self.item_type
    }

    /// Returns the main geometry of the Record, which is not the one worn on a body.
    #[must_use]
    pub const fn geometry(&self) -> Option<&Geometry> {
        self.geometry.as_ref()
    }

    /// Returns the sub-geometry of the Record with its tags.
    #[must_use]
    pub fn variants(&self) -> &[GeometryVariant] {
        &self.variants
    }

    /// Returns the geometry worn on `body`: the sub-geometry tagged with the variant's tag, else
    /// the main geometry when the Record names no geometry per body at all.
    pub(crate) fn geometry_for(&self, body: BodyVariant) -> Result<Worn<'_>, Unfit> {
        if let Some(found) = self.variants.iter().find(|found| found.has_tag(body.tag())) {
            return Ok(Worn {
                geometry: &found.geometry,
                main: false,
            });
        }
        let per_body = self.variants.iter().any(|found| {
            BodyVariant::ALL
                .iter()
                .any(|other| found.has_tag(other.tag()))
        });
        if per_body {
            return Err(Unfit::NoVariant);
        }
        self.geometry
            .as_ref()
            .map(|geometry| Worn {
                geometry,
                main: true,
            })
            .ok_or(Unfit::NoGeometry)
    }
}

#[cfg(test)]
mod tests {
    use p4kx_dataforge::Guid;

    use super::{BodyVariant, Wearable};
    use crate::{Geometry, GeometryVariant};

    const MALE_TEETH: &str = "heads/male/teeth.skin";
    const FEMALE_TEETH: &str = "heads/female/teeth.skin";

    fn variant(tags: &[&str], path: &str) -> GeometryVariant {
        GeometryVariant {
            tags: tags.iter().map(|tag| (*tag).to_owned()).collect(),
            geometry: Geometry {
                path: path.to_owned(),
                material: None,
                palette: None,
            },
        }
    }

    fn teeth() -> Wearable {
        Wearable {
            guid: Guid::from_stored([0; 16]),
            record_path: String::new(),
            class_name: "Head_Teeth".to_owned(),
            item_type: "Char_Head_Teeth".to_owned(),
            geometry: None,
            variants: vec![
                variant(&["Protos_Head", "Female"], FEMALE_TEETH),
                variant(&["Protos_Head", "Male"], MALE_TEETH),
                variant(&["other_head"], "heads/other/teeth.skin"),
            ],
            tags: Vec::new(),
            material_variants: Vec::new(),
            zones: super::ZoneList::default(),
        }
    }

    fn selected(body: BodyVariant, flags: &[&str]) -> Option<String> {
        let flags: Vec<String> = flags.iter().map(|flag| (*flag).to_owned()).collect();
        teeth()
            .geometry_selected(body, &flags)
            .ok()
            .map(|worn| worn.geometry.path.clone())
    }

    #[test]
    fn a_flag_that_covers_only_part_of_the_tags_leaves_the_choice_to_the_body() {
        assert_eq!(
            selected(BodyVariant::Male, &["Protos_Head"]).as_deref(),
            Some(MALE_TEETH)
        );
        assert_eq!(
            selected(BodyVariant::Female, &["Protos_Head"]).as_deref(),
            Some(FEMALE_TEETH)
        );
    }

    #[test]
    fn flags_that_cover_every_tag_of_a_sub_geometry_choose_it() {
        assert_eq!(
            selected(BodyVariant::Male, &["Protos_Head", "Female"]).as_deref(),
            Some(FEMALE_TEETH)
        );
        assert_eq!(
            selected(BodyVariant::Male, &["other_head"]).as_deref(),
            Some("heads/other/teeth.skin")
        );
    }
}
