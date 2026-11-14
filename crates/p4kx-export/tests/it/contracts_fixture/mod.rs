// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! A synthetic Archive with contracts, sub contracts, templates, mission types, a mission
//! organisation with tagged string variants and reputation rewards.

mod fixed_reward_origins;
mod gaps;
mod lookups;
mod mission_item_titles;
mod prerequisites;
mod records;
mod referenced;
mod reputation_gain_origins;
mod structures;
mod title_limits;
mod title_missing;
mod title_origins;

use std::error::Error;
use std::path::{Path, PathBuf};

use p4kx_testkit::{ArchiveBuilder, DataForgeBuilder, GlobalIniBuilder, TestEntry};

pub(crate) const GENERATOR: u128 = 0x1000_0000_0000_4000_8000_0000_0000_0001;
pub(crate) const CONTRACT_A: u128 = 0x2000_0000_0000_4000_8000_0000_0000_0001;
pub(crate) const CONTRACT_B: u128 = 0x2000_0000_0000_4000_8000_0000_0000_0002;
pub(crate) const CONTRACT_C: u128 = 0x2000_0000_0000_4000_8000_0000_0000_0003;
pub(crate) const CONTRACT_DEV: u128 = 0x2000_0000_0000_4000_8000_0000_0000_0004;
pub(crate) const CONTRACT_D: u128 = 0x2000_0000_0000_4000_8000_0000_0000_0005;
pub(crate) const CONTRACT_E: u128 = 0x2000_0000_0000_4000_8000_0000_0000_0006;
pub(crate) const GENERATOR_COPY: u128 = 0x1000_0000_0000_4000_8000_0000_0000_0002;
pub(crate) const GENERATOR_DEV: u128 = 0x1000_0000_0000_4000_8000_0000_0000_0003;
pub(crate) const CONTRACT_F: u128 = 0x2000_0000_0000_4000_8000_0000_0000_0007;
pub(crate) const CONTRACT_G: u128 = 0x2000_0000_0000_4000_8000_0000_0000_0008;
pub(crate) const CONTRACT_H: u128 = 0x2000_0000_0000_4000_8000_0000_0000_0009;
pub(crate) const CONTRACT_I: u128 = 0x2000_0000_0000_4000_8000_0000_0000_000a;
pub(crate) const SOLO_ORGANIZATION: u128 = 0x8000_0000_0000_4000_8000_0000_0000_0002;
pub(crate) const STRAY_TEMPLATE: u128 = 0x4000_0000_0000_4000_8000_0000_0000_0003;
pub(crate) const MISSING_TEMPLATE: u128 = 0x4000_0000_0000_4000_8000_0000_0000_0099;
pub(crate) const BROKER: u128 = 0xd000_0000_0000_4000_8000_0000_0000_0001;
pub(crate) const SUB_ONE: u128 = 0x3000_0000_0000_4000_8000_0000_0000_0001;
pub(crate) const SUB_TWO: u128 = 0x3000_0000_0000_4000_8000_0000_0000_0002;
pub(crate) const TEMPLATE: u128 = 0x4000_0000_0000_4000_8000_0000_0000_0001;
pub(crate) const PLAIN_TEMPLATE: u128 = 0x4000_0000_0000_4000_8000_0000_0000_0002;
pub(crate) const DELIVERY: u128 = 0x5000_0000_0000_4000_8000_0000_0000_0001;
pub(crate) const BOUNTY: u128 = 0x5000_0000_0000_4000_8000_0000_0000_0002;
pub(crate) const ESCORT: u128 = 0x5000_0000_0000_4000_8000_0000_0000_0003;
pub(crate) const FACTION: u128 = 0x6000_0000_0000_4000_8000_0000_0000_0001;
pub(crate) const FACTION_SCOPE: u128 = 0x6000_0000_0000_4000_8000_0000_0000_0002;
pub(crate) const AFFINITY: u128 = 0x6000_0000_0000_4000_8000_0000_0000_0003;
pub(crate) const RANK_TWO_HUNDRED: u128 = 0x7000_0000_0000_4000_8000_0000_0000_0001;
pub(crate) const RANK_FIFTY: u128 = 0x7000_0000_0000_4000_8000_0000_0000_0002;
pub(crate) const LOSS: u128 = 0x7000_0000_0000_4000_8000_0000_0000_0003;
pub(crate) const ORGANIZATION: u128 = 0x8000_0000_0000_4000_8000_0000_0000_0001;
pub(crate) const DATABASE: u128 = 0x9000_0000_0000_4000_8000_0000_0000_0001;
pub(crate) const TAG_TITLE: u128 = 0x9000_0000_0000_4000_8000_0000_0000_0002;
pub(crate) const TAG_OTHER: u128 = 0x9000_0000_0000_4000_8000_0000_0000_0003;

