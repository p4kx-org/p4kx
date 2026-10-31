// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The `Requirement` of a loadout assortment or of one of its groups: decided where the Archive
//! decides it.

use p4kx_dataforge::{StructValue, Value};

use crate::read::has_items;

const TAGS: &str = "SLoadoutRequirementTags";
const ANY_OF: &str = "SLoadoutRequirementOR";
const INVERT: &str = "SLoadoutRequirementNOT";

/// Requirements nested deeper than this are undecided, so the recursion is bounded.
const MAX_DEPTH: usize = 16;

/// What the Archive says about a Requirement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Verdict {
    /// It holds for every spawn: no Requirement, or one that restricts nothing.
    Met,
    /// It holds for no spawn.
    Unmet,
    /// It depends on tags that only the game knows at spawn time.
    Undecided,
}

impl Verdict {
    const fn not(self) -> Self {
        match self {
            Self::Met => Self::Unmet,
            Self::Unmet => Self::Met,
            Self::Undecided => Self::Undecided,
        }
    }
}

/// Evaluates the Requirement a field holds; no field or a null pointer is no Requirement.
///
/// A tag Requirement with neither required nor banned tags restricts nothing. Anything that names
/// a tag is [`Verdict::Undecided`]: the Archive does not hold the tags of the spawn. `OR` and
/// `NOT` combine the verdicts of their parts; an `OR` without parts, a `NOT` without a part and
/// a Requirement of a type not known here are undecided.
pub(crate) fn evaluate(requirement: Option<&Value<'_>>) -> Verdict {
    match requirement {
        None | Some(Value::Null) => Verdict::Met,
        Some(value) => evaluate_value(value, 0),
    }
}

fn evaluate_value(value: &Value<'_>, depth: usize) -> Verdict {
    match value {
        Value::Struct(structure) if depth < MAX_DEPTH => evaluate_struct(structure, depth),
        _ => Verdict::Undecided,
    }
}

fn evaluate_struct(structure: &StructValue<'_>, depth: usize) -> Verdict {
    match structure.type_name() {
        TAGS => {
            if has_items(structure, "RequiredTags") || has_items(structure, "BannedTags") {
                Verdict::Undecided
            } else {
                Verdict::Met
            }
        }
        ANY_OF => any_of(structure, depth),
        INVERT => structure
            .field("RequirementToInvert")
            .map_or(Verdict::Undecided, |inner| {
                evaluate_value(inner, depth + 1).not()
            }),
        _ => Verdict::Undecided,
    }
}

fn any_of(structure: &StructValue<'_>, depth: usize) -> Verdict {
    let Some(Value::Array(parts)) = structure.field("Requirements") else {
        return Verdict::Undecided;
    };
    if parts.is_empty() {
        return Verdict::Undecided;
    }
    let mut verdict = Verdict::Unmet;
    for part in parts {
        match evaluate_value(part, depth + 1) {
            Verdict::Met => return Verdict::Met,
            Verdict::Undecided => verdict = Verdict::Undecided,
            Verdict::Unmet => {}
        }
    }
    verdict
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use p4kx_dataforge::DataForge;
    use p4kx_testkit::{
        DataForgeBuilder, TestInstance, TestRecord, TestRequirement, TestValue, entity_schema,
    };

    use super::{MAX_DEPTH, Verdict, evaluate};

    type TestResult = Result<(), Box<dyn Error>>;

    const TAG: u128 = 0x500;

    /// Evaluates the Requirement `value` as the Requirement of the only group of an assortment.
    fn verdict_of(value: TestValue) -> Result<Verdict, Box<dyn Error>> {
        let group = TestInstance::new("SGroupedLoadouts").value("Requirement", value);
        let record = TestRecord::new(
            1,
            "SLoadoutAssortment",
            "SLoadoutAssortment.test",
            "libs/foundry/records/loadoutkits/test.xml",
        )
        .value(
            "GroupedLoadouts",
            TestValue::Array(vec![TestValue::Struct(group)]),
        );
        let bytes = entity_schema(DataForgeBuilder::new())
            .record(record)
            .build();
        let forge = DataForge::open(&bytes)?;
        let tree = forge.records().next().ok_or("no record")?.tree()?;
        let Some(p4kx_dataforge::Value::Array(groups)) = tree.field("GroupedLoadouts") else {
            return Err("no groups".into());
        };
        let Some(p4kx_dataforge::Value::Struct(group)) = groups.first() else {
            return Err("no group".into());
        };
        Ok(evaluate(group.field("Requirement")))
    }

    fn tagged() -> TestRequirement {
        TestRequirement::Tags {
            required: vec![TAG],
            banned: Vec::new(),
        }
    }

    #[test]
    fn no_requirement_and_one_that_names_no_tag_are_met() -> TestResult {
        assert_eq!(evaluate(None), Verdict::Met);
        let nothing = TestRequirement::unrestricted().value();
        assert_eq!(verdict_of(nothing)?, Verdict::Met);
        Ok(())
    }

    #[test]
    fn an_or_of_a_met_and_an_undecided_part_is_met() -> TestResult {
        let any = TestRequirement::AnyOf(vec![tagged(), TestRequirement::unrestricted()]);
        assert_eq!(verdict_of(any.value())?, Verdict::Met);
        Ok(())
    }

    #[test]
    fn a_tag_and_an_unknown_type_are_undecided() -> TestResult {
        assert_eq!(verdict_of(tagged().value())?, Verdict::Undecided);
        let unknown = TestValue::Struct(TestInstance::new("SLoadoutRequirementBase"));
        assert_eq!(verdict_of(unknown)?, Verdict::Undecided);
        Ok(())
    }

    #[test]
    fn a_not_without_a_part_is_undecided() -> TestResult {
        let empty = TestValue::Struct(TestInstance::new("SLoadoutRequirementNOT"));
        assert_eq!(verdict_of(empty)?, Verdict::Undecided);
        Ok(())
    }

    #[test]
    fn nesting_beyond_the_limit_is_undecided() -> TestResult {
        let nest = |depth: usize| {
            (0..depth).fold(TestRequirement::unrestricted(), |part, _| {
                TestRequirement::Not(Box::new(part))
            })
        };
        let within = MAX_DEPTH - 1;
        assert_ne!(verdict_of(nest(within).value())?, Verdict::Undecided);
        assert_eq!(verdict_of(nest(MAX_DEPTH + 1).value())?, Verdict::Undecided);
        Ok(())
    }
}
