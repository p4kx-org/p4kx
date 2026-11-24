// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;
use std::sync::Arc;

use p4kx_dataforge::{StructValue, Value};
use p4kx_extraction::DataForgeOrigin;
use p4kx_model::Model;

use super::uncover::uncover;

use super::jobs::{Job, issue};
use super::{BodyVariant, PartOrigin};
use crate::read::{has_items, struct_items, text, unsigned, unsigned_list};
use crate::{Issue, IssueKind};

const CLOTHING: &str = "SCItemClothingParams";
const MAX_CHUNKS: usize = 256;

/// The layer the body itself is on; clothing is on the layers above.
const BODY_LAYER: u32 = 0;

/// One zone of the body an Entity covers when it is worn (`SCItemClothingMeshChunkParams`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZoneChunk {
    pub(crate) zone: String,
    pub(crate) layer: u32,
    pub(crate) visible_layers: Vec<u32>,
    pub(crate) conditional: bool,
    incomplete: bool,
    pub(super) position: Option<(usize, usize)>,
}

impl ZoneChunk {
    /// Returns the name of the zone, e.g. `torso01_zone`.
    #[must_use]
    pub fn zone(&self) -> &str {
        &self.zone
    }

    /// Returns the layer the Entity covers the zone on: 0 is the body, 1 the clothing next to it.
    #[must_use]
    pub const fn layer(&self) -> u32 {
        self.layer
    }

    /// Returns the layers below this one that stay visible through it.
    #[must_use]
    pub fn visible_layers(&self) -> &[u32] {
        &self.visible_layers
    }

    /// Returns whether the Record makes the zone depend on conditions that are not evaluated:
    /// ports that must hold items, or tags the items must or must not carry. A condition whose
    /// entries name no port and no tag restricts nothing and does not count.
    #[must_use]
    pub const fn conditional(&self) -> bool {
        self.conditional
    }

    fn read(chunk: &StructValue<'_>) -> Option<Self> {
        let zone = text(chunk, "MeshChunk").to_ascii_lowercase();
        if zone.is_empty() {
            return None;
        }
        let layer = unsigned(chunk, "Layer");
        let visible_layers = unsigned_list(chunk, "VisibleLayers");
        let incomplete = (layer.is_none() && chunk.field("Layer").is_some())
            || !complete_visible_layers(chunk.field("VisibleLayers"), visible_layers.len())
            || !complete_conditions(chunk);
        Some(Self {
            zone,
            layer: layer.unwrap_or(BODY_LAYER),
            visible_layers,
            conditional: restricted(chunk),
            incomplete,
            position: None,
        })
    }

    fn hides_body(&self) -> bool {
        self.layer > BODY_LAYER && !self.visible_layers.contains(&BODY_LAYER)
    }

    fn covers_body(&self) -> bool {
        self.hides_body() && !self.conditional
    }
}

/// The condition fields of `SCItemClothingPortVisibilityConditionParams` that restrict a chunk.
const RESTRICTIONS: [&str; 4] = ["Ports", "DisableTags", "SkipTags", "ActiveTags"];

/// Returns whether the `VisibilityConditions` of `chunk` restrict it. Only a condition whose
/// `PortVisibility` entries all name no port and no tag restricts nothing; one without entries is
/// not understood and counts as restricting.
fn restricted(chunk: &StructValue<'_>) -> bool {
    struct_items(chunk, "VisibilityConditions").any(|condition| {
        let mut entries = struct_items(condition, "PortVisibility").peekable();
        entries.peek().is_none()
            || entries.any(|entry| RESTRICTIONS.iter().any(|field| has_items(entry, field)))
    })
}

fn complete_visible_layers(field: Option<&Value<'_>>, decoded: usize) -> bool {
    match field {
        None => true,
        Some(Value::Array(items)) => items.len() == decoded,
        _ => false,
    }
}

fn complete_conditions(chunk: &StructValue<'_>) -> bool {
    complete_array(chunk.field("VisibilityConditions"), |value| {
        let Value::Struct(condition) = value else {
            return false;
        };
        complete_array(condition.field("PortVisibility"), |value| {
            let Value::Struct(entry) = value else {
                return false;
            };
            RESTRICTIONS.iter().all(|field| {
                complete_array(entry.field(field), |value| {
                    !matches!(value, Value::Error(_))
                })
            })
        })
    })
}

fn complete_array<'forge>(
    field: Option<&Value<'forge>>,
    valid: impl FnMut(&Value<'forge>) -> bool,
) -> bool {
    match field {
        None => true,
        Some(Value::Array(items)) => items.iter().all(valid),
        _ => false,
    }
}

/// What the `SCItemClothingParams` of an Entity say: the body zones it covers and the ports whose
/// items it hides.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct ZoneList {
    /// The zones, at most [`MAX_CHUNKS`] of them.
    pub(crate) chunks: Arc<[ZoneChunk]>,
    pub(crate) truncated: bool,
    pub(super) incomplete: bool,
    pub(crate) forge_origin: Option<Arc<DataForgeOrigin>>,
    /// The names of the ports, in lower case, whose items are not drawn when the Entity is worn.
    pub(crate) hidden_ports: Arc<[String]>,
}

impl ZoneList {
    pub(super) fn known_chunks(&self) -> impl Iterator<Item = &ZoneChunk> + Clone {
        self.chunks.iter().filter(|chunk| !chunk.incomplete)
    }
}

