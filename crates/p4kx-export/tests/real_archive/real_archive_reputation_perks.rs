// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;
use std::error::Error;

use p4kx_export::Kind;
use p4kx_extraction::{DataForge, StructValue, TreeBuilder, Value as Original};
use serde_json::Value;

use crate::authored_definition_witness::{field, verify_state};
use crate::catalog_v3_consumer as consumer;
use crate::real_archive_auto_spawns::provenance::Origins;
use crate::real_archive_canonical_texts::TextIndex;
use crate::real_archive_faction_reputation_source::presentation::Witness;

const LIST_TYPE: &str = "SPerkReputationListParams";
const KIND: Kind = Kind::ReputationPerks;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

pub(super) fn check_lists(
    forge: &DataForge<'_>,
    catalog: &Value,
    origins: &Origins<'_>,
    texts: &TextIndex<'_>,
) -> TestResult<(usize, usize)> {
    let rows = consumer::rows(catalog, KIND)?;
    let expected: BTreeSet<_> = forge
        .records()
        .filter(|record| matches!(record.type_name(), Ok(LIST_TYPE)))
        .map(|record| record.index())
        .collect();
    assert!(
        !expected.is_empty(),
        "configured Build contains reputation perk lists"
    );
    let mut seen = BTreeSet::new();
    let mut order = Vec::new();
    let mut trees = TreeBuilder::new();
    let mut entries = 0;
    for row in rows {
        let index = u32::try_from(
            consumer::field(row, KIND, "recordIndex")?
                .as_u64()
                .ok_or("physical perk-list index")?,
        )?;
        let source = forge.record(index).ok_or("original perk-list record")?;
        let guid = consumer::identity(row, KIND)?;
        assert!(seen.insert(index), "one row per physical perk list");
        assert!(
            matches!(source.type_name(), Ok(LIST_TYPE)),
            "exact source record type"
        );
        assert!(guid == source.guid().to_string(), "exact source list GUID");
        order.push((guid, index));
        let tree = trees
            .build(&source)
            .map_err(|_| "read original perk-list body")?;
        let witness = Witness::new(forge, &tree, origins, texts);
        let slots = tree.field("perks");
        witness.array(slots, field(row, "perks")?, |source, cell| {
            check_entry(&witness, source, cell)
        })?;
        if let Some(Original::Array(slots)) = slots {
            entries += slots.len();
        }
    }
    assert_eq!(seen, expected, "all physical perk-list records emitted");
    assert!(
        order.is_sorted(),
        "GUID and physical ordinal define perk-list order"
    );
    Ok((rows.len(), entries))
}

fn check_entry<'source>(
    witness: &Witness<'_, '_, 'source>,
    source: Option<&Original<'source>>,
    cell: &Value,
) -> TestResult {
    witness.owned(source, cell, |source, row| {
        for (name, target) in [
            ("scope", "SReputationScopeParams"),
            ("standing", "SReputationStandingParams"),
        ] {
            witness.link(source.field(name), field(row, name)?, target)?;
        }
        witness.owned(source.field("perk"), field(row, "perk")?, |source, row| {
            check_perk(witness, source, row)
        })
    })
}

fn check_perk<'source>(
    witness: &Witness<'_, '_, 'source>,
    source: &StructValue<'source>,
    row: &Value,
) -> TestResult {
    for name in ["name", "description", "icon"] {
        witness.scalar(source.field(name), field(row, name)?)?;
    }
    for name in ["displayName", "displayDescription"] {
        witness.locale(source.field(name), field(row, name)?)?;
    }
    let details = field(row, "details")?;
    let kind = match source.type_name() {
        "SPerkParamsBase" => "base",
        "SPerkParamsItem" => {
            witness.link(
                source.field("rewardItem"),
                field(details, "rewardItem")?,
                "EntityClassDefinition",
            )?;
            "item"
        }
        "SPerkShopItemCategoryBase" => "shopItemCategory",
        "SPerkEnabledItemCategory" => "enabledItemCategory",
        "SPerkDiscountItemCategory" => {
            witness.scalar(
                source.field("discountPercentage"),
                field(details, "discountPercentage")?,
            )?;
            "discountItemCategory"
        }
        _ => "authored",
    };
    assert!(
        details.get("type").and_then(Value::as_str) == Some(kind),
        "original concrete perk type"
    );
    if matches!(
        kind,
        "shopItemCategory" | "enabledItemCategory" | "discountItemCategory"
    ) {
        check_category(witness, source, field(details, "category")?)?;
    }
    Ok(())
}

fn check_category<'source>(
    witness: &Witness<'_, '_, 'source>,
    source: &StructValue<'source>,
    row: &Value,
) -> TestResult {
    for name in ["type", "subType"] {
        witness.scalar(source.field(name), field(row, name)?)?;
    }
    for (name, target) in [("franchise", "ShopFranchise"), ("requiredTag", "Tag")] {
        witness.link(source.field(name), field(row, name)?, target)?;
    }
    let original = source.field("id");
    let cell = field(row, "id")?;
    witness.occurrence(original, cell)?;
    if let Some(Original::Guid(guid)) = original {
        assert!(
            consumer::present(cell)?.as_str() == Some(guid.to_string().as_str()),
            "original perk category GUID"
        );
        Ok(())
    } else {
        verify_state(original, cell)
    }
}
