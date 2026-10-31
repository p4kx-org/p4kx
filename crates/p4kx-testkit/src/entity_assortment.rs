// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::{TestInstance, TestLoadout, TestRecord, TestRequirement, TestValue};

/// One group of a synthetic assortment: its name, its alternative Loadouts in order and,
/// optionally, its Requirement.
#[derive(Debug, Clone, PartialEq)]
pub struct TestGroup {
    name: String,
    loadouts: Vec<TestLoadout>,
    requirement: Option<TestRequirement>,
}

impl TestGroup {
    /// Creates a group without a Requirement.
    #[must_use]
    pub fn new(name: impl Into<String>, loadouts: Vec<TestLoadout>) -> Self {
        Self {
            name: name.into(),
            loadouts,
            requirement: None,
        }
    }

    /// Sets the Requirement of the group.
    #[must_use]
    pub fn requiring(mut self, requirement: TestRequirement) -> Self {
        self.requirement = Some(requirement);
        self
    }

    fn value(&self) -> TestValue {
        let mut instance = TestInstance::new("SGroupedLoadouts")
            .value("Group", TestValue::text(self.name.clone()))
            .value(
                "Loadouts",
                TestValue::Array(self.loadouts.iter().map(TestLoadout::value).collect()),
            );
        if let Some(requirement) = &self.requirement {
            instance = instance.value("Requirement", requirement.value());
        }
        TestValue::Struct(instance)
    }
}

/// Returns the Record of type `SLoadoutAssortment` named `name` with the groups `groups`, each a
/// group name and its alternative Loadouts in order, none with a Requirement.
///
/// Its record path is `libs/foundry/records/loadoutkits/<name>`; the database needs
/// [`entity_schema`](crate::entity_schema).
#[must_use]
pub fn assortment_record(
    guid: u128,
    name: &str,
    groups: &[(&str, Vec<TestLoadout>)],
) -> TestRecord {
    let groups: Vec<TestGroup> = groups
        .iter()
        .map(|(group, loadouts)| TestGroup::new(*group, loadouts.clone()))
        .collect();
    assortment_record_requiring(guid, name, None, &groups)
}

/// Like [`assortment_record`], with the optional Requirement of the assortment and the groups as
/// [`TestGroup`]s.
#[must_use]
pub fn assortment_record_requiring(
    guid: u128,
    name: &str,
    requirement: Option<&TestRequirement>,
    groups: &[TestGroup],
) -> TestRecord {
    let record = TestRecord::new(
        guid,
        "SLoadoutAssortment",
        format!("SLoadoutAssortment.{name}"),
        format!("libs/foundry/records/loadoutkits/{name}.xml"),
    )
    .value(
        "GroupedLoadouts",
        TestValue::Array(groups.iter().map(TestGroup::value).collect()),
    );
    match requirement {
        Some(requirement) => record.value("Requirement", requirement.value()),
        None => record,
    }
}
