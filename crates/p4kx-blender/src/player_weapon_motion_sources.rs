// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

use p4kx_assembly::ENTITY_TYPE;
use p4kx_extraction::{
    CancelToken, Record, SourceLocation, SourceScope, SourceUseRole, SourceUses, SourceValue,
    SourcedValue, StructValue, Value,
};
use schemars::JsonSchema;
use serde::Serialize;

use crate::player_source::{PlayerSourceError, check};
use crate::player_source_classes::{ClassTarget, Classes};
use crate::player_source_loadout::{Declaration, Loadout};
use crate::soc_scene_classes;

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WeaponMotionSources {
    pub(crate) weapons: Vec<Weapon>,
    pub(crate) loadouts: Vec<LoadoutLink>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Weapon {
    record: SourcedValue<u32>,
    pub(crate) component: SourcedValue<usize>,
    guid: SourcedValue<String>,
    item_type: SourcedValue<String>,
    subtype: SourcedValue<String>,
    pub(crate) tags: SourcedValue<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LoadoutLink {
    loadout: SourcedValue<usize>,
    declaration: SourcedValue<usize>,
    ambiguous_target: SourcedValue<bool>,
    weapons: Vec<SourcedValue<usize>>,
}

struct Reader<'a> {
    indices: BTreeMap<u32, Vec<usize>>,
    output: Vec<Weapon>,
    sources: &'a SourceUses,
    scope: &'a SourceScope,
    cancel: &'a CancelToken,
}

pub(crate) fn read(
    loadouts: &[SourcedValue<Loadout>],
    classes: Classes<'_, '_>,
    sources: &SourceUses,
    cancel: &CancelToken,
) -> Result<WeaponMotionSources, PlayerSourceError> {
    let Some((entities, scope)) = classes else {
        return Ok(WeaponMotionSources::default());
    };
    let mut reader = Reader {
        indices: BTreeMap::new(),
        output: Vec::new(),
        sources,
        scope,
        cancel,
    };
    let mut links = Vec::new();
    for (loadout, value) in loadouts.iter().enumerate() {
        check(cancel)?;
        let Some(Loadout::Present { declarations, .. }) = value.value() else {
            continue;
        };
        for (declaration, option) in declarations.iter().enumerate() {
            check(cancel)?;
            let Declaration::Item { target, .. } = option else {
                continue;
            };
            let Some(class) = target.value() else {
                continue;
            };
            let (records, ambiguous_target) = match class {
                ClassTarget::Matched { record } => (std::slice::from_ref(record), false),
                ClassTarget::Ambiguous { records } => (records.as_slice(), true),
                _ => continue,
            };
            let mut weapons = Vec::new();
            for record in records {
                let index = record.record_index();
                if let Some(record) = entities.forge().record(index)
                    && matches!(record.type_name(), Ok(ENTITY_TYPE))
                {
                    weapons.extend_from_slice(reader.weapon(&record)?);
                }
            }
            if !weapons.is_empty() {
                links.push(LoadoutLink {
                    loadout: value.map_ref(|_| loadout),
                    declaration: target.map_ref(|_| declaration),
                    ambiguous_target: target.map_ref(|_| ambiguous_target),
                    weapons: reader.links(weapons, target)?,
                });
            }
        }
    }
    Ok(WeaponMotionSources {
        weapons: reader.output,
        loadouts: links,
    })
}

impl Reader<'_> {
    fn links(
        &self,
        weapons: Vec<usize>,
        target: &SourcedValue<ClassTarget>,
    ) -> Result<Vec<SourcedValue<usize>>, PlayerSourceError> {
        weapons
            .into_iter()
            .map(|weapon| {
                let original = self.output.get(weapon).ok_or(PlayerSourceError::Binding)?;
                Ok(self.sources.value_with_origins(
                    SourceValue::Present(weapon),
                    &[target.origin(), original.component.origin()],
                )?)
            })
            .collect()
    }

    fn weapon(&mut self, record: &Record<'_>) -> Result<&[usize], PlayerSourceError> {
        let index = record.index();
        if let std::collections::btree_map::Entry::Vacant(slot) = self.indices.entry(index) {
            let first = self.output.len();
            if let Ok(tree) = record.tree()
                && let Some(Value::Array(components)) = tree.field("Components")
            {
                for (component, candidate) in components.iter().enumerate() {
                    check(self.cancel)?;
                    if let Some(definition) = personal_weapon(candidate) {
                        let reader = Definition {
                            sources: self.sources,
                            scope: self.scope,
                            index,
                            component,
                        };
                        self.output.push(Weapon {
                            record: reader.coordinate(index, "$guid")?,
                            component: reader
                                .coordinate(component, &format!("Components[{component}]"))?,
                            guid: reader.coordinate(record.guid().to_string(), "$guid")?,
                            item_type: reader.field(definition, "Type")?,
                            subtype: reader.field(definition, "SubType")?,
                            tags: reader.field(definition, "Tags")?,
                        });
                    }
                }
            }
            slot.insert((first..self.output.len()).collect());
        }
        self.indices
            .get(&index)
            .map(Vec::as_slice)
            .ok_or(PlayerSourceError::Binding)
    }
}

fn personal_weapon<'a, 'forge>(value: &'a Value<'forge>) -> Option<&'a StructValue<'forge>> {
    let Value::Struct(component) = value else {
        return None;
    };
    if component.type_name() != "SAttachableComponentParams" {
        return None;
    }
    let Value::Struct(definition) = component.field("AttachDef")? else {
        return None;
    };
    match definition.field("Type") {
        Some(Value::String(value) | Value::Enum(value))
            if value.eq_ignore_ascii_case("WeaponPersonal") =>
        {
            Some(definition)
        }
        _ => None,
    }
}

struct Definition<'a> {
    sources: &'a SourceUses,
    scope: &'a SourceScope,
    index: u32,
    component: usize,
}

impl Definition<'_> {
    fn coordinate<T>(&self, value: T, field: &str) -> Result<SourcedValue<T>, PlayerSourceError> {
        self.sourced(SourceValue::Present(value), field)
    }

    fn field(
        &self,
        definition: &StructValue<'_>,
        name: &str,
    ) -> Result<SourcedValue<String>, PlayerSourceError> {
        let raw = definition.field(name);
        let value = match raw {
            Some(Value::Enum(raw)) => SourceValue::Present((*raw).to_owned()),
            _ => soc_scene_classes::string(raw),
        };
        self.sourced(
            value,
            &format!("Components[{}].AttachDef.{name}", self.component),
        )
    }

    fn sourced<T>(
        &self,
        value: SourceValue<T>,
        field: &str,
    ) -> Result<SourcedValue<T>, PlayerSourceError> {
        let complete = !matches!(value, SourceValue::Unreadable);
        let source = self.scope.at(
            SourceLocation::DataForge {
                record_index: self.index,
                field: field.to_owned(),
            },
            SourceUseRole::Payload,
        )?;
        Ok(self.sources.value(value, &[source], complete))
    }
}
