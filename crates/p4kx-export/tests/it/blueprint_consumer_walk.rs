// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::{
    BlueprintCost, BlueprintCostContext, BlueprintCostMeaning, BlueprintCostPosition,
    BlueprintDefinition, BlueprintModifierRange, BlueprintPresentation, BlueprintRangeKind,
    BlueprintResearch, BlueprintResearchPointer, BlueprintTier, CatalogGuid, PhysicalBlueprintRef,
    SourceArenaOutput, SourceValueRef,
};
use p4kx_extraction::{SourceInventory, SourcedValue};
use serde_json::Value;

use crate::canonical_header::wire::Wire;

use crate::canonical_header::originals;

pub(crate) type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

/// Checks the physical Blueprint header and complete original arena through public getters.
pub(crate) fn visit_definition(
    row: &BlueprintDefinition<'_>,
    inventory: &SourceInventory,
    expected: &Value,
) -> TestResult {
    let wire = Wire::new(expected, inventory);
    wire.field("inputRef")?.raw(row.input_ref().as_str())?;
    assert_eq!(
        inventory
            .iter_sources()
            .filter(|source| source.id() == row.input_ref())
            .count(),
        1,
        "definition input resolves in its inventory"
    );
    let header = row.header();
    wire.field("guid")?.guid(&header.guid())?;
    wire.field("recordIndex")?.leaf(header.record_index())?;
    for (field, cell) in [
        ("recordName", header.record_name()),
        ("className", header.class_name()),
        ("recordPath", header.record_path()),
        ("sourceType", header.source_type()),
        ("fileName", header.file_name()),
        ("metadataTag", header.metadata_tag()),
    ] {
        wire.field(field)?.leaf(cell)?;
    }
    originals::visit_arena(row.source(), wire.field("source")?)
}

/// Checks a complete tier while resolving every navigation link in its physical owner.
pub(crate) fn visit_tier(
    tier: &BlueprintTier,
    owner: &BlueprintDefinition<'_>,
    inventory: &SourceInventory,
    expected: &Value,
) -> TestResult {
    let wire = Wire::new(expected, inventory);
    let arena = owner.source();
    visit_physical(tier.blueprint(), owner, wire.field("blueprint")?)?;
    wire.field("tierIndex")?.leaf(tier.tier_index())?;
    originals::visit_ref(tier.source(), arena, wire.field("source")?)?;
    visit_integer(tier.craft_time_seconds(), wire.field("craftTimeSeconds")?)?;
    optional_ref(tier.mandatory(), arena, wire.field("mandatory")?)?;
    refs(tier.optional(), arena, wire.field("optional")?)?;
    let costs = wire.field("costs")?;
    costs.len(tier.costs().len())?;
    for (index, cost) in tier.costs().iter().enumerate() {
        visit_cost(cost, arena, costs.index(index)?)?;
    }
    visit_research(tier.research(), arena, wire.field("research")?)
}

/// Checks the finalized presentation without resolving foreign Blueprint or item bodies.
pub(crate) fn visit_presentation(
    row: &BlueprintPresentation<SourcedValue<CatalogGuid>>,
    owner: &BlueprintDefinition<'_>,
    inventory: &SourceInventory,
    expected: &Value,
) -> TestResult {
    let wire = Wire::new(expected, inventory);
    let arena = owner.source();
    visit_physical(row.blueprint(), owner, wire.field("blueprint")?)?;
    wire.field("name")?
        .cell(row.name(), |name, wire| wire.raw(name.as_str()))?;
    wire.field("category")?.cell(row.category(), |guid, wire| {
        wire.raw(&guid.get().to_string())
    })?;
    wire.field("producedItem")?
        .cell(row.produced_item(), |guid, wire| {
            wire.raw(&guid.get().to_string())
        })?;
    optional_ref(row.first_tier(), arena, wire.field("firstTier")?)?;
    let ingredients = wire.field("ingredients")?;
    ingredients.len(row.ingredients().len())?;
    for (index, ingredient) in row.ingredients().iter().enumerate() {
        let wire = ingredients.index(index)?;
        originals::visit_ref(ingredient.cost(), arena, wire.field("cost")?)?;
        optional_ref(ingredient.slot(), arena, wire.field("slot")?)?;
    }
    wire.field("defaultAvailable")?
        .leaf(row.default_available())?;
    wire.field("dismantle")?
        .cell(row.dismantle(), |dismantle, wire| {
            wire.field("processGuid")?
                .raw(&dismantle.process_guid().get().to_string())?;
            optional_ref(dismantle.tier(), arena, wire.field("tier")?)
        })
}

