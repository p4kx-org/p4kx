// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForge, StructValue, Value as Original, ValueError};
use serde_json::{Value, json};

use super::TestResult;
use crate::authored_definition_witness::{field, verify_array, verify_link, verify_state};
use crate::catalog_v3_consumer as consumer;
use crate::real_archive_auto_spawns::original::Locations;
use crate::real_archive_auto_spawns::provenance::Origins;
use crate::real_archive_item_durability::Trace;
use crate::real_archive_npc_spawn_definitions::spawn::verify_single;

pub(super) struct Check<'view, 'source> {
    forge: &'view DataForge<'source>,
    origins: &'view Origins<'view>,
    locations: &'view Locations<'source>,
    owner: u32,
    pub(super) trace: &'view Trace<'view, 'source>,
}

impl<'view, 'source> Check<'view, 'source> {
    pub(super) const fn new(
        forge: &'view DataForge<'source>,
        origins: &'view Origins<'view>,
        locations: &'view Locations<'source>,
        owner: u32,
        trace: &'view Trace<'view, 'source>,
    ) -> Self {
        Self {
            forge,
            origins,
            locations,
            owner,
            trace,
        }
    }

    pub(super) fn configuration(
        &self,
        source: &Original<'source>,
        data: &StructValue<'source>,
        row: &Value,
    ) -> TestResult {
        self.origin(row, source, "$type")?;
        let row = consumer::present(row)?;
        self.array(data, row, "autoSpawnSettings", Self::spawn)?;
        self.array(data, row, "missionLimits", Self::limit)?;
        self.array(data, row, "missionModules", Self::mission_module)?;
        self.link(
            data.field("defaultSecurityNetworkManifest"),
            field(row, "defaultSecurityNetworkManifest")?,
            "SecurityNetworkManifest",
        )
    }

    fn spawn(&self, source: &StructValue<'source>, row: &Value) -> TestResult {
        for name in [
            "name",
            "initialActivity",
            "excludeShipCrew",
            "excludeSpawnGender",
            "minGroupSize",
            "maxGroupSize",
            "maxConcurrentSpawns",
            "maxSpawns",
            "missionAlliedMarker",
            "isCritical",
        ] {
            self.scalar(source, row, name)?;
        }
        for name in ["minSpawnDelay", "maxSpawnDelay"] {
            self.single(source, row, name)?;
        }
        self.array(source, row, "inventoryItems", Self::inventory)?;
        self.link(
            source.field("factionOverride"),
            field(row, "factionOverride")?,
            "Faction",
        )?;
        for name in [
            "positiveCharacterTags",
            "negativeCharacterTags",
            "closetPositiveTags",
            "closetNegativeTags",
            "roomPositiveTags",
            "roomNegativeTags",
            "defendAreaPositiveTags",
            "defendAreaNegativeTags",
            "scheduleAreaPositiveTags",
            "scheduleAreaNegativeTags",
            "entityTags",
        ] {
            self.tags(source.field(name), field(row, name)?)?;
        }
        Ok(())
    }

    fn limit(&self, source: &StructValue<'source>, row: &Value) -> TestResult {
        self.link(source.field("tag"), field(row, "tag")?, "Tag")?;
        self.scalar(source, row, "autoConsumeIfPlayerAtLocation")?;
        self.scalar(source, row, "maxMissionInstances")
    }

    fn inventory(&self, source: &StructValue<'source>, row: &Value) -> TestResult {
        self.link(
            source.field("entityClass"),
            field(row, "entityClass")?,
            "EntityClassDefinition",
        )?;
        for name in ["quantity", "amountInventoriesToFill", "markAsMissionItem"] {
            self.scalar(source, row, name)?;
        }
        Ok(())
    }

