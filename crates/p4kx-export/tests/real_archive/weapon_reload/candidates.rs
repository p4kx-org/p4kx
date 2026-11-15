// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Entry;
use serde_json::Value;

use crate::real_archive_item_durability::{number, rows, text};

use super::options::OptionRow;
use super::proof::Inventory;
use super::scopes::{self, Declaration, Scope};
use super::{Attachment, Motion, TestResult, unique};

pub(super) struct Branch<'call, 'catalog> {
    pub(super) parent: &'call Entry,
    pub(super) child: &'call Entry,
    pub(super) position: &'call Value,
    pub(super) actual: &'catalog [Value],
    pub(super) weapon_uses: &'catalog [Value],
    pub(super) inventory: &'call Inventory<'catalog>,
}

struct ExpectedTag<'scope> {
    option_token: usize,
    weapon_token: usize,
    declaration: &'scope Declaration,
}

impl Branch<'_, '_> {
    pub(super) fn compare(
        &self,
        options: &[OptionRow<'_, '_>],
        bound: &[&str],
        tags: &Scope,
        weapon: &Attachment<'_>,
        motion: &Motion<'_>,
    ) -> TestResult<usize> {
        let owner = u64::from(weapon.index);
        let parent = u64::try_from(self.parent.index())?;
        let candidates = self
            .actual
            .iter()
            .filter(|row| {
                row.pointer("/weaponRecordIndex/value")
                    .and_then(Value::as_u64)
                    == Some(owner)
                    && row
                        .pointer("/branchDatabase/entryIndex")
                        .and_then(Value::as_u64)
                        == Some(parent)
                    && row.pointer("/branchPosition/value") == Some(self.position)
                    && row
                        .pointer("/optionPosition/value")
                        .is_some_and(|position| {
                            options.iter().any(|option| {
                                option.row.pointer("/position/value") == Some(position)
                            })
                        })
            })
            .collect::<Vec<_>>();
        let mut seen = 0;
        for option in options.iter().filter(|option| {
            bound
                .iter()
                .any(|name| option.group.eq_ignore_ascii_case(name))
        }) {
            let expected = matches(option, tags, weapon);
            if expected.is_empty() {
                continue;
            }
            let position = option
                .row
                .pointer("/position/value")
                .ok_or("original option source")?;
            let row = unique(
                candidates
                    .iter()
                    .copied()
                    .filter(|row| row.pointer("/optionPosition/value") == Some(position)),
                "exact candidate for original bound option and tag",
            )?;
            self.check_candidate(row, option, &expected, weapon, motion)?;
            seen += 1;
        }
        assert!(
            candidates.len() == seen,
            "only source-bound tag matches are published"
        );
        Ok(seen)
    }

    fn check_candidate(
        &self,
        row: &Value,
        option: &OptionRow<'_, '_>,
        expected: &[ExpectedTag<'_>],
        weapon: &Attachment<'_>,
        motion: &Motion<'_>,
    ) -> TestResult {
        assert!(
            number(row, "/attachmentComponentOrdinal/value")? == u64::try_from(weapon.ordinal)?
                && number(row, "/playerRecordIndex/value")? == u64::from(motion.index)
                && number(row, "/playerComponentOrdinal/value")? == u64::try_from(motion.ordinal)?
                && number(row, "/optionDatabase/entryIndex")? == u64::try_from(self.child.index())?,
            "candidate links exact weapon, actor and option physical owners",
        );
        self.inventory
            .verify_entry(text(row, "/branchDatabase/inputRef")?, self.parent)?;
        self.inventory
            .verify_entry(text(row, "/optionDatabase/inputRef")?, self.child)?;
        let matches = rows(row, "/matches")?;
        assert!(
            matches.len() == expected.len(),
            "all original shared tag tokens survive"
        );
        for (actual, expected) in matches.iter().zip(expected) {
            assert!(
                number(actual, "/weaponToken/value")? == u64::try_from(expected.weapon_token)?
                    && number(actual, "/optionToken/value")?
                        == u64::try_from(expected.option_token)?
                    && number(actual, "/source/entryIndex")?
                        == u64::try_from(expected.declaration.entry)?
                    && actual.pointer("/declaration/value") == Some(&expected.declaration.position),
                "each token pair links its exact physical TagDef declaration",
            );
            self.inventory.declaration(
                text(actual, "/source/inputRef")?,
                expected.declaration,
                &actual["declaration"],
            )?;
        }
        self.check_status(row, option)
    }

    fn check_status(&self, row: &Value, option: &OptionRow<'_, '_>) -> TestResult {
        assert!(
            text(row, "/tagEvidence/value")? == "sharedDeclaration"
                && text(row, "/playback/value")? == "unverified"
                && row.get("reloadDuration").is_none(),
            "scope evidence keeps playback explicitly unverified",
        );
        self.inventory.closure(&row["tagEvidence"])?;
        self.inventory.closure(&row["playback"])?;
        let original_tags = unique(
            rows(option.row, "/attributes")?.iter().filter(|attr| {
                attr.pointer("/name/value").and_then(Value::as_str) == Some("Tags")
                    && attr.pointer("/namespace/value") == Some(&Value::Null)
            }),
            "one original shared tag contribution from the selected option",
        )?;
        let use_refs = rows(row, "/tagEvidence/sourceUsesRefs")?;
        for used in rows(&original_tags["value"], "/sourceUsesRefs")?
            .iter()
            .chain(self.weapon_uses)
        {
            assert!(
                use_refs.contains(used),
                "candidate includes the exact option and weapon tag contributions",
            );
        }
        Ok(())
    }
}

fn matches<'scope>(
    option: &OptionRow<'_, '_>,
    tags: &'scope Scope,
    weapon: &Attachment<'_>,
) -> Vec<ExpectedTag<'scope>> {
    let Some(option_names) = scopes::source_attribute(option.original, "Tags") else {
        return Vec::new();
    };
    if !tags.complete {
        return Vec::new();
    }
    let mut matches = Vec::new();
    for (option_token, name) in option_names.split('+').enumerate() {
        let name = name.trim();
        let [declaration] = tags.named(name) else {
            continue;
        };
        for (weapon_token, weapon_name) in weapon.tags.split_whitespace().enumerate() {
            if weapon_name.eq_ignore_ascii_case(name) {
                matches.push(ExpectedTag {
                    option_token,
                    weapon_token,
                    declaration,
                });
            }
        }
    }
    matches
}