/// Reads the zones and hidden ports from the Record's clothing components.
pub(crate) fn read_zones<'value, 'forge: 'value>(
    components: impl Iterator<Item = (usize, &'value StructValue<'forge>)>,
    forge_origin: Option<&Arc<DataForgeOrigin>>,
) -> ZoneList {
    let components: Vec<_> = components
        .filter(|(_, component)| component.type_name() == CLOTHING)
        .collect();
    let clothing = || components.iter().map(|(_, component)| *component);
    let ReadChunks {
        chunks,
        truncated,
        incomplete,
    } = read_chunks(components.iter().copied());
    let hidden_ports = clothing()
        .flat_map(|component| struct_items(component, "HiddenParts"))
        .map(|part| text(part, "PortName").to_ascii_lowercase())
        .filter(|port| !port.is_empty())
        .collect();
    ZoneList {
        chunks: chunks.into(),
        truncated,
        incomplete,
        forge_origin: forge_origin.filter(|_| !components.is_empty()).cloned(),
        hidden_ports,
    }
}

#[derive(Default)]
struct ReadChunks {
    chunks: Vec<ZoneChunk>,
    truncated: bool,
    incomplete: bool,
}

fn read_chunks<'value, 'forge: 'value>(
    components: impl Iterator<Item = (usize, &'value StructValue<'forge>)>,
) -> ReadChunks {
    let mut read = ReadChunks::default();
    for (component_ordinal, component) in components {
        let Some(value) = component.field("Chunks") else {
            continue;
        };
        let Value::Array(items) = value else {
            read.incomplete = true;
            continue;
        };
        for (ordinal, item) in items.iter().enumerate() {
            let chunk = match item {
                Value::Struct(chunk) => ZoneChunk::read(chunk),
                _ => None,
            };
            let Some(mut chunk) = chunk else {
                read.incomplete = true;
                continue;
            };
            if read.chunks.len() == MAX_CHUNKS {
                read.truncated = true;
                return read;
            }
            read.incomplete |= chunk.incomplete;
            chunk.position = Some((component_ordinal, ordinal));
            read.chunks.push(chunk);
        }
    }
    read
}

/// Returns the zones of the body that the `chunks` of any worn Entity cover: those a chunk above
/// the body names without letting the body show through it.
pub(crate) fn covered<'c>(chunks: impl IntoIterator<Item = &'c ZoneChunk>) -> BTreeSet<&'c str> {
    chunks
        .into_iter()
        .filter(|chunk| chunk.covers_body())
        .map(|chunk| chunk.zone.as_str())
        .collect()
}

/// Returns the zones of `variant`'s body that a conditional chunk of `chunks` would cover if its
/// conditions held and that no other chunk covers: the zones whose cut depends on conditions that
/// are not evaluated.
pub(crate) fn undecided<'c>(
    chunks: impl IntoIterator<Item = &'c ZoneChunk>,
    variant: BodyVariant,
    covered: &BTreeSet<&str>,
) -> BTreeSet<&'c str> {
    chunks
        .into_iter()
        .filter(|chunk| chunk.hides_body() && chunk.conditional)
        .map(|chunk| chunk.zone.as_str())
        .filter(|zone| {
            variant.zone_names().iter().any(|name| name == zone) && !covered.contains(zone)
        })
        .collect()
}

/// Body zones affected by the authored clothing chunks for a body variant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClothingZoneCoverage<'chunks> {
    covered: BTreeSet<&'chunks str>,
    undecided: BTreeSet<&'chunks str>,
}

impl<'chunks> ClothingZoneCoverage<'chunks> {
    /// Returns the zones unconditionally covered by the clothing.
    #[must_use]
    pub const fn covered(&self) -> &BTreeSet<&'chunks str> {
        &self.covered
    }

    /// Returns the remaining zones whose coverage depends on unevaluated conditions.
    #[must_use]
    pub const fn undecided(&self) -> &BTreeSet<&'chunks str> {
        &self.undecided
    }

    pub(super) fn of(
        chunks: impl IntoIterator<Item = &'chunks ZoneChunk> + Clone,
        variant: BodyVariant,
    ) -> Self {
        let mut covered = covered(chunks.clone());
        covered.retain(|zone| variant.zone_names().contains(zone));
        let undecided = undecided(chunks, variant, &covered);
        Self { covered, undecided }
    }
}

/// Returns the model of `job` as it is worn: the body without the zones that clothing covers,
/// reported in `issues` with the zones whose cover is undecided; any other model as it is.
pub(super) fn without_covered(
    job: &Job,
    model: Arc<Model>,
    variant: BodyVariant,
    coverage: &ClothingZoneCoverage<'_>,
    issues: &mut Vec<Issue>,
) -> Arc<Model> {
    if job.origin != PartOrigin::Body {
        return model;
    }
    let (model, kind) = uncovered(model, variant, &coverage.covered);
    issues.extend(kind.map(|kind| issue(job, kind)));
    if !coverage.undecided.is_empty() {
        let zones = coverage.undecided.iter().map(|zone| (*zone).to_owned());
        let kind = IssueKind::ClothingConditionsUndecided(zones.collect());
        issues.push(issue(job, kind));
    }
    model
}

/// Returns the body `model` without the zones in `covered`, with the issue that says which were
/// hidden or why none could be; the model as it is when no zone is covered.
pub(crate) fn uncovered(
    model: Arc<Model>,
    variant: BodyVariant,
    covered: &BTreeSet<&str>,
) -> (Arc<Model>, Option<IssueKind>) {
    match uncover(&model, variant, covered) {
        Ok(None) => (model, None),
        Ok(Some(done)) => {
            let zones = done.hidden.iter().map(|zone| (*zone).to_owned()).collect();
            (
                Arc::new(done.model),
                Some(IssueKind::BodyZonesHidden(zones)),
            )
        }
        Err(misfit) => (model, Some(IssueKind::BodyZonesUnknown(misfit.to_string()))),
    }
}

#[cfg(test)]
mod tests;
