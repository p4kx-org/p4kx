// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::Cell;

use p4kx_export::Kind;
use p4kx_extraction::{DataForge, RecordTree, StructValue, Value as Original};
use serde_json::Value;

use super::{TestResult, verify_class, verify_scalar};
use crate::authored_definition_witness::{field, verify_array, verify_link, verify_state};
use crate::real_archive_auto_spawns::{original::Locations, provenance::Origins};
use crate::real_archive_canonical_texts::{TextDomain, TextIndex, TextReferenceShape};

pub(super) struct Witness<'scope, 'catalog, 'source> {
    forge: &'source DataForge<'source>,
    origins: &'scope Origins<'catalog>,
    texts: &'scope TextIndex<'catalog>,
    locations: Locations<'source>,
    owner: u32,
    checks: Cell<usize>,
}

impl<'scope, 'catalog, 'source> Witness<'scope, 'catalog, 'source> {
    pub(super) fn new(
        forge: &'source DataForge<'source>,
        tree: &RecordTree<'source>,
        origins: &'scope Origins<'catalog>,
        texts: &'scope TextIndex<'catalog>,
    ) -> Self {
        Self {
            forge,
            origins,
            texts,
            locations: Locations::read(tree.root()),
            owner: tree.record_index(),
            checks: Cell::new(0),
        }
    }

    pub(super) const fn checks(&self) -> usize {
        self.checks.get()
    }

    pub(super) fn verify(
        &self,
        kind: Kind,
        source: &StructValue<'source>,
        row: &Value,
    ) -> TestResult {
        match kind {
            Kind::Infractions => {
                self.locale(
                    source.field("name"),
                    field(row, "name")?,
                    TextDomain::PublicName,
                )?;
                self.locale(
                    source.field("description"),
                    field(row, "description")?,
                    TextDomain::GameText,
                )?;
                self.array(source, row, "triggers", |value, cell| {
                    self.scalar(value, cell)
                })?;
                self.owned(
                    source.field("defaultParameters"),
                    field(row, "defaultParameters")?,
                    |source, row| self.parameters(source, row),
                )
            }
            Kind::InfractionSets => self.uses(source, row),
            Kind::Jurisdictions => self.jurisdiction(source, row),
            _ => Err("kind has no law definition witness".into()),
        }
    }

    fn jurisdiction(&self, source: &StructValue<'source>, row: &Value) -> TestResult {
        self.scalars(
            source,
            row,
            &[
                "subsumptionJurisdiction",
                "logoPath",
                "baseFine",
                "earlyPaymentPeriod",
                "isPrison",
                "respectsParentJurisdictionLaws",
            ],
        )?;
        self.scalar(
            source.field("maxStolenGoodsPossessionSCU"),
            field(row, "maxStolenGoodsPossessionScu")?,
        )?;
        self.locale(
            source.field("name"),
            field(row, "name")?,
            TextDomain::PublicName,
        )?;
        for (name, target) in [
            ("parentJurisdiction", "Jurisdiction"),
            ("journalEntry", "JournalEntry"),
        ] {
            self.link(source.field(name), field(row, name)?, target)?;
        }
        for (name, target) in [
            ("infractionSets", "InfractionSet"),
            ("prohibitedGoods", "EntityClassDefinition"),
            ("prohibitedResources", "ResourceType"),
        ] {
            self.links(source, row, name, target)?;
        }
        self.uses(source, row)?;
        self.array(source, row, "controlledSubstanceClasses", |value, cell| {
            self.owned(value, cell, |source, row| {
                self.links(source, row, "commodities", "EntityClassDefinition")?;
                self.links(source, row, "resources", "ResourceType")?;
                self.scalar(
                    source.field("maxPossessionSCU"),
                    field(row, "maxPossessionScu")?,
                )
            })
        })?;
        self.array(source, row, "impoundingDefinitions", |value, cell| {
            self.owned(value, cell, |source, row| self.impounding(source, row))
        })
    }

    fn impounding(&self, source: &StructValue<'source>, row: &Value) -> TestResult {
        self.locale(
            source.field("name"),
            field(row, "name")?,
            TextDomain::PublicName,
        )?;
        self.scalars(
            source,
            row,
            &[
                "trigger",
                "impoundingTimeSeconds",
                "ignoreIfAgainstPartyMember",
            ],
        )?;
        self.scalar(
            source.field("impoundingFineUEC"),
            field(row, "impoundingFineUec")?,
        )
    }

