// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;
use std::fmt;

use p4kx_export::mining_locations::{
    ContainerInstanceKey, GameTextRef, HarvestableOccurrenceRoute, MiningLocationSnapshot,
    PublicNameRef,
};
use serde::de::{DeserializeSeed, SeqAccess, Visitor};
use serde_json::Value;

use super::TestResult;
use crate::catalog_capture::CatalogCapture;

pub(super) const CONTAINERS: &str = "/placements/containerInstances/rows";
pub(super) const PUBLIC_NAMES: &str = "/texts/publicNames";
pub(super) const GAME_TEXTS: &str = "/texts/gameTexts";

pub(super) struct SelectedKeys<'snapshot> {
    routes: BTreeSet<RouteKey<'snapshot>>,
    public_names: BTreeSet<&'snapshot str>,
    game_texts: BTreeSet<&'snapshot str>,
}

impl<'snapshot> SelectedKeys<'snapshot> {
    pub(super) fn for_snapshot(snapshot: &'snapshot MiningLocationSnapshot) -> TestResult<Self> {
        let mut routes = BTreeSet::new();
        for occurrence in &snapshot.harvestable_occurrences {
            if let HarvestableOccurrenceRoute::Reached(reached) = &occurrence.route {
                let key = reached
                    .id
                    .value()
                    .ok_or("SDK reached occurrence has no route")?;
                if !routes.insert(RouteKey::sdk(key)) {
                    return Err("duplicate SDK harvest occurrence route".into());
                }
            }
        }
        let public_names = snapshot
            .public_names
            .iter()
            .map(|row| {
                row.id
                    .value()
                    .map(PublicNameRef::as_str)
                    .ok_or("SDK public name has no ID")
            })
            .collect::<Result<BTreeSet<_>, _>>()?;
        let game_texts = snapshot
            .game_texts
            .iter()
            .map(|row| {
                row.id
                    .value()
                    .map(GameTextRef::as_str)
                    .ok_or("SDK game text has no ID")
            })
            .collect::<Result<BTreeSet<_>, _>>()?;
        if public_names.len() != snapshot.public_names.len()
            || game_texts.len() != snapshot.game_texts.len()
        {
            return Err("duplicate SDK text ID".into());
        }
        Ok(Self {
            routes,
            public_names,
            game_texts,
        })
    }

    pub(super) fn project<'selection>(
        &'selection self,
        paths: &mut CatalogCapture<Rows<'selection>>,
    ) {
        paths.project(CONTAINERS, Rows::Routes(&self.routes));
        paths.project(PUBLIC_NAMES, Rows::Texts(&self.public_names));
        paths.project(GAME_TEXTS, Rows::Texts(&self.game_texts));
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct RouteKey<'route> {
    root: u32,
    ordinals: &'route [u32],
    external_steps: &'route [usize],
}

impl<'route> RouteKey<'route> {
    fn sdk(key: &'route ContainerInstanceKey) -> Self {
        Self {
            root: key.root_entry_index(),
            ordinals: key.child_node_indices(),
            external_steps: key.external_child_step_indices(),
        }
    }
}

#[derive(Clone, Copy)]
pub(super) enum Rows<'keys> {
    Routes(&'keys BTreeSet<RouteKey<'keys>>),
    Texts(&'keys BTreeSet<&'keys str>),
}