fn visit_physical(
    reference: &PhysicalBlueprintRef,
    owner: &BlueprintDefinition<'_>,
    wire: Wire<'_>,
) -> TestResult {
    wire.field("inputRef")?
        .raw(reference.input_ref().as_str())?;
    wire.field("recordIndex")?.raw(&reference.record_index())?;
    assert_eq!(
        reference.input_ref(),
        owner.input_ref(),
        "navigation retains its physical Blueprint input"
    );
    assert_eq!(
        Some(reference.record_index()),
        owner.header().record_index().value().copied(),
        "navigation retains its physical Blueprint Record"
    );
    Ok(())
}

fn visit_cost(cost: &BlueprintCost, arena: &SourceArenaOutput<'_>, wire: Wire<'_>) -> TestResult {
    originals::visit_ref(cost.source(), arena, wire.field("source")?)?;
    wire.field("position")?
        .cell(cost.position(), |position, wire| match position {
            BlueprintCostPosition::Mandatory => wire.tag("mandatory"),
            BlueprintCostPosition::Optional { index } => {
                wire.tag("optional")?;
                wire.field("index")?.raw(index)
            }
            BlueprintCostPosition::Option { parent, index } => {
                wire.tag("option")?;
                originals::visit_ref(*parent, arena, wire.field("parent")?)?;
                wire.field("index")?.raw(index)
            }
            _ => Err("new cost positions require a typed traversal".into()),
        })?;
    visit_context(cost.context(), arena, wire.field("context")?)?;
    visit_meaning(cost.meaning(), arena, wire.field("meaning")?)
}

fn visit_meaning(
    meaning: &BlueprintCostMeaning,
    arena: &SourceArenaOutput<'_>,
    wire: Wire<'_>,
) -> TestResult {
    match meaning {
        BlueprintCostMeaning::Select {
            count,
            debug_name,
            display_name,
            options,
        } => {
            wire.tag("select")?;
            visit_integer(count, wire.field("count")?)?;
            optional_ref(*debug_name, arena, wire.field("debugName")?)?;
            wire.field("displayName")?
                .cell(display_name, |name, wire| wire.raw(name.as_str()))?;
            refs(options, arena, wire.field("options")?)
        }
        BlueprintCostMeaning::Resource {
            resource_guid,
            quantity_scu,
            min_quality,
        } => {
            wire.tag("resource")?;
            wire.field("resourceGuid")?
                .cell(resource_guid, |guid, wire| {
                    wire.raw(&guid.get().to_string())
                })?;
            wire.field("quantityScu")?.leaf(quantity_scu)?;
            visit_integer(min_quality, wire.field("minQuality")?)
        }
        BlueprintCostMeaning::Item {
            item_guid,
            quantity_units,
            min_quality,
        } => {
            wire.tag("item")?;
            wire.field("itemGuid")?
                .cell(item_guid, |guid, wire| wire.raw(&guid.get().to_string()))?;
            visit_integer(quantity_units, wire.field("quantityUnits")?)?;
            visit_integer(min_quality, wire.field("minQuality")?)
        }
        _ => Err("new cost meanings require a typed traversal".into()),
    }
}

