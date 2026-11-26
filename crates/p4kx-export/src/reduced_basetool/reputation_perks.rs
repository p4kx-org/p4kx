// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::CancelToken;
use serde::Serialize;

use super::ReducedError;
use super::cell::Cell;
use super::completeness;
use super::number::Number;
use super::record_identity::RecordIdentity;
use super::record_link::RecordLink;
use super::record_targets::RecordTargets;
use super::selected_texts::TextRefs;
use crate::catalog_queries::{
    ReputationPerk, ReputationPerkEntry, ReputationPerkList, ReputationRows,
};
use crate::catalog_rows::staged::{StagedCatalogRows, StagedCatalogTable};
use crate::catalog_text_contract::GameTextRef;
use crate::catalog_v3::reputation_perk::{PerkCategory, PerkDetails};
use crate::guid_text::CatalogGuid;

pub(super) const PATH: &str = "definitions.reputationPerks";

pub(super) struct Perks {
    pub(super) table: StagedCatalogTable,
    pub(super) targets: RecordTargets,
}

pub(super) struct RankTargets<'read> {
    pub(super) scopes: &'read RecordTargets,
    pub(super) standings: &'read RecordTargets,
}

pub(super) fn stage<'read>(
    source: &'read ReputationRows<ReputationPerkList<'_>>,
    ranks: RankTargets<'_>,
    scratch: &Path,
    cancel: Option<&CancelToken>,
    references: &mut TextRefs<'read>,
) -> Result<Perks, ReducedError> {
    completeness::inventory(PATH, source.availability, &source.coverage)?;
    let mut targets = RecordTargets::default();
    let mut output = StagedCatalogRows::new(scratch, cancel);
    for row in &source.rows {
        let header = row.source();
        completeness::value(PATH, header.class_name())?;
        completeness::value(PATH, header.record_path())?;
        targets.insert(PATH, header)?;
        output.append(&List {
            identity: header.into(),
            perks: completeness::list(PATH, row.perks(), |perk| {
                Entry::read(perk, &ranks, references)
            })?,
        })?;
    }
    let table = output.finish(source.availability)?;
    completeness::rows(PATH, source.rows.len(), table.len())?;
    Ok(Perks { table, targets })
}

#[derive(Serialize)]
struct List<'read> {
    #[serde(flatten)]
    identity: RecordIdentity<'read>,
    perks: Cell<Vec<Cell<Entry<'read>>>>,
}

#[derive(Serialize)]
struct Entry<'read> {
    scope: Cell<RecordLink<'read>>,
    standing: Cell<RecordLink<'read>>,
    perk: Cell<Perk<'read>>,
}

impl<'read> Entry<'read> {
    fn read(
        source: &'read ReputationPerkEntry<'_>,
        ranks: &RankTargets<'_>,
        references: &mut TextRefs<'read>,
    ) -> Result<Self, ReducedError> {
        Ok(Self {
            scope: completeness::cell(PATH, source.scope())?
                .try_map(|link| RecordLink::read_source(PATH, link, ranks.scopes, |_| true))?,
            standing: completeness::cell(PATH, source.standing())?
                .try_map(|link| RecordLink::read_source(PATH, link, ranks.standings, |_| true))?,
            perk: completeness::cell(PATH, source.perk())?
                .try_map(|value| Perk::read(value, references))?,
        })
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Perk<'read> {
    source_type: Cell<&'read str>,
    name: Cell<&'read str>,
    description: Cell<&'read str>,
    display_name: Cell<&'read GameTextRef>,
    display_description: Cell<&'read GameTextRef>,
    icon: Cell<&'read str>,
    details: Details<'read>,
}

impl<'read> Perk<'read> {
    fn read(
        source: &'read ReputationPerk<'_>,
        references: &mut TextRefs<'read>,
    ) -> Result<Self, ReducedError> {
        let display_name = completeness::cell(PATH, source.display_name())?;
        let display_description = completeness::cell(PATH, source.display_description())?;
        references.game_text(display_name.value);
        references.game_text(display_description.value);
        Ok(Self {
            source_type: completeness::cell(PATH, source.source_type())?.map(|value| *value),
            name: completeness::cell(PATH, source.name())?.map(|value| *value),
            description: completeness::cell(PATH, source.description())?.map(|value| *value),
            display_name,
            display_description,
            icon: completeness::cell(PATH, source.icon())?.map(|value| *value),
            details: Details::read(source.details())?,
        })
    }
}

#[derive(Serialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
enum Details<'read> {
    Base,
    Item {
        reward_item: Cell<RecordLink<'read>>,
    },
    ShopItemCategory {
        category: Box<Category<'read>>,
    },
    EnabledItemCategory {
        category: Box<Category<'read>>,
    },
    DiscountItemCategory {
        category: Box<Category<'read>>,
        discount_percentage: Cell<Number<'static>>,
    },
    Authored,
}

impl<'read> Details<'read> {
    fn read(source: &'read PerkDetails<'_>) -> Result<Self, ReducedError> {
        Ok(match source {
            PerkDetails::Base => Self::Base,
            PerkDetails::Item { reward_item } => Self::Item {
                reward_item: completeness::cell(PATH, reward_item)?
                    .try_map(|link| RecordLink::from_source(PATH, link))?,
            },
            PerkDetails::ShopItemCategory { category } => Self::ShopItemCategory {
                category: Box::new(Category::read(category)?),
            },
            PerkDetails::EnabledItemCategory { category } => Self::EnabledItemCategory {
                category: Box::new(Category::read(category)?),
            },
            PerkDetails::DiscountItemCategory {
                category,
                discount_percentage,
            } => Self::DiscountItemCategory {
                category: Box::new(Category::read(category)?),
                discount_percentage: completeness::cell(PATH, discount_percentage)?
                    .map(Number::from),
            },
            PerkDetails::Authored => Self::Authored,
        })
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Category<'read> {
    id: Cell<&'read CatalogGuid>,
    franchise: Cell<RecordLink<'read>>,
    #[serde(rename = "type")]
    item_type: Cell<&'read str>,
    sub_type: Cell<&'read str>,
    required_tag: Cell<RecordLink<'read>>,
}

impl<'read> Category<'read> {
    fn read(source: &'read PerkCategory<'_>) -> Result<Self, ReducedError> {
        Ok(Self {
            id: completeness::cell(PATH, source.id())?,
            franchise: completeness::cell(PATH, source.franchise())?
                .try_map(|link| RecordLink::from_source(PATH, link))?,
            item_type: completeness::cell(PATH, source.item_type())?.map(|value| *value),
            sub_type: completeness::cell(PATH, source.sub_type())?.map(|value| *value),
            required_tag: completeness::cell(PATH, source.required_tag())?
                .try_map(|link| RecordLink::from_source(PATH, link))?,
        })
    }
}
