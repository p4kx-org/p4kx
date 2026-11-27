// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::DatasetAvailability;
use p4kx_export::catalog_queries::mission_properties::MissionFlowOwnerRef;
use p4kx_export::{Kind, is_kind_leftover};
use p4kx_extraction::{DataForge, DataForgeEntry};

use crate::real_archive_catalog_table_parity::{MissionDisposition, TestResult};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Family {
    Template,
    Broker,
}

impl Family {
    fn of(owner: &MissionFlowOwnerRef) -> TestResult<Self> {
        match owner {
            MissionFlowOwnerRef::ContractTemplate(_) => Ok(Self::Template),
            MissionFlowOwnerRef::MissionBrokerEntry(_) => Ok(Self::Broker),
            _ => Err("unsupported mission owner family".into()),
        }
    }
}

#[derive(Eq, Ord, PartialEq, PartialOrd)]
struct Header {
    family: Family,
    guid: String,
    record_index: u32,
}

pub(super) struct OriginalScope {
    declarations: Declarations,
    headers: Vec<Header>,
    pub(super) dispositions: [MissionDisposition; 2],
}

impl OriginalScope {
    pub(super) fn read(
        input: &DataForgeEntry,
        expected: [MissionDisposition; 2],
    ) -> TestResult<Self> {
        let forge = input
            .open()
            .map_err(|_| "parse the bound original DataForge")?;
        let template = Kind::ContractTemplates
            .rule()
            .ok_or("Template Record rule")?;
        let broker = Kind::MissionBrokerEntries
            .rule()
            .ok_or("Broker Record rule")?;
        let declarations = Declarations::survey(&forge, template.type_name, broker.type_name);
        let mut scope = Self {
            declarations,
            headers: Vec::new(),
            dispositions: [MissionDisposition::default(); 2],
        };
        for record in forge.records() {
            let path = record.record_path();
            if path
                .as_ref()
                .is_ok_and(|path| !template.matches_path(path) && !broker.matches_path(path))
            {
                continue;
            }
            let name = record
                .type_name()
                .map_err(|_| "unreadable mission selection candidate type")?;
            let (family, kind, rule) = if name == template.type_name {
                (Family::Template, Kind::ContractTemplates, template)
            } else if name == broker.type_name {
                (Family::Broker, Kind::MissionBrokerEntries, broker)
            } else {
                continue;
            };
            let path = path.map_err(|_| "unreadable mission selection candidate path")?;
            if !rule.matches_path(path) {
                continue;
            }
            let name = record
                .name()
                .map_err(|_| "unreadable original mission Record name")?;
            scope.observe(
                Header {
                    family,
                    guid: record.guid().to_string(),
                    record_index: record.index(),
                },
                is_kind_leftover(kind, path, name),
            );
        }
        if scope.dispositions != expected {
            return Err(format!("the original Template and Broker dispositions differ: expected {expected:?}, found {:?}", scope.dispositions).into());
        }
        scope.headers.sort_unstable();
        Ok(scope)
    }

    fn observe(&mut self, header: Header, skipped: bool) {
        let [template, broker] = &mut self.dispositions;
        let disposition = match header.family {
            Family::Template => template,
            Family::Broker => broker,
        };
        if skipped {
            disposition.skipped += 1;
        } else {
            disposition.owners += 1;
            self.headers.push(header);
        }
    }

    pub(super) const fn owners(&self) -> usize {
        self.headers.len()
    }

    pub(super) fn check_owner(&self, ordinal: usize, owner: &MissionFlowOwnerRef) -> TestResult {
        let physical = owner.physical();
        let current = Header {
            family: Family::of(owner)?,
            guid: physical.record_guid().get().to_string(),
            record_index: physical.record_index(),
        };
        if self.headers.get(ordinal) != Some(&current) {
            return Err(format!(
                "the callback owner differs from original physical GUID order at ordinal {ordinal}"
            )
            .into());
        }
        Ok(())
    }

    pub(super) const fn availability(&self) -> [DatasetAvailability; 4] {
        let combined = declared(
            self.declarations.template || self.declarations.broker,
            self.declarations.unreadable,
        );
        [
            combined,
            combined,
            declared(self.declarations.template, self.declarations.unreadable),
            declared(self.declarations.broker, self.declarations.unreadable),
        ]
    }
}

#[derive(Default)]
struct Declarations {
    template: bool,
    broker: bool,
    unreadable: bool,
}

impl Declarations {
    fn survey(forge: &DataForge<'_>, template: &str, broker: &str) -> Self {
        let mut result = Self::default();
        for structure in forge.structs() {
            match structure.name() {
                Ok(name) if name == template => result.template = true,
                Ok(name) if name == broker => result.broker = true,
                Ok(_) => {}
                Err(_) => result.unreadable = true,
            }
        }
        result
    }
}

const fn declared(observed: bool, unreadable: bool) -> DatasetAvailability {
    match (observed, unreadable) {
        (true, _) => DatasetAvailability::Available,
        (false, true) => DatasetAvailability::Unavailable,
        (false, false) => DatasetAvailability::Absent,
    }
}
