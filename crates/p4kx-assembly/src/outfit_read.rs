// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_dataforge::{Guid, StructValue, Value, ValueError};

use crate::loadout_path::PathRead;
use crate::read::{struct_field, struct_items};
use crate::spec::{LoadoutSpec, read_loadout};

const MAX_OUTFIT_LAYERS: usize = 16;

/// A name tag of an outfit as the Record states it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TagSpec {
    /// A Reference to a Tag Record.
    Guid(Guid),
    /// A tag written out as text.
    Name(String),
}

/// One outfit of `SItemPortLoadoutOutfitParams` as the Record states it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OutfitSpec {
    pub(crate) tags: Vec<TagSpec>,
    pub(crate) files: Vec<String>,
    pub(crate) applied: bool,
}

/// What an Entity's `SItemPortLoadoutOutfitParams` holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OutfitInfo {
    pub(crate) base_files: Vec<String>,
    pub(crate) outfits: Vec<OutfitSpec>,
}

/// An outfit read once: the layers it is made of and, when asked for, the report.
pub(crate) struct OutfitRead {
    pub(crate) loadout: Option<LoadoutSpec>,
    pub(crate) info: Option<OutfitInfo>,
}

/// Returns for each outfit whether it applies, given for each whether it has name tags: every
/// outfit without name tags applies; when all have name tags, the first one does.
pub(crate) fn applied(tagged: &[bool]) -> Vec<bool> {
    let any_untagged = tagged.iter().any(|tagged| !tagged);
    tagged
        .iter()
        .enumerate()
        .map(|(at, tagged)| if any_untagged { !tagged } else { at == 0 })
        .collect()
}

/// Reads an outfit once. The layers are the base Loadout, then the Loadouts of the outfits that
/// apply. With `report`, the XML files of the outfits that do not apply are read too, for
/// [`OutfitInfo`]; without it their Loadouts are not touched.
pub(crate) fn read(
    outfit: &StructValue<'_>,
    record: &Arc<str>,
    report: bool,
    paths: &PathRead<'_>,
) -> OutfitRead {
    let base = struct_field(outfit, "BaseLoadout")
        .and_then(|loadout| read_loadout(loadout, record, paths));
    let candidates: Vec<&StructValue<'_>> = struct_items(outfit, "Outfits").collect();
    let tags: Vec<Vec<TagSpec>> = candidates.iter().map(|outfit| tag_specs(outfit)).collect();
    let tagged: Vec<bool> = tags.iter().map(|tags| !tags.is_empty()).collect();
    let applies = applied(&tagged);

    let mut layers: Vec<LoadoutSpec> = base.iter().cloned().collect();
    let mut specs = Vec::with_capacity(if report { candidates.len() } else { 0 });
    for ((candidate, tags), applied) in candidates.into_iter().zip(tags).zip(applies) {
        if !applied && !report {
            continue;
        }
        let loadouts = struct_items(candidate, "Loadouts")
            .filter_map(|loadout| read_loadout(loadout, record, paths));
        let mut files = Vec::new();
        for loadout in loadouts {
            loadout.xml_paths(&mut files);
            if applied {
                layers.push(loadout);
            }
        }
        if report {
            specs.push(OutfitSpec {
                tags,
                files,
                applied,
            });
        }
    }
    layers.truncate(MAX_OUTFIT_LAYERS);
    let info = report.then(|| {
        let mut base_files = Vec::new();
        if let Some(base) = &base {
            base.xml_paths(&mut base_files);
        }
        OutfitInfo {
            base_files,
            outfits: specs,
        }
    });
    OutfitRead {
        loadout: (!layers.is_empty()).then(|| LoadoutSpec::Layered(layers.into())),
        info,
    }
}

fn tag_specs(outfit: &StructValue<'_>) -> Vec<TagSpec> {
    match outfit.field("OutfitNameTags") {
        Some(Value::Array(items)) => items.iter().filter_map(tag_spec).collect(),
        _ => Vec::new(),
    }
}

fn tag_spec(value: &Value<'_>) -> Option<TagSpec> {
    match value {
        Value::Reference(link) => Some(TagSpec::Guid(link.guid())),
        Value::Error(ValueError::DanglingReference { guid }) => Some(TagSpec::Guid(*guid)),
        Value::String(text) | Value::Enum(text) | Value::Locale(text) if !text.is_empty() => {
            Some(TagSpec::Name((*text).to_owned()))
        }
        _ => None,
    }
}

impl LoadoutSpec {
    /// Appends the paths of the XML Loadouts this Loadout is made of, in the order they are laid
    /// over each other.
    pub(crate) fn xml_paths(&self, paths: &mut Vec<String>) {
        match self {
            Self::Xml(path) => paths.push(path.to_string()),
            Self::Layered(layers) => {
                for layer in layers.iter() {
                    layer.xml_paths(paths);
                }
            }
            Self::Manual(_) | Self::Assortments(_) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use p4kx_dataforge::Value;

    use super::{applied, tag_spec};

    #[test]
    fn a_null_reference_is_no_name_tag() {
        assert_eq!(tag_spec(&Value::Null), None);
        assert_eq!(tag_spec(&Value::String("")), None);
    }

    #[test]
    fn untagged_outfits_all_apply_and_tagged_ones_do_not() {
        assert_eq!(applied(&[false, true, false]), [true, false, true]);
    }

    #[test]
    fn when_every_outfit_is_tagged_the_first_applies() {
        assert_eq!(applied(&[true, true]), [true, false]);
        assert_eq!(applied(&[true]), [true]);
    }

    #[test]
    fn no_outfit_applies_none() {
        assert_eq!(applied(&[]), [] as [bool; 0]);
    }
}
