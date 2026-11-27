// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::io;

use p4kx_assembly::BodyVariant;
use p4kx_extraction::{Archive, CancelToken, SourceUseRole};
use serde_json::Value;

use crate::PlayerSourceCapture;
use crate::player_bundle_rows;
use crate::player_bundle_wire::Row;

#[path = "player_adb_clip_alias_witness_original.rs"]
mod original;

use self::original::{Animation, Name, Originals, Receipts, member, number};

type Check = Result<(), Box<dyn std::error::Error>>;

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn installed_adb_aliases_reconcile_original_occurrences_and_body_candidates() -> Check {
    let archive = Archive::open_with_key(
        p4kx_testkit::real_archive_path()?,
        Some(p4kx_testkit::real_archive_key()?),
    )
    .map_err(|_| "cannot open selected installed Archive")?;
    let mut originals = Originals::default();
    let capture = PlayerSourceCapture::read(
        &archive,
        &BodyVariant::ALL,
        &CancelToken::new(),
        &mut originals,
    )
    .map_err(|_| "cannot capture selected Player sources")?;
    let receipts = Receipts::new(&capture.source_inventory, &originals)?;
    let mut projection = Projection::new(&originals, receipts);
    player_bundle_rows::write(&capture, |row| projection.verify(row))?;
    projection.finish()
}

struct Group<'a> {
    key: String,
    original: &'a Animation,
    name: &'a Name,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct ClipId {
    rig: usize,
    rule: usize,
    chunk: usize,
    record: usize,
}

impl ClipId {
    fn read(rig: usize, value: &Value) -> io::Result<Self> {
        Ok(Self {
            rig,
            rule: number(value, "rule")?,
            chunk: number(value, "chunk")?,
            record: number(value, "record")?,
        })
    }
}

struct Candidate {
    identity: ClipId,
    group: usize,
    path_uses: Vec<String>,
}

struct Projection<'a> {
    originals: &'a Originals,
    receipts: Receipts<'a>,
    occurrences: VecDeque<&'a Animation>,
    groups: Vec<Group<'a>>,
    by_key: BTreeMap<String, usize>,
    groups_seen: usize,
    fragments_seen: BTreeSet<(usize, usize)>,
    rigs: BTreeSet<usize>,
    matched: BTreeMap<usize, usize>,
    rules: BTreeMap<(usize, usize), Vec<String>>,
    candidates: VecDeque<Candidate>,
}

impl<'a> Projection<'a> {
    fn new(originals: &'a Originals, receipts: Receipts<'a>) -> Self {
        let occurrences: VecDeque<_> = originals.animations().collect();
        let mut groups = Vec::new();
        let mut by_key = BTreeMap::new();
        for &original in &occurrences {
            if let [name] = original.names.as_slice()
                && let Some(key) = bare_alias(&name.spelling)
            {
                by_key.entry(key.clone()).or_insert_with(|| {
                    let index = groups.len();
                    groups.push(Group {
                        key,
                        original,
                        name,
                    });
                    index
                });
            }
        }
        Self {
            originals,
            receipts,
            occurrences,
            groups,
            by_key,
            groups_seen: 0,
            fragments_seen: BTreeSet::new(),
            rigs: BTreeSet::new(),
            matched: BTreeMap::new(),
            rules: BTreeMap::new(),
            candidates: VecDeque::new(),
        }
    }

    fn verify(&mut self, row: &Row<'_>) -> io::Result<()> {
        match row {
            Row::AdbFragment {
                document,
                index,
                value,
            } => self.fragment(*document, *index, &serde_json::to_value(value)?),
            Row::AdbClipAliasGroup { index, value } => {
                self.group(*index, &serde_json::to_value(value)?)
            }
            Row::AdbClipAliasOccurrence { value } => self.occurrence(&serde_json::to_value(value)?),
            Row::RigClipRig { index, value } => self.rig(*index, &serde_json::to_value(value)?),
            Row::RigClipRule { rig, index, value } => {
                let wire = serde_json::to_value(value)?;
                if wire.pointer("/kind/value/type").and_then(Value::as_str) == Some("recursiveCaf")
                {
                    self.rules
                        .insert((*rig, *index), original::references(&wire["kind"])?);
                }
                Ok(())
            }
            Row::RigClipCandidate { rig, value } => self.clip(*rig, &serde_json::to_value(value)?),
            Row::RigClipAliasCandidate { value } => self.candidate(&serde_json::to_value(value)?),
            _ => Ok(()),
        }
    }

