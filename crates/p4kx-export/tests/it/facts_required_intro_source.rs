// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Field, Record, RecordTree, StructValue, Value, ValueError};

use crate::health_origin_fixture::{self, Fixture, TestResult};

use super::super::{FAIL_TAG, GATED, GENERATOR, INTRO, OVERRIDDEN, SUB, TAG};
use super::{FAILED_INTRO, LOWER_INTRO, MISSING_TAG, REPEATED};

#[derive(Clone, Copy)]
pub(super) struct Site {
    pub(super) record: u32,
    pub(super) ordinal: u64,
}

#[derive(Clone, Copy)]
pub(super) struct Requirement {
    pub(super) prerequisite: Site,
    pub(super) tag: Site,
}

pub(super) struct Grant {
    pub(super) id: Site,
    pub(super) first_result: Site,
    pub(super) tag: Site,
}

pub(super) struct Sources {
    pub(super) handler: Requirement,
    pub(super) contract: Requirement,
    pub(super) sub: Requirement,
    pub(super) ungranted: Requirement,
    pub(super) repeated: [Requirement; 2],
    pub(super) dangling: Site,
    pub(super) primary: Grant,
    pub(super) duplicate: Grant,
    pub(super) lower: Grant,
    pub(super) rejected: Grant,
    pub(super) later_result: Site,
}

impl Sources {
    pub(super) fn read(fixture: &Fixture) -> TestResult<Self> {
        let forge = fixture.entry.open()?;
        let first = forge.record(3).ok_or("first original Generator")?;
        let second = forge.record(4).ok_or("second original Generator")?;
        check_generators(&first, &second)?;
        let first_tree = first.tree()?;
        let second_tree = second.tree()?;
        assert!(
            first_tree.is_from(&first) && second_tree.is_from(&second),
            "actual tree owners"
        );
        assert!(
            !first_tree.is_from(&second),
            "GUID equality is not physical Record identity"
        );
        let left = Original::new(&first_tree);
        let right = Original::new(&second_tree);
        let first_handler = structure(element(first_tree.root(), "generators", 0)?)?;
        let second_handler = structure(element(second_tree.root(), "generators", 0)?)?;
        let gated = member(first_handler, "contracts", 2, GATED)?;
        let overridden = member(first_handler, "contracts", 1, OVERRIDDEN)?;
        let child = member(overridden, "subContracts", 0, SUB)?;
        let availability = structure(field(first_handler, "defaultAvailability")?)?;
        let repeated = member(second_handler, "contracts", 0, REPEATED)?;
        let repeated_tags = Tags::read(repeated, "additionalPrerequisites")?;
        let (primary, _) = left.grant(first_handler, 0, INTRO, TAG, &[true])?;
        let (duplicate, _) = right.grant(second_handler, 0, INTRO, TAG, &[true])?;
        let (lower, _) = right.grant(second_handler, 1, LOWER_INTRO, TAG, &[true])?;
        let (rejected, later_result) =
            right.grant(second_handler, 2, FAILED_INTRO, FAIL_TAG, &[false, true])?;
        Ok(Self {
            handler: Tags::read(availability, "prerequisites")?.requirement(&left, 0, FAIL_TAG)?,
            contract: Tags::read(gated, "additionalPrerequisites")?.requirement(&left, 0, TAG)?,
            sub: Tags::read(child, "additionalPrerequisites")?.requirement(&left, 0, TAG)?,
            ungranted: Tags::read(gated, "additionalPrerequisites")?
                .requirement(&left, 1, FAIL_TAG)?,
            repeated: [
                repeated_tags.requirement(&right, 0, TAG)?,
                repeated_tags.requirement(&right, 2, TAG)?,
            ],
            dangling: repeated_tags.dangling(&right)?,
            primary,
            duplicate,
            lower,
            rejected,
            later_result: later_result.ok_or("actual second outcome")?,
        })
    }
}

fn check_generators(first: &Record<'_>, second: &Record<'_>) -> TestResult {
    for record in [first, second] {
        assert_eq!(
            record.type_name()?,
            "ContractGenerator",
            "original record family"
        );
        assert_eq!(
            record.guid().to_string(),
            health_origin_fixture::guid(GENERATOR),
            "the two original physical records have the same GUID"
        );
    }
    assert_ne!(
        first.record_path()?,
        second.record_path()?,
        "distinct authored files"
    );
    Ok(())
}

struct Original<'read, 'source> {
    record: u32,
    values: Vec<&'read Value<'source>>,
}

impl<'read, 'source> Original<'read, 'source> {
    fn new(tree: &'read RecordTree<'source>) -> Self {
        let mut pending: Vec<_> = tree.fields().iter().rev().map(Field::value).collect();
        let mut values = Vec::new();
        while let Some(value) = pending.pop() {
            values.push(value);
            match value {
                Value::Struct(owner) => {
                    pending.extend(owner.fields().iter().rev().map(Field::value));
                }
                Value::Array(items) => pending.extend(items.iter().rev()),
                _ => {}
            }
        }
        Self {
            record: tree.record_index(),
            values,
        }
    }

