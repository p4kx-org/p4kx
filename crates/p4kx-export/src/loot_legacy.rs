// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    CancelToken, Record, RecordTree, SourceUseRole, SourceValue, SourcedValue, Value,
};

use crate::CatalogError;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_v3::loot_definition_catalog::read_projected;
use crate::contract_loot_legacy::{
    LegacyLootArchetype, LegacyLootEntry, LegacyLootQuantity, LegacyLootTable,
    LegacyLootTableEntry, LegacySecondaryEntry, LegacySecondaryGroup, LegacySpawnWith,
};

#[path = "legacy_loot_fields.rs"]
mod legacy_loot_fields;

use legacy_loot_fields::{Fact, Fields, Slots};

const STACK_SIZE: &str = "EntryOptionalData_StackSize";
const SPAWN_WITH: &str = "EntryOptionalData_SpawnWith";

pub(crate) fn legacy_table<'source>(
    record: &Record<'source>,
    tree: &RecordTree<'source>,
    input: &OriginalCatalogInput<'_, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<LegacyLootTable<'source>, CatalogError> {
    let (original, entries) = read_projected(record, tree, input, cancel, |facts| {
        let read = Fields { facts };
        read.slots(None, "lootArchetypes", |slot| {
            read.cell(slot, |slot| {
                if !matches!(slot.value(), Value::Struct(_)) {
                    return Ok(None);
                }
                read.table_entry(slot).map(Some)
            })
        })
    })?;
    Ok(LegacyLootTable { original, entries })
}

pub(crate) fn legacy_archetype<'source>(
    record: &Record<'source>,
    tree: &RecordTree<'source>,
    input: &OriginalCatalogInput<'_, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<LegacyLootArchetype<'source>, CatalogError> {
    let (original, (excluded_tag_guids, entries, secondary_groups)) =
        read_projected(record, tree, input, cancel, |facts| {
            let read = Fields { facts };
            let excluded =
                read.nested(None, "excludedTags", |tags| read.references(tags, "tags"))?;
            let primary = read.nested(None, "primaryOrGroup", |group| {
                read.slots(Some(group), "entries", |slot| {
                    read.cell(slot, |slot| read.primary_entry(slot))
                })
            })?;
            let secondary = read.slots(None, "secondaryOrGroups", |slot| {
                read.cell(slot, |slot| read.secondary_group(slot))
            })?;
            Ok((excluded, primary, secondary))
        })?;
    Ok(LegacyLootArchetype {
        original,
        excluded_tag_guids,
        entries,
        secondary_groups,
    })
}