    fn uses(&self, source: &StructValue<'source>, row: &Value) -> TestResult {
        self.array(source, row, "infractions", |value, cell| {
            self.owned(value, cell, |source, row| {
                self.link(
                    source.field("definition"),
                    field(row, "definition")?,
                    "InfractionDefinition",
                )?;
                self.owned(
                    source.field("parameterOverrides"),
                    field(row, "parameterOverrides")?,
                    |source, row| self.parameters(source, row),
                )
            })
        })
    }

    fn parameters(&self, source: &StructValue<'source>, row: &Value) -> TestResult {
        self.scalars(
            source,
            row,
            &[
                "isFelony",
                "graceAllowance",
                "graceAllowanceCooldown",
                "gracePeriod",
                "graceCooloffScale",
                "displayGraceTime",
                "escalatedPaymentFineMultiplier",
                "earlyPaymentPeriod",
                "lifetime",
                "coolOffTime",
                "pressChargesNotificationTime",
                "removeTimeSeconds",
                "ignoreIfAgainstPartyMember",
                "hideCrimeNotification",
                "hideCrimeInJournal",
                "felonyMerits",
            ],
        )?;
        self.array(source, row, "graceWarnings", |value, cell| {
            self.locale(value, cell, TextDomain::GameText)
        })
    }

    fn scalars(&self, source: &StructValue<'source>, row: &Value, names: &[&str]) -> TestResult {
        for name in names {
            self.scalar(source.field(name), field(row, name)?)?;
        }
        Ok(())
    }

    fn scalar(&self, source: Option<&Original<'source>>, cell: &Value) -> TestResult {
        self.occurrence(source, cell, "$value")?;
        verify_scalar(source, cell)
    }

    fn owned(
        &self,
        source: Option<&Original<'source>>,
        cell: &Value,
        verify: impl FnOnce(&StructValue<'source>, &Value) -> TestResult,
    ) -> TestResult {
        let part = if matches!(source, Some(Original::Struct(_))) {
            "$type"
        } else {
            "$value"
        };
        self.occurrence(source, cell, part)?;
        verify_class(source, cell, verify)
    }

    fn array(
        &self,
        source: &StructValue<'source>,
        row: &Value,
        name: &str,
        verify: impl Fn(Option<&Original<'source>>, &Value) -> TestResult,
    ) -> TestResult {
        let value = source.field(name);
        let cell = field(row, name)?;
        self.occurrence(value, cell, "$value")?;
        verify_array(value, cell, verify)
    }

    fn links(
        &self,
        source: &StructValue<'source>,
        row: &Value,
        name: &str,
        target: &str,
    ) -> TestResult {
        self.array(source, row, name, |value, cell| {
            self.link(value, cell, target)
        })
    }

    fn link(&self, source: Option<&Original<'source>>, cell: &Value, target: &str) -> TestResult {
        self.occurrence(source, cell, "$value")?;
        verify_link(self.forge, target, source, cell)
    }

    fn locale(
        &self,
        source: Option<&Original<'source>>,
        cell: &Value,
        domain: TextDomain,
    ) -> TestResult {
        self.occurrence(source, cell, "$value")?;
        let Some(Original::Locale(key)) = source else {
            return verify_state(source, cell);
        };
        let target = self
            .texts
            .reference(cell, domain, TextReferenceShape::Required)?
            .ok_or("authored law Locale has a canonical text reference")?;
        assert!(
            target
                .source()
                .pointer("/key/value")
                .and_then(Value::as_str)
                == Some(*key),
            "original law Locale key"
        );
        Ok(())
    }

    fn occurrence(
        &self,
        source: Option<&Original<'source>>,
        cell: &Value,
        part: &str,
    ) -> TestResult {
        self.checks.set(self.checks.get() + 1);
        if let Some(value) = source.filter(|value| !matches!(value, Original::Error(_))) {
            self.origins
                .occurrence(cell, self.owner, self.locations.ordinal(value)?, part)?;
        }
        Ok(())
    }
}
