// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};
use std::path::Path;

use p4kx_export::{
    CatalogPublicationMode, CatalogSummary, LoadoutSummary, PaintSummary, Playability, ShopSummary,
    StarmapSummary, StoreSlugSummary,
};

const STALE_SLUGS_SHOWN: usize = 5;
fn print_classification(stdout: &mut impl Write, summary: &CatalogSummary) -> io::Result<()> {
    let playability = summary.playability();
    for (kind, counts) in [("items", playability.items), ("ships", playability.ships)] {
        writeln!(
            stdout,
            "{kind} playability: {} player, {} ai, {} mission, {} special, {} unclassified",
            counts.of(Playability::Player),
            counts.of(Playability::Ai),
            counts.of(Playability::Mission),
            counts.of(Playability::Special),
            counts.unclassified()
        )?;
    }
    let variants = summary.variants();
    for (kind, counts) in [("items", variants.items), ("ships", variants.ships)] {
        writeln!(
            stdout,
            "{kind} variants: {} in {} groups",
            counts.variants, counts.groups
        )?;
    }
    let quantizations = summary.quantizations();
    writeln!(
        stdout,
        "quality quantizations: {} used by no ResourceType, {} links reach no quantization",
        quantizations.unused, quantizations.unresolved_links
    )?;
    Ok(())
}

fn print_starmap(out: &mut impl Write, starmap: StarmapSummary) -> io::Result<()> {
    writeln!(
        out,
        "starmap placement: {} of {} objects have a position, {} are placed apart, {} systems, {} containers unreadable, {} missing, {} children malformed",
        starmap.placed,
        starmap.objects,
        starmap.ambiguous,
        starmap.systems,
        starmap.unreadable_containers,
        starmap.missing_containers,
        starmap.malformed_children
    )
}

fn print_paints(out: &mut impl Write, paints: PaintSummary) -> io::Result<()> {
    writeln!(
        out,
        "paint assignment: {} of {} paints fit a ship, {} fit none, {} ships have paints, {} vehicle files unreadable",
        paints.assigned, paints.paints, paints.unassigned, paints.ships, paints.unreadable_vehicles
    )
}

fn print_store_slugs(out: &mut impl Write, slugs: &StoreSlugSummary) -> io::Result<()> {
    if slugs.stale_check_skipped {
        return writeln!(
            out,
            "ship store slugs: no catalog ship matches a store slug, stale check skipped"
        );
    }
    writeln!(
        out,
        "ship store slugs: {} of {} slugs map to a ship, {} resolve, {} retired, {} unmapped, {} stale",
        slugs.mapped,
        slugs.slugs,
        slugs.resolved,
        slugs.retired,
        slugs.unmapped,
        slugs.stale.len()
    )?;
    if !slugs.stale.is_empty() {
        let shown = slugs
            .stale
            .iter()
            .take(STALE_SLUGS_SHOWN)
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join(", ");
        let more = slugs.stale.len().saturating_sub(STALE_SLUGS_SHOWN);
        if more == 0 {
            writeln!(out, "stale ship store slugs: {shown}")?;
        } else {
            writeln!(out, "stale ship store slugs: {shown} and {more} more")?;
        }
    }
    Ok(())
}

fn print_shops(out: &mut impl Write, shops: &ShopSummary) -> io::Result<()> {
    writeln!(
        out,
        "shops: {} of {} registry entries, {} malformed, {} repeated, {} with a kind, {} linked to a starmap object, {} not, {} commodity exchanges, {} containers unreadable, {} with an item list ({} ambiguous, {} unscanned, {} placeholders, {} files missing, {} files unreadable), registry {}",
        shops.shops,
        shops.registry_entries,
        shops.malformed,
        shops.duplicate,
        shops.with_kind,
        shops.linked,
        shops.unlinked,
        shops.commodity_exchanges,
        shops.containers_unreadable,
        shops.with_items,
        shops.inventory_ambiguous,
        shops.inventory_unscanned,
        shops.inventory_placeholder,
        shops.inventories_missing,
        shops.inventories_unreadable,
        if shops.registry_unreadable {
            "unreadable"
        } else if shops.registry_missing {
            "missing"
        } else {
            "read"
        }
    )
}

fn print_health(stdout: &mut impl Write, summary: &CatalogSummary) -> io::Result<()> {
    let health = summary.export_health();
    writeln!(
        stdout,
        "export health: {}; publication mode: {}",
        if health.is_complete() {
            "complete"
        } else {
            "incomplete"
        },
        match health.publication_mode {
            CatalogPublicationMode::Normal => "normal",
            CatalogPublicationMode::DiagnosticPartial => "diagnosticPartial",
        }
    )?;
    writeln!(
        stdout,
        "kind completeness describes the Filter selection scope"
    )
}

/// Prints human-readable export health and the catalog's selection and row summaries.
pub(crate) fn print(summary: &CatalogSummary, out: &Path) -> io::Result<()> {
    let mut stdout = io::stdout().lock();
    print_health(&mut stdout, summary)?;
    for kind in summary.kinds() {
        writeln!(
            stdout,
            "{}: {} written, {} skipped, {}",
            kind.kind,
            kind.count,
            kind.skipped,
            if kind.complete { "complete" } else { "partial" }
        )?;
    }
    let classes = summary.item_classes();
    writeln!(
        stdout,
        "item classes: {} read, {} other values, {} without a Class: line or English description",
        classes.classified, classes.other, classes.missing
    )?;
    print_paints(&mut stdout, summary.paints())?;
    print_loadouts(&mut stdout, summary.loadouts())?;
    print_starmap(&mut stdout, summary.starmap())?;
    print_shops(&mut stdout, summary.shops())?;
    print_store_slugs(&mut stdout, summary.store_slugs())?;
    print_classification(&mut stdout, summary)?;
    for text in summary.texts() {
        writeln!(
            stdout,
            "{}: {} texts resolved, {} unresolved, {} placeholders{}",
            text.language,
            text.resolved,
            text.unresolved,
            text.placeholder,
            if text.available {
                ""
            } else {
                ", the Archive holds no global.ini"
            }
        )?;
    }
    writeln!(
        stdout,
        "wrote {}: {}, {} Records skipped, {} failed",
        out.display(),
        size_text(summary),
        summary.skipped(),
        summary.failures().len()
    )
}

fn print_loadouts(out: &mut impl Write, loadouts: LoadoutSummary) -> io::Result<()> {
    writeln!(
        out,
        "loadouts: {} ships and {} items, {} ports, {} hold an Entity ({} docked vehicles), {} unresolved, {} Entities without an entry in items or ships, {} loadouts with issues",
        loadouts.ships,
        loadouts.items,
        loadouts.ports,
        loadouts.attached,
        loadouts.docked_vehicles,
        loadouts.unresolved,
        loadouts.outside,
        loadouts.with_issues
    )
}

fn size_text(summary: &CatalogSummary) -> String {
    summary.compressed_size().map_or_else(
        || format!("{} bytes", summary.size()),
        |compressed| {
            format!(
                "{compressed} bytes zstd-compressed from {} bytes",
                summary.size()
            )
        },
    )
}
