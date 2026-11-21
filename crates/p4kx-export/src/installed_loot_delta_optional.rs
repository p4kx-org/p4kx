// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    DecimalU64, RecordTree, SourceInventory, SourceUseRole, SourceValueState, Step, Value,
};

use crate::catalog_queries::loot::{LootDefinition, LootTableEntry};
use crate::resource_number::ResourceNumber;
use crate::source_value_catalog::{SourceNode, SourcePayload, SourceValueRef};

use super::TestResult;
use super::fixture::{DataForgeIdentity, RecordIdentity};
use super::projection::has_exact_use;

pub(super) struct OriginalOptional {
    array: u64,
    slots: Vec<OriginalSlot>,
    limits: Vec<OriginalLimit>,
}

struct OriginalSlot {
    ordinal: u64,
    kind: Option<String>,
}

#[derive(Clone, Copy)]
struct OriginalLimit {
    ordinal: u64,
    slot: u32,
    value: i32,
}

#[derive(Default)]
struct Survey {
    array: Option<(u64, usize)>,
    slots: Vec<OriginalSlot>,
    limits: Vec<OriginalLimit>,
}

impl OriginalOptional {
    pub(super) fn read(tree: &RecordTree<'_>, record: &RecordIdentity) -> TestResult<Self> {
        let mut survey = Survey::default();
        let mut read = Ok(());
        tree.root().visit_owned_values(|visit| {
            let [
                Step::Field("lootTable"),
                Step::Field("lootArchetypes"),
                Step::Element(entry),
                Step::Field("optionalData"),
                suffix @ ..,
            ] = visit.steps()
            else {
                return;
            };
            if *entry == record.entry_ordinal && read.is_ok() {
                read = survey.observe(visit.ordinal(), visit.value(), suffix);
            }
        });
        read?;
        survey.finish(record)
    }

    pub(super) fn check(
        &self,
        definition: &LootDefinition<'_>,
        view: &LootTableEntry<'_>,
        inventory: &SourceInventory,
        dataforge: &DataForgeIdentity,
        owner: u32,
    ) -> TestResult {
        let array = definition
            .value(local(self.array))
            .ok_or("original optional array missing")?;
        let Some(SourcePayload::Array { elements }) = array.value().value() else {
            return Err("original optional array was projected as another type".into());
        };
        if elements.len() != self.slots.len() {
            return Err("optional array lost authored positions".into());
        }
        for (reference, original) in elements.iter().zip(&self.slots) {
            let node = definition
                .value(*reference)
                .ok_or("original optional slot missing")?;
            if reference.value_index() != original.ordinal || !original.matches(node) {
                return Err("optional slot changed its original order or concrete type".into());
            }
        }
        for limit in &self.limits {
            check_original_limit(definition, elements, inventory, dataforge, owner, *limit)?;
        }
        self.check_limit(view, inventory, dataforge, owner)
    }

    fn check_limit(
        &self,
        view: &LootTableEntry<'_>,
        inventory: &SourceInventory,
        dataforge: &DataForgeIdentity,
        owner: u32,
    ) -> TestResult {
        let Some(original) = self.limits.first() else {
            if view.dupe_limit.is_some()
                || view.dupe_limit_source.state() != SourceValueState::Absent
            {
                return Err("the normal Loot view manufactured a duplicate limit".into());
            }
            return Ok(());
        };
        let selected = view
            .dupe_limit_source
            .value()
            .copied()
            .ok_or("duplicate-limit source link missing")?;
        if selected.value_index() != original.ordinal
            || view.dupe_limit != Some(i64::from(original.value))
        {
            return Err("duplicate-limit view disagrees with the original first slot".into());
        }
        if !has_exact_use(
            view.dupe_limit_source.origin(),
            inventory,
            dataforge,
            owner,
            original.ordinal,
            SourceUseRole::Conversion,
        ) {
            return Err("duplicate-limit view lacks its exact original field source".into());
        }
        Ok(())
    }
}

