// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use crate::CatalogError;
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_v3::gameplay_property_catalog::GameplayPropertyRows;
use crate::quality_catalog::{Assembly, QualityRows};
use p4kx_extraction::{CancelToken, Guid};

use crate::crafting::Crafting;
use crate::export_ledger::ExportLedger;

use super::ReadCrafting;
use super::projection::Piece;
use super::read::Tracked;

fn insert<'f>(data: &mut Crafting<'f>, links: &mut HashMap<Guid, Vec<Guid>>, piece: Piece<'f>) {
    match piece {
        Piece::Blueprint(value) => data.blueprints.push(value),
        Piece::Property(value) => data.properties.push(value),
        Piece::Distribution(value) => data.quality_distributions.push(value),
        Piece::Quantization(value) => data.quality_quantizations.push(value),
        Piece::Pool(value) => data.pools.push(value),
        Piece::Dismantle(value) => data.dismantle.push(value),
        Piece::ResourceLinks {
            resource_type,
            quantizations,
        } => {
            for quantization in quantizations {
                links.entry(quantization).or_default().push(resource_type);
            }
        }
        Piece::Globals(value) => {
            data.global
                .default_blueprints
                .extend(value.default_blueprints);
            data.global
                .dismantle_blacklist_resources
                .extend(value.dismantle_blacklist_resources);
            data.global
                .dismantle_blacklist_items
                .extend(value.dismantle_blacklist_items);
        }
        Piece::Skipped => data.skipped_blueprints = data.skipped_blueprints.saturating_add(1),
        Piece::Failed(value) => data.failures.push(value),
    }
}

fn quantizations(data: &mut Crafting<'_>, mut links: HashMap<Guid, Vec<Guid>>) {
    for quantization in &mut data.quality_quantizations {
        if let Some(mut resource_types) = links.remove(&quantization.guid) {
            resource_types.sort_unstable();
            resource_types.dedup();
            quantization.resource_types = resource_types;
        }
    }
    data.unresolved_quantization_links = links
        .into_values()
        .map(|mut resources| {
            resources.sort_unstable();
            resources.dedup();
            resources.len()
        })
        .sum();
}

pub(super) fn assemble<'f>(
    tracked: Vec<Tracked<'f>>,
    origins: Option<&CatalogOrigins>,
    cancel: Option<&CancelToken>,
) -> Result<ReadCrafting<'f>, CatalogError> {
    let mut data = Crafting::default();
    let mut ledger = ExportLedger::default();
    let mut links = HashMap::new();
    let mut quality = Assembly::default();
    let mut gameplay = GameplayPropertyRows::default();
    for tracked in tracked {
        ledger.merge(tracked.ledger);
        quality.add(tracked.quality);
        gameplay.add(tracked.gameplay);
        if let Some(piece) = tracked.piece {
            insert(&mut data, &mut links, piece);
        }
    }
    data.blueprints.sort_by_key(|item| item.guid);
    data.properties.sort_by_key(|item| item.guid);
    data.quality_distributions.sort_by_key(|item| item.guid);
    data.quality_quantizations.sort_by_key(|item| item.guid);
    data.dismantle.sort_by_key(|item| item.guid);
    data.pools.sort_by_key(|item| item.guid);
    quantizations(&mut data, links);
    gameplay.order();
    let quality = match origins {
        Some(origins) => quality.finish(origins, cancel)?,
        None => QualityRows::default(),
    };
    Ok(ReadCrafting {
        data,
        ledger,
        gameplay,
        quality,
    })
}
