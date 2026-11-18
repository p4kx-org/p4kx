// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    LocalizationEntry, RecordTree, SourceValueState, Step, StructValue, Value, is_placeholder_key,
};

use crate::catalog_queries::equipment::ItemSource;
use crate::{ItemClass, Playability};

use super::{Graph, TestResult, location};

const ATTACHMENT: &str = "SAttachableComponentParams";
const PHYSICS: &str = "SEntityPhysicsControllerParams";
const PURCHASABLE: &str = "SCItemPurchasableParams";
const MISSION_PATHS: [&str; 2] = [
    "libs/foundry/records/entities/scitem/mission_entities/",
    "libs/foundry/records/entities/scitem/missionstorage/",
];

pub(super) struct OriginalItem<'record, 'source> {
    pub(super) attachment_index: u32,
    physics_index: Option<u32>,
    purchasable_index: Option<u32>,
    localization: &'record StructValue<'source>,
    shop: Option<&'source str>,
    path: &'source str,
}

impl<'record, 'source> OriginalItem<'record, 'source> {
    pub(super) fn read(tree: &'record RecordTree<'source>) -> TestResult<Self> {
        let Some(Value::Array(components)) = tree.field("Components") else {
            return Err("selected Item must have an authored component array".into());
        };
        if components
            .iter()
            .any(|value| matches!(value, Value::Error(_)))
        {
            return Err("selected witness requires readable component positions".into());
        }
        let (attachment_index, attachment) =
            first(components, ATTACHMENT)?.ok_or("original attachment missing")?;
        let physics = first(components, PHYSICS)?;
        let purchasable = first(components, PURCHASABLE)?;
        let localization = structure(attachment, "AttachDef")
            .and_then(|definition| structure(definition, "Localization"))
            .ok_or("original attachment Locale definition missing")?;
        Ok(Self {
            attachment_index,
            physics_index: physics.map(|(index, _)| index),
            purchasable_index: purchasable.map(|(index, _)| index),
            localization,
            shop: purchasable.and_then(|(_, item)| locale_key(item, "displayName")),
            path: tree.record_path(),
        })
    }

    pub(super) fn check_values(
        &self,
        row: &ItemSource<'_>,
        english: &LocalizationEntry,
    ) -> TestResult {
        let description = locale_key(self.localization, "Description")
            .ok_or("selected witness requires an authored description key")?;
        let ini = english
            .parse()
            .map_err(|_| "original English index failed")?;
        let display = ini
            .resolve(description)
            .ok_or("selected witness requires an English description")?
            .replace("\\n", "\n");
        let class = display
            .lines()
            .find_map(|line| line.trim_start().strip_prefix("Class:"))
            .and_then(|class| ItemClass::from_name(class.trim()));
        let playability = if self.shop.is_some() {
            Some(Playability::Player)
        } else if MISSION_PATHS.iter().any(|prefix| {
            self.path
                .get(..prefix.len())
                .is_some_and(|path| path.eq_ignore_ascii_case(prefix))
        }) {
            Some(Playability::Mission)
        } else if locale_key(self.localization, "Name").is_none() {
            Some(Playability::Special)
        } else {
            None
        };
        let selection = row.selection();
        check_cell(
            selection.attachment_component_ordinal(),
            Some(self.attachment_index),
        )?;
        check_cell(selection.physics_component_ordinal(), self.physics_index)?;
        check_cell(
            selection.purchasable_component_ordinal(),
            self.purchasable_index,
        )?;
        check_cell(row.item_class(), class)?;
        check_cell(row.playability(), playability)
    }

    pub(super) fn check_coordinates(
        &self,
        tree: &RecordTree<'_>,
        row: &serde_json::Value,
        graph: &Graph<'_>,
    ) -> TestResult {
        for (index, field) in [
            (Some(self.attachment_index), "attachmentComponentOrdinal"),
            (self.physics_index, "physicsComponentOrdinal"),
            (self.purchasable_index, "purchasableComponentOrdinal"),
        ] {
            let Some(index) = index else { continue };
            let location = location(tree, &[Step::Field("Components"), Step::Element(index)])?;
            let cell = row
                .get("selection")
                .and_then(|selection| selection.get(field))
                .ok_or("original component selection cell missing")?;
            graph.coordinate(cell, &location)?;
        }
        Ok(())
    }
}

fn check_cell<T: Copy + PartialEq>(
    cell: &p4kx_extraction::SourcedValue<T>,
    expected: Option<T>,
) -> TestResult {
    let state = if expected.is_some() {
        SourceValueState::Present
    } else {
        SourceValueState::Absent
    };
    if cell.state() != state || cell.value().copied() != expected {
        return Err("Item cell differs from its independently observed original field".into());
    }
    Ok(())
}

fn first<'record, 'source>(
    components: &'record [Value<'source>],
    name: &str,
) -> TestResult<Option<(u32, &'record StructValue<'source>)>> {
    for (index, value) in components.iter().enumerate() {
        if let Value::Struct(component) = value
            && component.type_name() == name
        {
            return Ok(Some((u32::try_from(index)?, component)));
        }
    }
    Ok(None)
}

fn structure<'record, 'source>(
    node: &'record StructValue<'source>,
    name: &str,
) -> Option<&'record StructValue<'source>> {
    match node.field(name) {
        Some(Value::Struct(value)) => Some(value),
        _ => None,
    }
}

fn locale_key<'source>(node: &StructValue<'source>, name: &str) -> Option<&'source str> {
    match node.field(name) {
        Some(Value::Locale(token)) if !token.is_empty() && !is_placeholder_key(token) => {
            Some(*token)
        }
        _ => None,
    }
}
