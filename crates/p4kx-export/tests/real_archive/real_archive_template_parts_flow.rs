// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{HashMap, HashSet};

use serde_json::Value;

use super::{Figures, PartsResult, nullable, required};
use crate::real_archive_contract_templates::mission_graph::{self as graph, Mission};

type OwnerKey<'catalog> = (&'catalog str, u64, Option<&'catalog str>);

pub(super) struct Flows<'catalog> {
    rows: HashMap<OwnerKey<'catalog>, &'catalog Value>,
}

impl<'catalog> Flows<'catalog> {
    pub(super) fn read(catalog: &'catalog Value) -> PartsResult<Self> {
        let table = catalog
            .pointer("/configurations/missionFlows")
            .ok_or("canonical mission flow table")?;
        let configurations = graph::array(table, "rows")?;
        match graph::text(table, "/availability")? {
            "available" => {}
            "absent" if configurations.is_empty() => {}
            _ => return Err("canonical flow source availability".into()),
        }
        let mut rows = HashMap::new();
        for row in configurations {
            if graph::text(row, "/owner/type")? == "contractTemplate"
                && rows.insert(owner_key(row)?, row).is_some()
            {
                return Err("one flow configuration per original Template declaration".into());
            }
        }
        Ok(Self { rows })
    }

    pub(super) fn count(
        &self,
        mission: &Mission<'catalog>,
        name: &str,
        figures: &mut Figures,
    ) -> PartsResult {
        let ids = objective_ids(mission)?;
        for link in graph::array(mission.definition, "missionFlows")? {
            let key = owner_key(link)?;
            let row = self
                .rows
                .get(&key)
                .ok_or("actual flow configuration for the D link")?;
            if graph::text(link, "/owner/type")? != "contractTemplate"
                || required(link, "owner")? != required(row, "owner")?
                || graph::text(link, "/owner/inputRef")?
                    != graph::text(mission.definition, "/inputRef")?
                || link.pointer("/owner/recordIndex")
                    != mission.definition.pointer("/recordIndex/value")
                || graph::text(link, "/owner/recordGuid")? != graph::guid(mission.definition)?
            {
                return Err("the flow declaration belongs to this actual Template".into());
            }
            match graph::text(row, "/selection/type")? {
                "selected" => {
                    count_triggers(mission, graph::array(row, "triggers")?, name, &ids, figures)?;
                }
                "absent" | "shadowedDeclaration" | "noStruct" => {}
                _ => return Err("known Flow selection state".into()),
            }
        }
        Ok(())
    }
}

fn owner_key(row: &Value) -> PartsResult<OwnerKey<'_>> {
    let position = row
        .pointer("/owner/recordIndex")
        .and_then(Value::as_u64)
        .ok_or("original flow owner Record index")?;
    let declaration = nullable(row, "declarationIndex")?
        .map(|value| value.as_str().ok_or("original flow declaration ordinal"))
        .transpose()?;
    Ok((graph::text(row, "/owner/inputRef")?, position, declaration))
}

fn objective_ids<'catalog>(mission: &Mission<'catalog>) -> PartsResult<HashSet<&'catalog str>> {
    graph::array(mission.projection, "objectives")?
        .iter()
        .map(|objective| guid(mission.field(required(objective, "id")?)?))
        .collect()
}

fn guid(node: &Value) -> PartsResult<&str> {
    if graph::text(node, "/value/value/type")? != "guid" {
        return Err("actual original objective GUID".into());
    }
    Ok(graph::text(node, "/value/value/value")?)
}

fn count_triggers<'catalog>(
    mission: &Mission<'catalog>,
    triggers: &[Value],
    name: &str,
    ids: &HashSet<&'catalog str>,
    figures: &mut Figures,
) -> PartsResult {
    let mut links = Links {
        mission,
        name,
        ids,
        figures,
    };
    for reference in triggers {
        links.figures.flow_triggers += 1;
        let trigger = required(mission.follow(reference)?, "trigger")?;
        if let Some(condition) = nullable(trigger, "condition")? {
            links.conditions(condition)?;
        }
        if let Some(action) = nullable(trigger, "action")? {
            links.action(action)?;
        }
    }
    Ok(())
}

struct Links<'read, 'catalog> {
    mission: &'read Mission<'catalog>,
    name: &'read str,
    ids: &'read HashSet<&'catalog str>,
    figures: &'read mut Figures,
}

impl Links<'_, '_> {
    fn objective(&mut self, meaning: &Value) -> PartsResult {
        let binding = required(meaning, "objective")?;
        if graph::text(binding, "/interpretation")? == "id" {
            let id = guid(self.mission.field(required(binding, "field")?)?)?;
            self.figures.flow_objective_links += 1;
            assert!(
                self.ids.contains(id),
                "{}: flow names objective {id}",
                self.name
            );
        } else {
            self.figures.unresolved_objective_links += 1;
        }
        Ok(())
    }

    fn conditions(&mut self, first: &Value) -> PartsResult {
        let mut pending = vec![first];
        while let Some(reference) = pending.pop() {
            let condition = required(self.mission.follow(reference)?, "condition")?;
            match graph::text(condition, "/type")? {
                "tokenState" => self.objective(condition)?,
                "and" | "or" => pending.extend(graph::array(condition, "children")?.iter().rev()),
                "not" => pending.extend(nullable(condition, "child")?),
                "allTokensState" | "anyTokensState" | "onMissionStart" | "propertyIsSet" => {}
                _ => return Err("known selected mission flow condition".into()),
            }
        }
        Ok(())
    }

    fn action(&mut self, reference: &Value) -> PartsResult {
        let mut pending = vec![reference];
        while let Some(reference) = pending.pop() {
            let node = self.mission.follow(reference)?;
            if node.pointer("/value/value/type").and_then(Value::as_str) != Some("struct") {
                continue;
            }
            let action = required(node, "action")?;
            match graph::text(action, "/type")? {
                "tokenStateChange" => self.objective(action)?,
                "pickRandomMissionPhase" => {
                    for reference in graph::array(action, "phases")? {
                        self.objective(required(self.mission.follow(reference)?, "randomPhase")?)?;
                    }
                }
                "multipleActions" => {
                    pending.extend(graph::array(action, "actions")?.iter().rev());
                }
                "pickRandomAction" => {
                    for entry in graph::array(action, "entries")?.iter().rev() {
                        if graph::text(entry, "/type")? == "declared" {
                            pending.extend(nullable(entry, "action")?);
                        }
                    }
                }
                "stateChange" => {}
                _ => return Err("known selected mission flow action".into()),
            }
        }
        Ok(())
    }
}