    fn fragment(&mut self, document: usize, index: usize, value: &Value) -> io::Result<()> {
        let fragment = self.originals.fragment(document, index)?;
        let coordinate = [
            number(value, "listOrdinal")?,
            number(value, "groupOrdinal")?,
            number(value, "optionOrdinal")?,
        ];
        if coordinate != fragment.coordinate || !self.fragments_seen.insert((document, index)) {
            return Err(io::Error::other(
                "ADB fragment address differs from original",
            ));
        }
        let mut remaining = fragment.animations.iter();
        for layer in original::array(value, "layers")? {
            for animation in original::array(layer, "animations")? {
                let original = remaining
                    .next()
                    .ok_or_else(|| io::Error::other("unexpected authored animation"))?;
                let attributes = original::array(&animation["name"], "occurrences")?;
                if original.layer != number(layer, "ordinal")?
                    || original.ordinal != number(animation, "ordinal")?
                    || !attributes
                        .iter()
                        .map(|row| row.pointer("/value/value").and_then(Value::as_str))
                        .eq(original
                            .names
                            .iter()
                            .map(|name| Some(name.spelling.as_str())))
                {
                    return Err(io::Error::other(
                        "original Animation order or authored alias spelling differs",
                    ));
                }
                for (field, name) in attributes.iter().zip(&original.names) {
                    self.receipts.xml(
                        &field["value"],
                        original,
                        &name.position,
                        SourceUseRole::Payload,
                    )?;
                }
            }
        }
        if remaining.next().is_some() {
            return Err(io::Error::other("omitted authored Animation"));
        }

        Ok(())
    }

    fn group(&mut self, index: usize, value: &Value) -> io::Result<()> {
        let group = self
            .groups
            .get(index)
            .ok_or_else(|| io::Error::other("unexpected alias group"))?;
        if index != self.groups_seen
            || value.pointer("/aliasKey/value").and_then(Value::as_str) != Some(group.key.as_str())
            || value["controllerRigBinding"] != "unresolved"
        {
            return Err(io::Error::other(
                "first authored alias order or candidate-only state differs",
            ));
        }
        self.receipts.xml(
            &value["aliasKey"],
            group.original,
            &group.name.position,
            SourceUseRole::Payload,
        )?;
        self.groups_seen += 1;
        Ok(())
    }

    fn occurrence(&mut self, value: &Value) -> io::Result<()> {
        let original = self
            .occurrences
            .pop_front()
            .ok_or_else(|| io::Error::other("unexpected alias occurrence"))?;
        if number(value, "document")? != original.document
            || number(value, "fragment")? != original.fragment
            || number(&value["layer"], "value")? != original.layer
            || number(&value["animation"], "value")? != original.ordinal
        {
            return Err(io::Error::other(
                "alias receipt addresses a different original animation",
            ));
        }
        self.receipts.xml(
            &value["layer"],
            original,
            &original.layer_position,
            SourceUseRole::Payload,
        )?;
        self.receipts.xml(
            &value["animation"],
            original,
            &original.position,
            SourceUseRole::Payload,
        )?;
        let field = &value["group"];
        match original.names.as_slice() {
            [name] => {
                let Some(key) = bare_alias(&name.spelling) else {
                    return self.unavailable(field, original, "unsupported");
                };
                let group = self
                    .by_key
                    .get(&key)
                    .ok_or_else(|| io::Error::other("alias group missing"))?;
                if field["state"] != "present" || number(field, "value")? != *group {
                    return Err(io::Error::other(
                        "authored alias linked to a different group",
                    ));
                }
                self.receipts
                    .xml(field, original, &name.position, SourceUseRole::Payload)?;
                let representative = self
                    .groups
                    .get(*group)
                    .ok_or_else(|| io::Error::other("missing first authored alias"))?;
                self.receipts
                    .name(field, representative.original, representative.name)
            }
            [] => self.unavailable(field, original, "absent"),

            _ => self.unavailable(field, original, "invalid"),
        }
    }