impl<'scope, 'read, 'source> Fields<'_, 'scope, 'read, 'source> {
    fn nested<T>(
        &self,
        owner: Option<&Fact<'scope, 'read, 'source>>,
        field: &str,
        project: impl FnOnce(&Fact<'scope, 'read, 'source>) -> Result<SourcedValue<T>, CatalogError>,
    ) -> Result<SourcedValue<T>, CatalogError> {
        let Some((_, nested)) = self.facts.field(owner, field)? else {
            return self.absent(owner);
        };
        if let Value::Struct(_) = nested.value() {
            project(nested)
        } else {
            self.cell(nested, |_| Ok(None))
        }
    }

    fn table_entry(
        &self,
        entry: &Fact<'scope, 'read, 'source>,
    ) -> Result<LegacyLootTableEntry, CatalogError> {
        let limit = |name| {
            self.nested(Some(entry), "numberOfResultsConstraints", |constraints| {
                self.integer(constraints, name)
            })
        };
        Ok(LegacyLootTableEntry {
            weight: self.number(entry, "weight")?,
            archetype_guid: self.reference(entry, "archetype")?,
            min_results: limit("minResults")?,
            max_results: limit("maxResults")?,
        })
    }

    fn primary_entry(
        &self,
        entry: &Fact<'scope, 'read, 'source>,
    ) -> Result<Option<LegacyLootEntry<'source>>, CatalogError> {
        if !matches!(entry.value(), Value::Struct(_)) {
            return Ok(None);
        }
        let additional = |name| {
            self.nested(Some(entry), "additionalTags", |tags| {
                self.references(tags, name)
            })
        };
        Ok(Some(LegacyLootEntry {
            name: self.label(entry, "name")?,
            weight: self.number(entry, "weight")?,
            tag_guid: self.reference(entry, "tag")?,
            additional_positive_tag_guids: additional("positiveTags")?,
            additional_negative_tag_guids: additional("negativeTags")?,
            stack_size: self.stack_size(entry)?,
            spawn_with: self.spawn_entries(entry)?,
        }))
    }

    fn stack_size(
        &self,
        entry: &Fact<'scope, 'read, 'source>,
    ) -> Result<SourcedValue<LegacyLootQuantity>, CatalogError> {
        let Some((_, array)) = self.facts.field(Some(entry), "optionalData")? else {
            return self.absent(Some(entry));
        };
        if !matches!(array.value(), Value::Array(_)) {
            return self.cell(array, |_| Ok(None));
        }
        let mut selectors = vec![Fields::selection_use(array)?];
        for slot in self.facts.elements(array)? {
            selectors.push(Fields::selection_use(slot)?);
            if matches!(slot.value(), Value::Struct(fields) if fields.type_name() == STACK_SIZE) {
                self.inspect_selector(slot, "min", &mut selectors)?;
                self.inspect_selector(slot, "max", &mut selectors)?;
                let min = self.number(slot, "min")?;
                let max = self.number(slot, "max")?;
                if min.value().is_some() || max.value().is_some() {
                    return self.bind_selected(
                        slot,
                        SourceValue::Present(LegacyLootQuantity { min, max }),
                        selectors,
                    );
                }
            }
        }
        let state = if selectors
            .iter()
            .any(|used| used.role() == SourceUseRole::Attempted)
        {
            SourceValue::Unreadable
        } else {
            SourceValue::Absent
        };
        self.bind_selected(array, state, selectors)
    }

    fn spawn_entries(
        &self,
        entry: &Fact<'scope, 'read, 'source>,
    ) -> Result<Slots<LegacySpawnWith<'source>>, CatalogError> {
        let Some((_, array)) = self.facts.field(Some(entry), "optionalData")? else {
            return self.absent(Some(entry));
        };
        if !matches!(array.value(), Value::Array(_)) {
            return self.cell(array, |_| Ok(None));
        }
        let mut rows = Vec::new();
        let mut selectors = Vec::new();
        for slot in self.facts.elements(array)? {
            selectors.push(Fields::selection_use(slot)?);
            if matches!(slot.value(), Value::Struct(fields) if fields.type_name() == SPAWN_WITH) {
                rows.push(self.bind(slot, SourceValue::Present(self.spawn_with(slot)?))?);
            }
        }
        self.bind_selected(array, SourceValue::Present(rows), selectors)
    }
    fn spawn_with(
        &self,
        entry: &Fact<'scope, 'read, 'source>,
    ) -> Result<LegacySpawnWith<'source>, CatalogError> {
        let tags = |name| {
            self.nested(Some(entry), "tagsToMatch", |tags| {
                self.references(tags, name)
            })
        };
        Ok(LegacySpawnWith {
            name: self.label(entry, "name")?,
            mode: self.label(entry, "mode")?,
            min: self.number(entry, "min")?,
            max: self.number(entry, "max")?,
            positive_tag_guids: tags("positiveTags")?,
            negative_tag_guids: tags("negativeTags")?,
        })
    }

    fn secondary_group(
        &self,
        group: &Fact<'scope, 'read, 'source>,
    ) -> Result<Option<LegacySecondaryGroup<'source>>, CatalogError> {
        if !matches!(group.value(), Value::Struct(_)) {
            return Ok(None);
        }
        let entries = self.slots(Some(group), "entries", |entry| {
            self.cell(entry, |entry| {
                if !matches!(entry.value(), Value::Struct(_)) {
                    return Ok(None);
                }
                Ok(Some(LegacySecondaryEntry {
                    weight: self.number(entry, "weight")?,
                    tag_guid: self.reference(entry, "tag")?,
                }))
            })
        })?;
        Ok(Some(LegacySecondaryGroup {
            group_name: self.label(group, "groupName")?,
            entries,
        }))
    }
}