    fn site(&self, value: &Value<'_>) -> TestResult<Site> {
        let ordinal = self
            .values
            .iter()
            .position(|current| std::ptr::eq(*current, value))
            .ok_or("original value occurrence")?;
        Ok(Site {
            record: self.record,
            ordinal: u64::try_from(ordinal)?,
        })
    }

    fn grant(
        &self,
        handler: &StructValue<'_>,
        index: usize,
        id: u128,
        tag: u128,
        outcomes: &[bool],
    ) -> TestResult<(Grant, Option<Site>)> {
        let intro = member(handler, "introContracts", index, id)?;
        let results = structure(field(intro, "contractResults")?)?;
        let result = structure(element(results, "contractResults", 0)?)?;
        assert_eq!(
            result.type_name(),
            "ContractResult_CompletionTags",
            "actual result kind"
        );
        let stored = array(result, "missionResults")?;
        assert_eq!(stored.len(), outcomes.len(), "all original outcome slots");
        for (value, expected) in stored.iter().zip(outcomes) {
            assert_eq!(
                field(structure(value)?, "value")?,
                &Value::Bool(*expected),
                "independent authored result flag"
            );
        }
        let first_result = field(structure(stored.first().ok_or("first outcome")?)?, "value")?;
        let later = stored
            .get(1)
            .map(structure)
            .transpose()?
            .map(|value| self.site(field(value, "value")?))
            .transpose()?;
        let tag_value = field(structure(element(result, "completionTags", 0)?)?, "tag")?;
        check_target(tag_value, tag)?;
        Ok((
            Grant {
                id: self.site(field(intro, "id")?)?,
                first_result: self.site(first_result)?,
                tag: self.site(tag_value)?,
            },
            later,
        ))
    }
}

struct Tags<'read, 'source> {
    prerequisite: &'read Value<'source>,
    tags: &'read [Value<'source>],
}

impl<'read, 'source> Tags<'read, 'source> {
    fn read(parent: &'read StructValue<'source>, name: &str) -> TestResult<Self> {
        let prerequisite = element(parent, name, 0)?;
        let structure = structure(prerequisite)?;
        assert_eq!(
            structure.type_name(),
            "ContractPrerequisite_CompletedContractTags",
            "actual prerequisite type"
        );
        let list = field(structure, "requiredCompletedContractTags")?;
        let Value::Struct(list) = list else {
            return Err("original completion tag list".into());
        };
        Ok(Self {
            prerequisite,
            tags: array(list, "tags")?,
        })
    }

    fn requirement(
        &self,
        source: &Original<'_, '_>,
        slot: usize,
        tag: u128,
    ) -> TestResult<Requirement> {
        let value = self
            .tags
            .get(slot)
            .ok_or("authored prerequisite tag slot")?;
        check_target(value, tag)?;
        Ok(Requirement {
            prerequisite: source.site(self.prerequisite)?,
            tag: source.site(value)?,
        })
    }

    fn dangling(&self, source: &Original<'_, '_>) -> TestResult<Site> {
        let [
            _,
            value @ Value::Error(ValueError::DanglingReference { guid }),
            _,
        ] = self.tags
        else {
            return Err(
                "exactly two resolved prerequisite tags around an original dangling slot".into(),
            );
        };
        assert_eq!(
            guid.to_string(),
            health_origin_fixture::guid(MISSING_TAG),
            "genuine missing target"
        );
        source.site(value)
    }
}

fn check_target(value: &Value<'_>, expected: u128) -> TestResult {
    let Value::Reference(target) = value else {
        return Err("original resolved Reference".into());
    };
    assert_eq!(
        target.guid().to_string(),
        health_origin_fixture::guid(expected),
        "original tag GUID"
    );
    assert!(target.location().is_none(), "a whole-Record target");
    Ok(())
}

fn member<'read, 'source>(
    owner: &'read StructValue<'source>,
    name: &str,
    index: usize,
    expected: u128,
) -> TestResult<&'read StructValue<'source>> {
    let member = structure(element(owner, name, index)?)?;
    let Value::Guid(id) = field(member, "id")? else {
        return Err("original contract id".into());
    };
    assert_eq!(
        id.to_string(),
        health_origin_fixture::guid(expected),
        "original authored member"
    );
    Ok(member)
}

fn field<'read, 'source>(
    parent: &'read StructValue<'source>,
    name: &str,
) -> TestResult<&'read Value<'source>> {
    parent
        .field(name)
        .ok_or_else(|| format!("original field {name}").into())
}

fn array<'read, 'source>(
    parent: &'read StructValue<'source>,
    name: &str,
) -> TestResult<&'read [Value<'source>]> {
    let Value::Array(values) = field(parent, name)? else {
        return Err(format!("original array {name}").into());
    };
    Ok(values)
}

fn element<'read, 'source>(
    parent: &'read StructValue<'source>,
    name: &str,
    index: usize,
) -> TestResult<&'read Value<'source>> {
    array(parent, name)?
        .get(index)
        .ok_or_else(|| format!("original array slot {name}/{index}").into())
}

fn structure<'read, 'source>(
    value: &'read Value<'source>,
) -> TestResult<&'read StructValue<'source>> {
    let Value::Struct(value) = value else {
        return Err("original struct".into());
    };
    Ok(value)
}
