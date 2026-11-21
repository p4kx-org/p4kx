// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_dataforge::{StructValue, TagTree};

use crate::cdf::is_character_definition;
use crate::geometry_choice::GeometryChoice;
use crate::paint_port::PaintOfferSources;
use crate::paint_provenance::{PaintSelection, VariantSource};
use crate::read;
use crate::spec::{EntitySpec, VehicleSpec};
use crate::{
    Attachment, EntityNode, Geometry, IssueKind, LoadoutResolver, MaterialVariant, PaintPort,
    PaintTagIndex, PortNode, PortSource,
};

#[path = "vehicle_paint_sources.rs"]
mod sources;

pub(crate) use sources::record_header;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PaintItem {
    tags: Box<str>,
    required_tags: Box<str>,
}

impl PaintItem {
    pub(crate) fn read(definition: &StructValue<'_>) -> Self {
        Self {
            tags: read::text(definition, "Tags").into(),
            required_tags: read::text(definition, "RequiredTags").into(),
        }
    }

    fn selector(&self) -> Result<&str, &'static str> {
        let mut selectors = self
            .tags
            .split_whitespace()
            .filter_map(|word| word.strip_prefix('@'))
            .filter(|word| !word.is_empty());
        let first = selectors
            .next()
            .ok_or("the fitted paint names no appearance tag")?;
        if selectors.any(|candidate| !candidate.eq_ignore_ascii_case(first)) {
            return Err("the fitted paint names several appearance tags");
        }
        Ok(first)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum PaintAppearance {
    Geometry {
        geometry: Geometry,
        selection: Arc<PaintSelection>,
    },
    Issue(IssueKind),
}

pub(crate) const fn is_paint_type(kind: &str) -> bool {
    kind.eq_ignore_ascii_case("Paints")
}

impl LoadoutResolver<'_> {
    /// Prepares the root vehicle's fitted paint for model assembly.
    #[must_use]
    pub const fn with_vehicle_appearance(mut self) -> Self {
        self.vehicle_appearance = true;
        self
    }

    pub(crate) const fn captures_vehicle_appearance(&self) -> bool {
        self.vehicle_appearance && self.observation.is_none()
    }

    pub(crate) fn prepare_vehicle_paint(&self, spec: &EntitySpec, root: &mut EntityNode) {
        if self.captures_vehicle_appearance()
            && let Some(appearance) = self.vehicle_paint(spec, root)
        {
            root.paint_appearance = Some(Box::new(appearance));
        }
    }

    fn vehicle_paint(&self, spec: &EntitySpec, root: &EntityNode) -> Option<PaintAppearance> {
        let vehicle = spec.vehicle.as_ref()?;
        let mut fitted = root.ports().iter().filter_map(|port| equipped(root, port));
        let (port, item, tags) = fitted.next()?;
        let outcome = if fitted.next().is_some() {
            Err("several typed paint ports hold items")
        } else {
            item.classification()
                .paint()
                .ok_or("the fitted item is not a paint")
                .and_then(|paint| {
                    let offers = if port.source() == PortSource::Replaced {
                        Some(
                            self.replacement_offers(paint, &tags, &vehicle.attach_tags)
                                .ok_or("the replacement paint does not satisfy the port and vehicle tags")?,
                        )
                    } else {
                        None
                    };
                    let selector = paint.selector()?;
                    let appearance = choose_hull(spec, vehicle, selector)?;
                    Ok((
                        appearance.geometry,
                        Arc::new(PaintSelection {
                            guid: item.guid(),
                            record: item.record_index(),
                            attachable: item.attachable_component_ordinal(),
                            tag: selector.to_owned(),
                            variant: appearance.variant,
                            prefers_cdf_material: appearance.prefers_cdf_material,
                            compatibility: sources::selected(spec, root, port, item, paint, offers),
                        }),
                    ))
                })
        };
        Some(match outcome {
            Ok((geometry, selection)) => PaintAppearance::Geometry {
                geometry,
                selection,
            },
            Err(reason) => PaintAppearance::Issue(IssueKind::PaintAppearance {
                port: port.name().to_owned(),
                reason,
            }),
        })
    }

    fn replacement_offers(
        &self,
        item: &PaintItem,
        port: &PaintPort,
        vehicle_tags: &str,
    ) -> Option<PaintOfferSources> {
        let tree = self
            .tag_tree
            .get_or_init(|| TagTree::build(self.entities.forge()));
        let tag_index = PaintTagIndex::new(tree);
        port.accept_with_sources(
            &lowercase_tags(&item.tags),
            &lowercase_tags(&item.required_tags),
            &lowercase_tags(vehicle_tags),
            &tag_index,
        )
    }
}

fn lowercase_tags(tags: &str) -> Vec<String> {
    tags.split_whitespace()
        .map(str::to_ascii_lowercase)
        .collect()
}

