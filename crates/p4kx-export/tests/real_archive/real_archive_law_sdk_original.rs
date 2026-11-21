// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::Kind;
use p4kx_export::catalog_queries::law::{LawRows, LawSnapshot};
use p4kx_export::catalog_queries::{CanonicalRecordHeader, DatasetAvailability};
use p4kx_extraction::{
    ArchiveWatch, DataForge, DataForgeOrigin, Record, TreeBuilder, Value as Original,
};
use serde::Serialize;
use serde_json::{Value, json};

use crate::authored_definition_witness::field;
use crate::real_archive_auto_spawns::{original::Locations, provenance::Origins};
use crate::real_archive_canonical_texts::TextIndex;
use crate::real_archive_catalog_table_parity::{TestResult, wire_row};
use crate::real_archive_law_definitions::verify_law_projection;
use crate::real_archive_security_manifests::verify_manifest;

#[derive(Clone, Copy)]
pub(super) struct Family {
    kind: Kind,
    source: &'static str,
    pub(super) path: &'static str,
}

const INFRACTIONS: Family = Family {
    kind: Kind::Infractions,
    source: "InfractionDefinition",
    path: "/definitions/infractions",
};
const SETS: Family = Family {
    kind: Kind::InfractionSets,
    source: "InfractionSet",
    path: "/definitions/infractionSets",
};
const JURISDICTIONS: Family = Family {
    kind: Kind::Jurisdictions,
    source: "Jurisdiction",
    path: "/definitions/jurisdictions",
};
const SECURITY: Family = Family {
    kind: Kind::SecurityNetworkManifests,
    source: "SecurityNetworkManifest",
    path: "/definitions/securityNetworkManifests",
};
pub(super) const FAMILIES: [Family; 4] = [INFRACTIONS, SETS, JURISDICTIONS, SECURITY];

#[derive(Default)]
struct Records<'source> {
    declared: bool,
    owners: Vec<Record<'source>>,
}

pub(super) struct OriginalLaws<'source> {
    infractions: Records<'source>,
    infraction_sets: Records<'source>,
    jurisdictions: Records<'source>,
    security: Records<'source>,
}

impl<'source> OriginalLaws<'source> {
    pub(super) fn read(forge: &'source DataForge<'_>, watch: &ArchiveWatch) -> TestResult<Self> {
        if forge.record(u32::MAX).is_some() {
            return Err("expected one physical index outside this original DataForge".into());
        }
        let mut result = Self {
            infractions: Records::default(),
            infraction_sets: Records::default(),
            jurisdictions: Records::default(),
            security: Records::default(),
        };
        for structure in forge.structs() {
            watch.poll()?;
            let name = structure
                .name()
                .map_err(|_| "unreadable original source declaration")?;
            if let Some(family) = result.lookup(name) {
                family.declared = true;
            }
        }
        for owner in forge.records() {
            watch.poll()?;
            let name = owner
                .type_name()
                .map_err(|_| "unreadable original source family")?;
            if let Some(family) = result.lookup(name) {
                family.owners.push(owner);
            }
        }
        for family in [
            &mut result.infractions,
            &mut result.infraction_sets,
            &mut result.jurisdictions,
            &mut result.security,
        ] {
            family
                .owners
                .sort_by_cached_key(|owner| (owner.guid().to_string(), owner.index()));
        }
        if [
            &result.infractions,
            &result.infraction_sets,
            &result.jurisdictions,
            &result.security,
        ]
        .iter()
        .all(|family| family.owners.is_empty())
        {
            return Err(
                "no original law or security Record in the selected installed Build".into(),
            );
        }
        Ok(result)
    }