pub(crate) const LOC_A: u128 = 0xa000_0000_0000_4000_8000_0000_0000_0001;
pub(crate) const LOC_B: u128 = 0xa000_0000_0000_4000_8000_0000_0000_0002;
pub(crate) const TYPE_SYSTEM: u128 = 0xb000_0000_0000_4000_8000_0000_0000_0001;
pub(crate) const TYPE_PLANET: u128 = 0xb000_0000_0000_4000_8000_0000_0000_0002;
pub(crate) const TYPE_STAR: u128 = 0xb000_0000_0000_4000_8000_0000_0000_0003;
pub(crate) const TYPE_STATION: u128 = 0xb000_0000_0000_4000_8000_0000_0000_0004;
pub(crate) const OBJ_SYSTEM: u128 = 0xc000_0000_0000_4000_8000_0000_0000_0001;
pub(crate) const OBJ_PLANET: u128 = 0xc000_0000_0000_4000_8000_0000_0000_0002;
pub(crate) const OBJ_STAR: u128 = 0xc000_0000_0000_4000_8000_0000_0000_0003;
pub(crate) const OBJ_STATION: u128 = 0xc100_0000_0000_4000_8000_0000_0000_0000;
pub(crate) const OBJ_MISSING: u128 = 0xc200_0000_0000_4000_8000_0000_0000_0001;
pub(crate) const STRAY_FACTION: u128 = 0x6000_0000_0000_4000_8000_0000_0000_0005;
pub(crate) const RIVAL: u128 = 0x6000_0000_0000_4000_8000_0000_0000_0004;

fn database() -> DataForgeBuilder {
    records::generator(lookups::places(lookups::reputation(
        referenced::referenced(structures::structures(
            DataForgeBuilder::new()
                .version(8)
                .enumeration("ECurrency", ["UEC"]),
        )),
    )))
}

fn english() -> TestEntry {
    TestEntry::zstd(
        "Data\\Localization\\english\\global.ini",
        GlobalIniBuilder::new()
            .text(
                "title_a",
                "Deliver ~mission(Cargo) to ~mission(Location) for ~mission(Contractor|Title) ~mission(Missing)  ",
            )
            .text("title_sub", "Stop ~mission(Target) in ~mission(Ship)")
            .text("title_template", "Plain Delivery   ")
            .text("title_handler", "Handler Title")
            .text("title_nested", "Pre ~mission(Wrap)")
            .text(
                "title_novariant",
                "By ~mission(Several) and ~mission(Solo)",
            )
            .text("org_solo", "Solo Corp")
            .text("wrap_text", "Wrapped ~mission(Location)")
            .text("contractor_acme", "Acme Agency")
            .text("title_broker", "Broker Haul")
            .text(
                "desc_handler",
                "  Wanted ~mission(Missing).\\nReward on delivery.\\n  ",
            )
            .text("desc_broker", "Haul it")
            .text("faction_acme", "Acme Emporium")
            .text("place_stanton", "Stanton")
            .text("place_hurston", "Hurston")
            .text("place_sol", "Sol")
            .text("place_station0", "Everus")
            .text("title_plain", "Plain Title")
            .text("cargo_scrap", "Scrap")
            .text("cargo_gold", "Gold")
            .text("org_acme_title", "Acme Corp")
            .text("org_acme_other", "Acme")
            .text("type_delivery", "Delivery")
            .text("type_bounty", "Bounty")
            .text("type_escort", "Escort")
            .build(),
    )
}

fn german() -> TestEntry {
    TestEntry::zstd(
        "Data\\Localization\\german_(germany)\\global.ini",
        GlobalIniBuilder::new()
            .text(
                "title_a",
                "Liefere ~mission(Cargo) nach ~mission(Location) fuer ~mission(Contractor|Title)",
            )
            .text("cargo_gold", "Gold")
            .text("org_acme_title", "Acme GmbH")
            .text("type_delivery", "Lieferung")
            .build(),
    )
}

/// Writes the fixture Archive into `dir` and returns its path.
pub(crate) fn write_archive(dir: &Path) -> Result<PathBuf, Box<dyn Error>> {
    let path = dir.join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::zstd("Data\\Game2.dcb", database().build()).encrypted())
        .entry(english())
        .entry(german())
        .write_to(&path)?;
    Ok(path)
}