fn equipped<'node>(
    root: &'node EntityNode,
    port: &'node PortNode,
) -> Option<(&'node PortNode, &'node EntityNode, PaintPort)> {
    let Attachment::Entity(item) = port.attachment() else {
        return None;
    };
    let xml = root.vehicle().and_then(|xml| xml.item_port(port.name()));
    let record = port.definition();
    let typed = xml.filter(|xml| !xml.types().is_empty()).map_or_else(
        || {
            record
                .is_some_and(|record| record.types().iter().any(|kind| is_paint_type(kind.name())))
        },
        |xml| xml.types().iter().any(|kind| is_paint_type(kind)),
    );
    typed.then(|| {
        let offered = xml
            .and_then(|xml| xml.port_tags_source().0)
            .or_else(|| record.map(crate::PortDefinition::port_tags));
        let required = xml
            .and_then(|xml| xml.required_tags_source().0)
            .or_else(|| record.map(crate::PortDefinition::required_tags));
        (port, item.as_ref(), PaintPort::new(offered, required))
    })
}

struct HullAppearance {
    geometry: Geometry,
    variant: VariantSource,
    prefers_cdf_material: bool,
}

fn choose_hull(
    spec: &EntitySpec,
    vehicle: &VehicleSpec,
    selector: &str,
) -> Result<HullAppearance, &'static str> {
    const AMBIGUOUS: &str = "the appearance tag selects more than one source variant";
    let model_variants = spec.variants.iter().map(|variant| {
        (
            variant.tags(),
            variant.geometry(),
            variant.geometry().variant_position(),
            variant.tag_locations.as_ref(),
        )
    });
    let appearance_only = vehicle.appearance_only.iter().map(|variant| {
        (
            variant.tags.as_slice(),
            &variant.geometry,
            variant.position.clone(),
            variant.tag_locations.as_ref(),
        )
    });
    let mut geometries = model_variants
        .chain(appearance_only)
        .filter(|(tags, _, _, _)| {
            !tags.is_empty() && tags.iter().all(|tag| tag.eq_ignore_ascii_case(selector))
        })
        .map(|(_, geometry, position, tag_locations)| (geometry, position, tag_locations));
    let geometry = geometries.next();
    let material = MaterialVariant::select_unique(&spec.material_variants, &[selector])
        .map_err(|_| AMBIGUOUS)?;
    if geometries.next().is_some() || (geometry.is_some() && material.is_some()) {
        return Err(AMBIGUOUS);
    }
    if let Some((geometry, position, tag_locations)) = geometry {
        let mut draw = geometry.clone();
        let mut prefers_cdf_material = false;
        if let Some(base) = &spec.geometry {
            if draw.material().is_none() {
                draw.inherit_material(base);
                prefers_cdf_material = is_character_definition(draw.path())
                    && draw.material().is_some()
                    && crate::build::file_key(draw.path()) != crate::build::file_key(base.path());
            }
            draw.inherit_palette(Some(base));
        }
        return Ok(HullAppearance {
            geometry: draw,
            variant: VariantSource::geometry(position, tag_locations.into()),
            prefers_cdf_material,
        });
    }
    if let Some(variant) = material {
        let mut draw = spec
            .geometry
            .clone()
            .ok_or("the material variant has no base geometry")?;
        draw.apply_material_variant(&variant);
        return Ok(HullAppearance {
            geometry: draw,
            variant: VariantSource::material(variant.location(), variant.tag_locations()),
            prefers_cdf_material: false,
        });
    }
    Err("the appearance tag selects no source variant")
}

impl EntityNode {
    pub(crate) fn painted_geometry(&self) -> Option<&Geometry> {
        match self.paint_appearance.as_deref()? {
            PaintAppearance::Geometry { geometry, .. } => Some(geometry),
            PaintAppearance::Issue(_) => None,
        }
    }

    pub(crate) fn rendered_paint(&self, drawn: Option<&Geometry>) -> Option<&Arc<PaintSelection>> {
        let PaintAppearance::Geometry {
            geometry,
            selection,
        } = self.paint_appearance.as_deref()?
        else {
            return None;
        };
        drawn
            .filter(|drawn| std::ptr::eq(*drawn, geometry))
            .map(|_| selection)
    }

    pub(crate) fn paint_issue(&self) -> Option<&IssueKind> {
        match self.paint_appearance.as_deref()? {
            PaintAppearance::Geometry { .. } => None,
            PaintAppearance::Issue(issue) => Some(issue),
        }
    }

    pub(crate) fn model_fallback(&self) -> Option<&Geometry> {
        if !matches!(self.choice, GeometryChoice::Main) || self.painted_geometry().is_none() {
            return self.fallback.as_ref();
        }
        self.geometry()
            .zip(self.painted_geometry())
            .filter(|(base, painted)| {
                crate::build::file_key(base.path()) != crate::build::file_key(painted.path())
            })
            .map(|(base, _)| base)
    }
}