fn visit_context(
    context: &BlueprintCostContext,
    arena: &SourceArenaOutput<'_>,
    wire: Wire<'_>,
) -> TestResult {
    optional_ref(context.source(), arena, wire.field("source")?)?;
    if let Some(multiplier) = context.quantity_multiplier() {
        wire.field("quantityMultiplier")?.leaf(multiplier)?;
    } else {
        wire.field("quantityMultiplier")?.raw(&Option::<u8>::None)?;
    }
    optional_ref(
        context.composition_inclusion(),
        arena,
        wire.field("compositionInclusion")?,
    )?;
    let modifiers = wire.field("modifiers")?;
    modifiers.len(context.modifiers().len())?;
    for (index, modifier) in context.modifiers().iter().enumerate() {
        let wire = modifiers.index(index)?;
        originals::visit_ref(modifier.source(), arena, wire.field("source")?)?;
        wire.field("propertyGuid")?
            .cell(modifier.property_guid(), |guid, wire| {
                wire.raw(&guid.get().to_string())
            })?;
        let ranges = wire.field("ranges")?;
        ranges.len(modifier.ranges().len())?;
        for (index, range) in modifier.ranges().iter().enumerate() {
            visit_range(range, arena, ranges.index(index)?)?;
        }
    }
    Ok(())
}

fn visit_range(
    range: &BlueprintModifierRange,
    arena: &SourceArenaOutput<'_>,
    wire: Wire<'_>,
) -> TestResult {
    originals::visit_ref(range.source(), arena, wire.field("source")?)?;
    wire.field("kind")?.raw(match range.kind() {
        BlueprintRangeKind::Multiplier => "multiplier",
        BlueprintRangeKind::Additive => "additive",
        _ => return Err("new range kinds require a typed traversal".into()),
    })?;
    visit_integer(range.start_quality(), wire.field("startQuality")?)?;
    visit_integer(range.end_quality(), wire.field("endQuality")?)?;
    wire.field("atStart")?.leaf(range.at_start())?;
    wire.field("atEnd")?.leaf(range.at_end())
}

fn visit_research(
    research: &BlueprintResearch,
    arena: &SourceArenaOutput<'_>,
    wire: Wire<'_>,
) -> TestResult {
    optional_ref(research.source(), arena, wire.field("source")?)?;
    wire.field("envelope")?
        .cell(research.envelope(), |envelope, wire| {
            visit_pointer(
                envelope.research_costs(),
                arena,
                wire.field("researchCosts")?,
            )?;
            visit_pointer(
                envelope.unlock_requirements(),
                arena,
                wire.field("unlockRequirements")?,
            )
        })
}

fn visit_pointer(
    pointer: &BlueprintResearchPointer,
    arena: &SourceArenaOutput<'_>,
    wire: Wire<'_>,
) -> TestResult {
    optional_ref(pointer.source(), arena, wire.field("source")?)?;
    wire.field("declaredBase")?.leaf(pointer.declared_base())?;
    wire.field("value")?
        .cell(pointer.value(), |reference, wire| {
            originals::visit_ref(*reference, arena, wire)
        })
}

fn visit_integer(
    value: &SourcedValue<p4kx_export::catalog_queries::ResourceInt64>,
    wire: Wire<'_>,
) -> TestResult {
    wire.cell(value, |value, wire| wire.raw(&value.get().to_string()))
}

fn refs(
    references: &[SourceValueRef],
    arena: &SourceArenaOutput<'_>,
    wire: Wire<'_>,
) -> TestResult {
    wire.len(references.len())?;
    for (index, reference) in references.iter().enumerate() {
        originals::visit_ref(*reference, arena, wire.index(index)?)?;
    }
    Ok(())
}

fn optional_ref(
    reference: Option<SourceValueRef>,
    arena: &SourceArenaOutput<'_>,
    wire: Wire<'_>,
) -> TestResult {
    reference.map_or_else(
        || wire.raw(&reference),
        |reference| originals::visit_ref(reference, arena, wire),
    )
}