    fn lookup(&mut self, source_type: &str) -> Option<&mut Records<'source>> {
        match source_type {
            name if name == INFRACTIONS.source => Some(&mut self.infractions),
            name if name == SETS.source => Some(&mut self.infraction_sets),
            name if name == JURISDICTIONS.source => Some(&mut self.jurisdictions),
            name if name == SECURITY.source => Some(&mut self.security),
            _ => None,
        }
    }

    pub(super) fn verify_absence(&self, sdk: &LawSnapshot<'_>) -> TestResult {
        self.infractions
            .verify_selection(INFRACTIONS, table(sdk.infractions.as_ref())?, 0)?;
        self.infraction_sets
            .verify_selection(SETS, table(sdk.infraction_sets.as_ref())?, 0)?;
        self.jurisdictions.verify_selection(
            JURISDICTIONS,
            table(sdk.jurisdictions.as_ref())?,
            0,
        )?;
        self.security
            .verify_selection(SECURITY, table(sdk.security_network_manifests.as_ref())?, 0)
    }

    pub(super) fn verify(
        &self,
        sdk: &LawSnapshot<'_>,
        forge: &'source DataForge<'source>,
        origin: &DataForgeOrigin,
        watch: &ArchiveWatch,
    ) -> TestResult<[usize; 4]> {
        let scope = json!({
            "provenance": sdk.source_inventory,
            "texts": {"publicNames": sdk.public_names, "gameTexts": sdk.game_texts},
        });
        let witness = Witness {
            forge,
            origins: Origins::read(&scope, origin)?,
            texts: TextIndex::new(&scope)?,
            watch,
        };
        Ok([
            witness.compare(
                INFRACTIONS,
                &self.infractions,
                table(sdk.infractions.as_ref())?,
                |row| &row.source,
            )?,
            witness.compare(
                SETS,
                &self.infraction_sets,
                table(sdk.infraction_sets.as_ref())?,
                |row| &row.source,
            )?,
            witness.compare(
                JURISDICTIONS,
                &self.jurisdictions,
                table(sdk.jurisdictions.as_ref())?,
                |row| &row.source,
            )?,
            witness.compare(
                SECURITY,
                &self.security,
                table(sdk.security_network_manifests.as_ref())?,
                |row| &row.source,
            )?,
        ])
    }
}

impl Records<'_> {
    fn verify_selection<Row>(
        &self,
        family: Family,
        rows: &LawRows<Row>,
        expected: usize,
    ) -> TestResult {
        let available = self.declared || !self.owners.is_empty();
        let availability_matches = matches!(
            (available, rows.availability),
            (true, DatasetAvailability::Available) | (false, DatasetAvailability::Absent),
        );
        if !availability_matches || rows.rows.len() != expected {
            return Err(format!(
                "{} SDK availability or row census differs from the original source",
                family.source
            )
            .into());
        }
        Ok(())
    }
}

struct Witness<'catalog, 'source> {
    forge: &'source DataForge<'source>,
    origins: Origins<'catalog>,
    texts: TextIndex<'catalog>,
    watch: &'catalog ArchiveWatch,
}

impl<'original> Witness<'_, 'original> {
    fn compare<'source, Row: Serialize>(
        &self,
        family: Family,
        original: &Records<'original>,
        sdk: &LawRows<Row>,
        header: impl for<'row> Fn(&'row Row) -> &'row CanonicalRecordHeader<'source>,
    ) -> TestResult<usize> {
        original.verify_selection(family, sdk, original.owners.len())?;
        for (owner, row) in original.owners.iter().zip(&sdk.rows) {
            self.watch.poll()?;
            let observed = header(row);
            if observed.record_index().value() != Some(&owner.index())
                || observed.guid().value() != Some(&owner.guid())
                || observed.record_path().value()
                    != Some(
                        &owner
                            .record_path()
                            .map_err(|_| "unreadable original law path")?,
                    )
                || observed.source_type().value() != Some(&family.source)
            {
                return Err(format!(
                    "{} SDK physical identity or ordering differs",
                    family.source
                )
                .into());
            }
        }
        if let (Some(owner), Some(row)) = (original.owners.first(), sdk.rows.first()) {
            self.verify_representative(family, owner, &wire_row(row)?)?;
        }
        Ok(sdk.rows.len())
    }

    fn verify_representative(
        &self,
        family: Family,
        owner: &Record<'original>,
        row: &Value,
    ) -> TestResult {
        self.watch.poll()?;
        let tree = TreeBuilder::new()
            .build(owner)
            .map_err(|_| "read a representative original law Record")?;
        if family.kind == SECURITY.kind {
            verify_manifest(self.forge, tree.root(), row)?;
            let locations = Locations::read(tree.root());
            for name in ["isNeutralTerritory", "teleportPlayerOut"] {
                if let Some(value @ Original::Bool(_)) = tree.field(name) {
                    self.origins.occurrence(
                        field(row, name)?,
                        tree.record_index(),
                        locations.ordinal(value)?,
                        "$value",
                    )?;
                }
            }
        } else {
            verify_law_projection(
                self.forge,
                &tree,
                family.kind,
                row,
                &self.origins,
                &self.texts,
            )?;
        }
        self.watch.poll()?;
        Ok(())
    }
}

fn table<Row>(value: Option<&LawRows<Row>>) -> TestResult<&LawRows<Row>> {
    value.ok_or_else(|| "requested law SDK family was omitted".into())
}