    fn tags(&self, source: Option<&Original<'source>>, row: &Value) -> TestResult {
        let Some(Original::Struct(tags)) = source else {
            return self.state(source, row);
        };
        let original = source.ok_or("original TagList")?;
        self.origin(row, original, "$type")?;
        let tags_source = tags.field("tags");
        let tags_row = field(consumer::present(row)?, "tags")?;
        if let Some(original) = tags_source {
            self.origin(tags_row, original, "$value")?;
        }
        verify_array(tags_source, tags_row, |item, cell| {
            self.link(item, cell, "Tag")
        })
    }

    pub(super) fn array(
        &self,
        owner: &StructValue<'source>,
        row: &Value,
        name: &str,
        compare: fn(&Self, &StructValue<'source>, &Value) -> TestResult,
    ) -> TestResult {
        let original = owner.field(name);
        let row = field(row, name)?;
        if let Some(original) = original {
            self.origin(row, original, "$value")?;
        }
        verify_array(original, row, |item, cell| {
            if let Some(item @ Original::Struct(value)) = item {
                self.origin(cell, item, "$type")?;
                compare(self, value, consumer::present(cell)?)
            } else {
                self.state(item, cell)
            }
        })
    }

    pub(super) fn single(
        &self,
        source: &StructValue<'source>,
        row: &Value,
        name: &str,
    ) -> TestResult {
        let original = source.field(name);
        let cell = field(row, name)?;
        verify_single(original, cell)?;
        if let Some(original) = original {
            self.origin(cell, original, "$value")?;
        }
        Ok(())
    }

    pub(super) fn scalar(
        &self,
        source: &StructValue<'source>,
        row: &Value,
        name: &str,
    ) -> TestResult {
        let source = source.field(name);
        let row = field(row, name)?;
        let expected = match source {
            Some(Original::Bool(value)) => json!(value),
            Some(Original::Int32(value)) => json!(value),
            Some(Original::String(value) | Original::Enum(value)) => json!(value),
            _ => return self.state(source, row),
        };
        assert!(
            consumer::present(row)? == &expected,
            "exact original location datum {name}"
        );
        self.origin(row, source.ok_or("original scalar")?, "$value")
    }

    pub(super) fn link(
        &self,
        source: Option<&Original<'source>>,
        row: &Value,
        target: &str,
    ) -> TestResult {
        verify_link(self.forge, target, source, row)?;
        match source {
            Some(original @ Original::Reference(link)) => {
                self.origin(row, original, "$value")?;
                let value = consumer::present(row)?;
                let ordinal = self.locations.ordinal(original)?;
                for (name, expected, header) in [
                    ("guid", json!(link.guid().to_string()), None),
                    (
                        "targetRecordIndex",
                        json!(link.record_index()),
                        Some("recordIndex"),
                    ),
                    ("recordPath", json!(link.record_path()), Some("fileName")),
                    ("targetType", json!(link.type_name()), Some("sourceType")),
                ] {
                    let cell = field(value, name)?;
                    assert!(
                        consumer::present(cell)? == &expected,
                        "the authored link retains its exact physical {name}"
                    );
                    self.origins.reference_field(
                        cell,
                        self.owner,
                        ordinal,
                        link.record_index(),
                        header,
                    )?;
                }
            }
            Some(original @ Original::Error(ValueError::DanglingReference { .. })) => {
                let guid = field(consumer::present(row)?, "guid")?;
                self.origin(guid, original, "$error.guid")?;
            }
            Some(original) => self.origin(row, original, "$value")?,
            None => {}
        }
        Ok(())
    }

    pub(super) fn state(&self, source: Option<&Original<'source>>, row: &Value) -> TestResult {
        verify_state(source, row)?;
        if let Some(original) = source {
            self.origin(row, original, "$value")?;
        }
        Ok(())
    }

    pub(super) fn origin(
        &self,
        row: &Value,
        original: &Original<'source>,
        part: &str,
    ) -> TestResult {
        self.origins
            .occurrence(row, self.owner, self.locations.ordinal(original)?, part)
    }
}
