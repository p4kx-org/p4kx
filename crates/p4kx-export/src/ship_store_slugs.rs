// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;
use std::str::FromStr;

use p4kx_extraction::Guid;
use serde::Deserialize;

use crate::CatalogError;
use crate::contract::ShipStoreSlug;
use crate::guid_text::CatalogGuid;
use crate::store_slug_summary::StoreSlugSummary;

const DATA: &str = include_str!("../data/ship-store-slugs.json");
const FORMAT: u32 = 1;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    format: u32,
    ships: Vec<Entry>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Entry {
    slug: String,
    rsi_id: u32,
    store_name: String,
    class_name: Option<String>,
    guid: Option<String>,
    retired: bool,
}

/// A ship of the store and the catalog ship it maps to, if any.
struct StoreShip {
    slug: String,
    rsi_id: u32,
    store_name: String,
    mapped: Option<Mapped>,
    retired: bool,
}

struct Mapped {
    class_name: String,
    guid: Guid,
}

/// The curated map from RSI store slugs to catalog ships, as `data/ship-store-slugs.json` holds
/// it (ADR-0117).
pub(crate) struct StoreSlugs {
    slugs: Vec<StoreShip>,
}

/// The exported entries and the counts of one resolution.
pub(crate) struct Resolved<'s> {
    pub(crate) exported: Vec<ShipStoreSlug<'s>>,
    pub(crate) summary: StoreSlugSummary,
}

impl StoreSlugs {
    /// Reads the data file compiled into the tool.
    pub(crate) fn embedded() -> Result<Self, CatalogError> {
        Self::parse(DATA)
    }

    /// Reads `text` as a data file and checks it: a known format, unique slugs and RSI ids, a
    /// class name with every GUID, and a mapping behind every `retired` mark.
    pub(crate) fn parse(text: &str) -> Result<Self, CatalogError> {
        let file: File = serde_json::from_str(text).map_err(|error| invalid(&error))?;
        if file.format != FORMAT {
            return Err(invalid(&format_args!("unknown format {}", file.format)));
        }
        let mut slugs = Vec::with_capacity(file.ships.len());
        let mut seen_slugs = HashSet::with_capacity(file.ships.len());
        let mut seen_ids = HashSet::with_capacity(file.ships.len());
        let mut seen_guids = HashSet::new();
        for entry in file.ships {
            let name = entry.slug.as_str();
            if !seen_slugs.insert(entry.slug.clone()) {
                return Err(invalid(&format_args!("slug {name} occurs twice")));
            }
            if !seen_ids.insert(entry.rsi_id) {
                return Err(invalid(&format_args!(
                    "RSI id {} occurs twice",
                    entry.rsi_id
                )));
            }
            let mapped = match (entry.class_name, entry.guid) {
                (Some(class_name), Some(guid)) => {
                    let guid = Guid::from_str(&guid)
                        .map_err(|error| invalid(&format_args!("slug {name}: {error}")))?;
                    if !seen_guids.insert(guid) {
                        return Err(invalid(&format_args!(
                            "the GUID of slug {name} maps a second slug"
                        )));
                    }
                    Some(Mapped { class_name, guid })
                }
                (None, None) => None,
                _ => {
                    return Err(invalid(&format_args!(
                        "slug {name} has a class name or a GUID but not both"
                    )));
                }
            };
            if entry.retired && mapped.is_none() {
                return Err(invalid(&format_args!("retired slug {name} has no mapping")));
            }
            slugs.push(StoreShip {
                slug: entry.slug,
                rsi_id: entry.rsi_id,
                store_name: entry.store_name,
                mapped,
                retired: entry.retired,
            });
        }
        slugs.sort_by(|left, right| left.slug.cmp(&right.slug));
        Ok(Self { slugs })
    }

    pub(crate) fn ship_guids(&self, filtered: bool) -> HashSet<Guid> {
        self.slugs
            .iter()
            .filter(|slug| !filtered || !slug.retired)
            .filter_map(|slug| slug.mapped.as_ref().map(|mapped| mapped.guid))
            .collect()
    }

    /// Resolves every mapped slug against `ships` and returns what the catalog exports.
    ///
    /// A slug resolves when a ship has its GUID and its class name. A mapped slug that does not
    /// resolve is stale and listed in the summary, not exported; a `retired` slug is exported
    /// without ship, unless its ship exists again, which makes it stale too. With a Filter
    /// (`filtered`) the ships are a selection, so only resolving slugs are exported and none is
    /// stale. When slugs are mapped and none resolves, the stale check is skipped and the
    /// summary says so.
    pub(crate) fn resolve<'a>(
        &self,
        ships: impl IntoIterator<Item = (Guid, &'a str)>,
        filtered: bool,
    ) -> Resolved<'_> {
        let classes: HashSet<(Guid, &str)> = ships.into_iter().collect();
        let mut summary = StoreSlugSummary {
            slugs: self.slugs.len(),
            ..StoreSlugSummary::default()
        };
        let mut exported = Vec::new();
        for slug in &self.slugs {
            let Some(mapped) = &slug.mapped else {
                summary.unmapped += 1;
                continue;
            };
            summary.mapped += 1;
            if slug.retired {
                summary.retired += 1;
                if filtered {
                    continue;
                }
                if classes.iter().any(|(guid, _)| *guid == mapped.guid) {
                    summary.stale.push(slug.slug.clone());
                } else {
                    exported.push(entry(slug, None));
                }
            } else if classes.contains(&(mapped.guid, mapped.class_name.as_str())) {
                summary.resolved += 1;
                exported.push(entry(slug, Some(mapped)));
            } else if !filtered {
                summary.stale.push(slug.slug.clone());
            }
        }
        if summary.mapped > 0 && summary.resolved == 0 {
            summary.stale.clear();
            summary.stale_check_skipped = !filtered;
        }
        summary.exported = exported.len();
        Resolved { exported, summary }
    }
}

fn entry<'s>(slug: &'s StoreShip, mapped: Option<&'s Mapped>) -> ShipStoreSlug<'s> {
    ShipStoreSlug {
        slug: &slug.slug,
        rsi_id: slug.rsi_id,
        store_name: &slug.store_name,
        class_name: mapped.map(|mapped| mapped.class_name.as_str()),
        ship_guid: mapped.map(|mapped| CatalogGuid(mapped.guid)),
        retired: slug.retired,
    }
}

fn invalid(reason: &dyn std::fmt::Display) -> CatalogError {
    CatalogError::StoreSlugs(reason.to_string())
}

#[cfg(test)]
mod tests;