    fn unavailable(&self, field: &Value, original: &Animation, state: &str) -> io::Result<()> {
        if field["state"] != state || !field["value"].is_null() {
            return Err(io::Error::other(
                "authored unusable alias disposition differs",
            ));
        }
        self.receipts
            .xml(field, original, &original.position, SourceUseRole::Payload)?;
        for name in &original.names {
            self.receipts
                .xml(field, original, &name.position, SourceUseRole::Payload)?;
        }
        Ok(())
    }

    fn rig(&mut self, index: usize, value: &Value) -> io::Result<()> {
        let body = BodyVariant::ALL
            .get(index)
            .ok_or_else(|| io::Error::other("unselected rig index"))?;
        if value["bodyVariant"].as_str() != Some(body.code()) || !self.rigs.insert(index) {
            return Err(io::Error::other(
                "rig index does not bind the selected body variant",
            ));
        }
        Ok(())
    }

    fn clip(&mut self, rig: usize, value: &Value) -> io::Result<()> {
        let path = member(&value["path"], "value")?;
        let Some(group) = path
            .rsplit(['/', '\\'])
            .next()
            .and_then(|file| file.rsplit_once('.'))
            .filter(|(_, extension)| extension.eq_ignore_ascii_case("caf"))
            .and_then(|(base, _)| bare_alias(base))
            .and_then(|key| self.by_key.get(&key))
            .copied()
        else {
            return Ok(());
        };
        let identity = ClipId::read(rig, value)?;
        let path_uses = original::references(&value["path"])?;
        self.receipts.caf_path(&path_uses)?;
        self.candidates.push_back(Candidate {
            identity,
            group,
            path_uses,
        });
        Ok(())
    }

    fn candidate(&mut self, value: &Value) -> io::Result<()> {
        let expected = self
            .candidates
            .pop_front()
            .ok_or_else(|| io::Error::other("extra physical alias candidate"))?;
        let actual = ClipId::read(number(value, "rig")?, value)?;
        let field = &value["group"];
        if actual != expected.identity
            || field["state"] != "present"
            || number(field, "value")? != expected.group
            || !self.rigs.contains(&actual.rig)
        {
            return Err(io::Error::other(
                "alias candidate does not address the original rig/rule/chunk/record",
            ));
        }
        let group = self
            .groups
            .get(expected.group)
            .ok_or_else(|| io::Error::other("unknown candidate alias group"))?;
        self.receipts.name(field, group.original, group.name)?;
        let rule = self
            .rules
            .get(&(actual.rig, actual.rule))
            .ok_or_else(|| io::Error::other("missing original recursive clip rule"))?;
        original::includes(
            field,
            expected.path_uses.iter().chain(rule).map(String::as_str),
        )?;
        *self.matched.entry(actual.rig).or_default() += 1;
        Ok(())
    }

    fn finish(self) -> Check {
        if self.fragments_seen.len() != self.originals.fragment_count()
            || self.groups_seen != self.groups.len()
            || !self.occurrences.is_empty()
            || !self.candidates.is_empty()
        {
            return Err(
                "installed alias projection omitted an original declaration or candidate".into(),
            );
        }
        if self.groups_seen == 0 {
            return Err(
                "installed coverage gap: selected ADBs contain no comparable aliases".into(),
            );
        }
        for (index, body) in BodyVariant::ALL.into_iter().enumerate() {
            if !self.matched.contains_key(&index) {
                return Err(format!(
                    "installed coverage gap: no observed alias-to-CAF candidate for {}",
                    body.code()
                )
                .into());
            }
        }
        Ok(())
    }
}

fn bare_alias(input: &str) -> Option<String> {
    let comparable = !input.is_empty()
        && input.trim() == input
        && input
            .bytes()
            .all(|byte| byte.is_ascii() && !byte.is_ascii_control() && !b"/\\*?:".contains(&byte));
    comparable.then(|| input.to_ascii_lowercase())
}