impl Rows<'_> {
    fn admits(
        self,
        row: &Value,
        ordinals: &mut Vec<u32>,
        external_steps: &mut Vec<usize>,
    ) -> TestResult<bool> {
        match self {
            Self::Routes(keys) => {
                let root = row
                    .pointer("/instanceId/value/rootEntryIndex")
                    .and_then(Value::as_u64)
                    .ok_or("catalog container has no physical root index")?;
                let children = row
                    .pointer("/instanceId/value/childNodeIndices")
                    .and_then(Value::as_array)
                    .ok_or("catalog container has no route positions")?;
                ordinals.clear();
                for child in children {
                    let index = child.as_u64().ok_or("invalid route position")?;
                    ordinals.push(u32::try_from(index)?);
                }
                let steps = row
                    .pointer("/instanceId/value/externalChildStepIndices")
                    .and_then(Value::as_array)
                    .ok_or("catalog route needs external document steps from a current producer")?;
                external_steps.clear();
                for step in steps {
                    let index = step.as_u64().ok_or("invalid external document step")?;
                    external_steps.push(usize::try_from(index)?);
                }
                if !external_steps.is_sorted_by(|left, right| left < right)
                    || external_steps
                        .last()
                        .is_some_and(|last| *last >= ordinals.len())
                {
                    return Err("catalog route has invalid external document step positions".into());
                }
                Ok(keys.contains(&RouteKey {
                    root: u32::try_from(root)?,
                    ordinals,
                    external_steps,
                }))
            }
            Self::Texts(keys) => {
                let id = row
                    .pointer("/id/value")
                    .and_then(Value::as_str)
                    .ok_or("catalog text has no ID")?;
                Ok(keys.contains(id))
            }
        }
    }
}

impl<'de> DeserializeSeed<'de> for Rows<'_> {
    type Value = Value;

    fn deserialize<D: serde::Deserializer<'de>>(self, deserializer: D) -> Result<Value, D::Error> {
        deserializer.deserialize_seq(self)
    }
}

impl<'de> Visitor<'de> for Rows<'_> {
    type Value = Value;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a catalog row array")
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<Value, A::Error> {
        let mut rows = Vec::new();
        let mut ordinals = Vec::new();
        let mut external_steps = Vec::new();
        while let Some(row) = sequence.next_element::<Value>()? {
            if self
                .admits(&row, &mut ordinals, &mut external_steps)
                .map_err(serde::de::Error::custom)?
            {
                rows.push(row);
            }
        }
        Ok(Value::Array(rows))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{RouteKey, Rows};
    use crate::catalog_capture::CatalogCapture;
    use crate::real_archive_verification::TestResult;
    use serde_json::json;

    #[test]
    fn selected_occurrences_keep_physical_routes_instead_of_path_aliases() -> TestResult {
        let absolute = [1, 4];
        let expected = BTreeSet::from([RouteKey {
            root: 7,
            ordinals: &absolute,
            external_steps: &[1],
        }]);
        let mut projection = CatalogCapture::default();
        projection.project("/selected", Rows::Routes(&expected));
        let catalog = br#"{
            "unrelated":[{"large":"skip"}],
            "selected":[
                {"instanceId":{"value":{"rootEntryIndex":8,"childNodeIndices":[1,4],"externalChildStepIndices":[1]}},"sourcePath":"same"},
                {"instanceId":{"value":{"rootEntryIndex":7,"childNodeIndices":[1,4],"externalChildStepIndices":[]}},"sourcePath":"inline"},
                {"instanceId":{"value":{"rootEntryIndex":7,"childNodeIndices":[1,4],"externalChildStepIndices":[1]}},"sourcePath":"same"},
                {"instanceId":{"value":{"rootEntryIndex":7,"childNodeIndices":[1,5],"externalChildStepIndices":[1]}},"sourcePath":"same"}
            ]
        }"#;
        let selected = projection.read(catalog.as_slice())?;
        assert_eq!(
            selected,
            json!({"selected":[{"instanceId":{"value":{"rootEntryIndex":7,"childNodeIndices":[1,4],"externalChildStepIndices":[1]}},"sourcePath":"same"}]}),
            "only the admitted physical Child route is retained"
        );
        let stale = br#"{"selected":[{"instanceId":{"value":{"rootEntryIndex":7,"childNodeIndices":[1,4]}}}]}"#;
        let error = projection
            .read(stale.as_slice())
            .expect_err("an incomplete historical wire key does not prove the intended occurrence");
        assert!(
            error.to_string().contains("external document steps"),
            "stale catalog names the missing key coordinate"
        );
        Ok(())
    }
}
