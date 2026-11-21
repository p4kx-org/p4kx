// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, DataForge, Value as DataForgeValue};
use p4kx_testkit::{TestInstance, TestValue};
use serde_json::{Value, json};

use super::{DATAFORGE, LOADOUT_PATH, TestResult, dataforge, entry_index};

pub(super) fn duplicate_component() -> TestValue {
    let loadout = TestInstance::new("SItemPortLoadoutXMLParams")
        .value("loadoutPath", TestValue::text(LOADOUT_PATH));
    TestValue::Struct(
        TestInstance::new("SEntityComponentDefaultLoadoutParams")
            .value("loadout", TestValue::Struct(loadout)),
    )
}

pub(super) struct RootProof<'source> {
    source: &'source Value,
    indices: Value,
}

impl<'source> RootProof<'source> {
    pub(super) fn new(
        archive: &Archive,
        sources: &'source [Value],
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let entry = entry_index(archive, DATAFORGE)?;
        let source = sources
            .iter()
            .find(|source| source["entryIndex"] == json!(entry))
            .and_then(|source| source.get("id"))
            .ok_or("the selected original DataForge source is missing")?;
        let fixture = dataforge();
        let forge = DataForge::open(&fixture)?;
        let tree = forge
            .record(0)
            .ok_or("the fixture root Record is missing")?
            .tree()?;
        let mut ordinals = Vec::new();
        tree.root().visit_owned_values(|visit| {
            if visit.field_name() == "loadoutPath"
                && let DataForgeValue::String(text) = visit.value()
                && *text == LOADOUT_PATH
            {
                ordinals.push(visit.ordinal());
            }
        });
        let [active, _other] = ordinals.as_slice() else {
            return Err("active and losing components must declare identical paths".into());
        };
        Ok(Self {
            source,
            indices: json!(["0", active.to_string()]),
        })
    }

    pub(super) fn verify(&self, origin: &Value, uses: &[Value]) -> TestResult {
        let refs = origin["sourceUsesRefs"]
            .as_array()
            .ok_or("the class origin is missing its selector uses")?;
        let observed: Vec<_> = uses
            .iter()
            .filter(|item| {
                refs.contains(&item["id"])
                    && &item["source"] == self.source
                    && item.pointer("/location/namespace") == Some(&json!("dataforgeValue"))
            })
            .map(|item| {
                [
                    item.get("scope"),
                    item.get("role"),
                    item.pointer("/location/field"),
                    item.pointer("/location/indices"),
                ]
            })
            .collect();
        assert_eq!(
            observed,
            [[
                Some(&json!("payload")),
                Some(&json!("selector")),
                Some(&json!("$value")),
                Some(&self.indices)
            ]],
            "only the actual winning DataForge Value supplies a physical selector"
        );
        assert_eq!(
            origin.get("sourceSetComplete"),
            Some(&json!(false)),
            "the original DataForge selector alone does not prove the whole class route"
        );
        Ok(())
    }
}