fn check_original_limit(
    definition: &LootDefinition<'_>,
    elements: &[SourceValueRef],
    inventory: &SourceInventory,
    dataforge: &DataForgeIdentity,
    owner: u32,
    original: OriginalLimit,
) -> TestResult {
    let slot =
        usize::try_from(original.slot).map_err(|_| "original optional slot not addressable")?;
    let reference = elements
        .get(slot)
        .copied()
        .ok_or("canonical authored slot missing")?;
    let node = definition
        .value(reference)
        .ok_or("original optional struct missing")?;
    let Some(SourcePayload::Struct { fields, .. }) = node.value().value() else {
        return Err("canonical optional slot lost its original struct".into());
    };
    let mut declarations = fields
        .iter()
        .filter(|field| field.name().value().copied() == Some("dupeLimit"));
    let field = declarations
        .next()
        .ok_or("original duplicate-limit declaration missing")?;
    if declarations.next().is_some() || field.value().value_index() != original.ordinal {
        return Err("canonical duplicate limit lost its original field ordinal".into());
    }
    let node = definition
        .value(field.value())
        .ok_or("original duplicate-limit value missing")?;
    if !matches!(node.value().value(), Some(SourcePayload::Number { value: ResourceNumber::Int32 { value } }) if *value == original.value)
    {
        return Err("canonical duplicate limit changed original width or value".into());
    }
    if !has_exact_use(
        node.value().origin(),
        inventory,
        dataforge,
        owner,
        original.ordinal,
        SourceUseRole::Payload,
    ) {
        return Err("canonical duplicate limit lacks its exact original field source".into());
    }
    Ok(())
}

impl OriginalSlot {
    fn matches(&self, node: &SourceNode<'_>) -> bool {
        match (self.kind.as_deref(), node.value().value()) {
            (Some(expected), Some(SourcePayload::Struct { source_type, .. })) => {
                source_type.value().copied() == Some(expected)
            }
            (None, None) => node.value().state() == SourceValueState::Null,
            _ => false,
        }
    }
}

impl Survey {
    fn observe(&mut self, ordinal: usize, value: &Value<'_>, suffix: &[Step<'_>]) -> TestResult {
        let ordinal =
            u64::try_from(ordinal).map_err(|_| "optional source ordinal not addressable")?;
        match (suffix, value) {
            ([], Value::Array(items)) if self.array.is_none() => {
                self.array = Some((ordinal, items.len()));
            }
            ([], _) => return Err("the authored optionalData is missing a readable array".into()),
            ([Step::Element(position)], source) => self.slot(*position, ordinal, source)?,
            ([Step::Element(slot), Step::Field("dupeLimit")], Value::Int32(value)) => {
                self.limits.push(OriginalLimit {
                    ordinal,
                    slot: *slot,
                    value: *value,
                });
            }
            ([Step::Element(_), Step::Field("dupeLimit")], _) => {
                return Err("the audited optional duplicate limit is not an original Int32".into());
            }
            _ => {}
        }
        Ok(())
    }

    fn slot(&mut self, position: u32, ordinal: u64, source: &Value<'_>) -> TestResult {
        if usize::try_from(position).ok() != Some(self.slots.len()) {
            return Err("original optional walk is not in authored order".into());
        }
        let kind = match source {
            Value::Struct(fields) => Some(fields.type_name().to_owned()),
            Value::Null => None,
            _ => {
                return Err(
                    "the audited optional slot has an unexpected type or read failure".into(),
                );
            }
        };
        self.slots.push(OriginalSlot { ordinal, kind });
        Ok(())
    }

    fn finish(self, record: &RecordIdentity) -> TestResult<OriginalOptional> {
        let (array, count) = self.array.ok_or("original optional array did not occur")?;
        if self.slots.len() != count {
            return Err("original optional walk did not cover the full array".into());
        }
        let mut audited = self
            .limits
            .iter()
            .filter(|limit| limit.slot == record.optional_slot);
        if audited.next().map(|limit| limit.value) != record.dupe_limit || audited.next().is_some()
        {
            return Err("original duplicate-limit member does not match the audited delta".into());
        }
        let slot =
            usize::try_from(record.optional_slot).map_err(|_| "audited slot not addressable")?;
        if record.dupe_limit.is_none() && self.slots.get(slot).is_some() {
            return Err("the removed optional occurrence is still present".into());
        }
        Ok(OriginalOptional {
            array,
            slots: self.slots,
            limits: self.limits,
        })
    }
}

const fn local(ordinal: u64) -> SourceValueRef {
    SourceValueRef {
        value_index: DecimalU64::new(ordinal),
    }
}
