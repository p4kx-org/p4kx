// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::io;

use p4kx_extraction::SourceLocation;
use serde_json::Value;

use super::{Forge, Sources, Weapon, sourced_index, xml};

struct Lexicon {
    documents: Vec<u64>,
    names: BTreeMap<String, usize>,
    complete: bool,
}

struct Evidence<'a> {
    originals: &'a BTreeMap<u64, xml::Document>,
    by_path: BTreeMap<String, Vec<u64>>,
}

struct OptionToken<'a> {
    document: u64,
    option: usize,
    ordinal: usize,
    spelling: &'a str,
    original: &'a Value,
}

type Position = (u64, u64, u64);
type Group = (u64, String);

pub(super) fn verify(
    weapons: &BTreeMap<usize, Weapon>,
    scopes: &BTreeMap<u64, Value>,
    postings: &[Value],
    originals: &BTreeMap<u64, xml::Document>,
    forge: &Forge<'_>,
    sources: &Sources<'_>,
) -> io::Result<()> {
    let names = weapon_names(weapons);
    let evidence = Evidence::new(originals);
    let mut coverage = BTreeMap::new();
    for posting in postings {
        let scope = sourced_index(posting, "scope")?;
        let tokens = array(&posting["optionTokens"])?;
        let first = tokens
            .first()
            .ok_or_else(|| io::Error::other("empty weapon motion posting"))?;
        let document = sourced_index(first, "document")?;
        let lexicon = evidence.lexicon(document)?;
        let observed = observed_scope(scopes, scope, &lexicon, posting)?;
        let mut positions = Vec::new();
        let key = check_posting_tokens(
            tokens,
            document,
            &lexicon,
            &evidence,
            sources,
            &mut positions,
        )?;
        let count = lexicon.names.get(&key).copied().unwrap_or_default();
        verify_basis(posting, observed, count, lexicon.complete)?;
        verify_weapon_tokens(posting, &key, &names, weapons, forge, sources)?;
        if coverage.insert((document, key), positions).is_some() {
            return Err(io::Error::other("duplicate scoped weapon posting"));
        }
    }
    if coverage.is_empty() || coverage != evidence.expected(&names)? {
        return Err(io::Error::other(
            "scoped weapon posting coordinates differ from original inventory",
        ));
    }
    Ok(())
}

fn weapon_names(weapons: &BTreeMap<usize, Weapon>) -> BTreeMap<String, Vec<(usize, usize, &str)>> {
    let mut result = BTreeMap::<_, Vec<_>>::new();
    for (&index, weapon) in weapons {
        if let Some(tags) = weapon.tags[1].as_str() {
            for (ordinal, tag) in tags.split_whitespace().enumerate() {
                result
                    .entry(tag.to_ascii_lowercase())
                    .or_default()
                    .push((index, ordinal, tag));
            }
        }
    }
    result
}

fn observed_scope<'a>(
    scopes: &'a BTreeMap<u64, Value>,
    scope: u64,
    lexicon: &Lexicon,
    posting: &Value,
) -> io::Result<&'a Value> {
    let projected = scopes
        .get(&scope)
        .ok_or_else(|| io::Error::other("missing weapon TagDef scope"))?;
    let documents = array(&projected["documents"])?;
    if projected["kind"] != "tags"
        || documents
            .iter()
            .map(Value::as_u64)
            .ne(lexicon.documents.iter().copied().map(Some))
    {
        return Err(io::Error::other(
            "weapon posting uses a different physical TagDef scope",
        ));
    }
    verify_scope_origin(&posting["scope"], projected, lexicon.complete)?;
    Ok(projected)
}

fn verify_scope_origin(actual: &Value, scope: &Value, complete: bool) -> io::Result<()> {
    if actual["state"] != "present"
        || actual["sourceSetComplete"].as_bool() != Some(complete)
        || [
            "containsEncryptedSource",
            "sourceUsesRefs",
            "sourceSetComplete",
        ]
        .iter()
        .any(|&key| actual[key] != scope[key])
    {
        return Err(io::Error::other(
            "weapon join provenance differs from its TagDef scope",
        ));
    }
    Ok(())
}

fn verify_basis(posting: &Value, scope: &Value, count: usize, complete: bool) -> io::Result<()> {
    let expected = match (count, complete) {
        (0, _) => {
            return Err(io::Error::other(
                "no authored TagDef occurrence supports posting",
            ));
        }
        (1, true) => "unique",
        (1, false) => "incomplete",
        _ => "competing",
    };
    if posting["basis"]["value"] != expected {
        return Err(io::Error::other(
            "weapon posting match basis disagrees with original TagDef inputs",
        ));
    }
    verify_scope_origin(&posting["basis"], scope, complete)
}

fn check_posting_tokens(
    raw: &[Value],
    document: u64,
    lexicon: &Lexicon,
    evidence: &Evidence<'_>,
    sources: &Sources<'_>,
    positions: &mut Vec<Position>,
) -> io::Result<String> {
    let source = evidence
        .originals
        .get(&document)
        .ok_or_else(|| io::Error::other("ADB input was not observed"))?;
    let (options, _) = source
        .adb()
        .ok_or_else(|| io::Error::other("weapon token input lacks ADB role"))?;
    let mut key = None;
    for wire in raw {
        let token = OptionToken::read(wire, document, options)?;
        let spelling = token.spelling.to_ascii_lowercase();
        if key.as_ref().is_some_and(|other| *other != spelling)
            || !lexicon.names.contains_key(&spelling)
        {
            return Err(io::Error::other(
                "weapon posting combines different original tokens",
            ));
        }
        key = Some(spelling);
        for field in ["document", "option", "ordinal", "spelling"] {
            source.attribute_field(&wire[field], token.original, "Tags", sources)?;
        }
        positions.push((token.document, token.option as u64, token.ordinal as u64));
    }
    key.ok_or_else(|| io::Error::other("empty weapon motion token group"))
}

impl<'a> OptionToken<'a> {
    fn read(wire: &Value, document: u64, options: &'a [Value]) -> io::Result<Self> {
        let option = usize::try_from(sourced_index(wire, "option")?).map_err(io::Error::other)?;
        let ordinal = usize::try_from(sourced_index(wire, "ordinal")?).map_err(io::Error::other)?;
        let Some(original) = options.get(option).and_then(only_global) else {
            return Err(io::Error::other(
                "posted ADB option has no unique original Tags attribute",
            ));
        };
        let spelling = original[1]
            .as_str()
            .and_then(|text| text.split('+').nth(ordinal))
            .ok_or_else(|| io::Error::other("posted ADB token ordinal is absent"))?;
        if sourced_index(wire, "document")? != document || wire["spelling"]["value"] != spelling {
            return Err(io::Error::other(
                "original ADB token spelling or physical owner differs",
            ));
        }
        Ok(Self {
            document,
            option,
            ordinal,
            spelling,
            original,
        })
    }
}

fn verify_weapon_tokens(
    posting: &Value,
    key: &str,
    names: &BTreeMap<String, Vec<(usize, usize, &str)>>,
    weapons: &BTreeMap<usize, Weapon>,
    forge: &Forge<'_>,
    sources: &Sources<'_>,
) -> io::Result<()> {
    let original = names
        .get(key)
        .ok_or_else(|| io::Error::other("missing original weapon Tags token"))?;
    let tokens = array(&posting["weaponTokens"])?;
    if original.len() != tokens.len() {
        return Err(io::Error::other("weapon token coverage differs"));
    }
    for (wire, &(weapon, ordinal, spelling)) in tokens.iter().zip(original) {
        if sourced_index(wire, "weapon")? != weapon as u64
            || sourced_index(wire, "ordinal")? != ordinal as u64
            || wire["spelling"]["value"] != spelling
        {
            return Err(io::Error::other(
                "weapon token spelling or original ordinal differs",
            ));
        }
        let original = weapons
            .get(&weapon)
            .ok_or_else(|| io::Error::other("weapon source coordinate is absent"))?;
        let component = format!("Components[{}]", original.component);
        for (key, field) in [
            ("weapon", component.clone()),
            ("ordinal", format!("{component}.AttachDef.Tags")),
            ("spelling", format!("{component}.AttachDef.Tags")),
        ] {
            sources.single_field(
                &wire[key],
                &forge.physical,
                &SourceLocation::DataForge {
                    record_index: original.record,
                    field,
                },
            )?;
        }
    }
    Ok(())
}

impl<'a> Evidence<'a> {
    fn new(originals: &'a BTreeMap<u64, xml::Document>) -> Self {
        let mut by_path = BTreeMap::<_, Vec<_>>::new();
        for (&index, original) in originals {
            by_path
                .entry(fold_path(original.path()))
                .or_default()
                .push(index);
        }
        Self { originals, by_path }
    }

    fn lexicon(&self, adb: u64) -> io::Result<Lexicon> {
        let document = self
            .originals
            .get(&adb)
            .ok_or_else(|| io::Error::other("ADB lacks physical source"))?;
        let (_, tag_definition) = document
            .adb()
            .ok_or_else(|| io::Error::other("tag scope owner is not an ADB"))?;
        let mut pending = VecDeque::from([tag_definition]);
        let mut seen = BTreeSet::new();
        let mut lexicon = Lexicon {
            documents: Vec::new(),
            names: BTreeMap::new(),
            complete: true,
        };
        while let Some(selector) = pending.pop_front() {
            let selected = selector
                .and_then(|path| self.by_path.get(&data_path(path)))
                .map(Vec::as_slice);
            let Some([target]) = selected else {
                lexicon.complete = false;
                continue;
            };
            let Some(definition) = self.originals[target].tag_definition() else {
                lexicon.complete = false;
                continue;
            };
            if !seen.insert(*target) {
                continue;
            }
            lexicon.documents.push(*target);
            lexicon.complete &= definition.version == Some("2");
            for name in definition.names {
                if let Some(source) = name[1]
                    .as_array()
                    .filter(|attrs| attrs.len() == 1)
                    .and_then(|attrs| attrs[0][1].as_str())
                    .filter(|name| !name.is_empty())
                {
                    *lexicon
                        .names
                        .entry(source.to_ascii_lowercase())
                        .or_default() += 1;
                } else {
                    lexicon.complete = false;
                }
            }
            pending.extend(definition.imports.iter().map(Option::as_deref));
        }
        Ok(lexicon)
    }

    fn expected(
        &self,
        weapons: &BTreeMap<String, Vec<(usize, usize, &str)>>,
    ) -> io::Result<BTreeMap<Group, Vec<Position>>> {
        let mut rows = BTreeMap::<_, Vec<_>>::new();
        for (&document, source) in self.originals {
            let Some((options, _)) = source.adb() else {
                continue;
            };
            let lexicon = self.lexicon(document)?;
            for (option, raw) in options.iter().enumerate() {
                let Some(text) = only_global(raw).and_then(|attribute| attribute[1].as_str())
                else {
                    continue;
                };
                for (ordinal, token) in text.split('+').enumerate() {
                    let name = token.to_ascii_lowercase();
                    if token.is_ascii()
                        && !token.is_empty()
                        && weapons.contains_key(&name)
                        && lexicon.names.contains_key(&name)
                    {
                        rows.entry((document, name)).or_default().push((
                            document,
                            option as u64,
                            ordinal as u64,
                        ));
                    }
                }
            }
        }
        Ok(rows)
    }
}

fn data_path(value: &str) -> String {
    let normalized = fold_path(value.trim());
    if normalized.starts_with("data/") {
        normalized
    } else {
        format!("data/{normalized}")
    }
}

fn fold_path(value: &str) -> String {
    value.replace('\\', "/").to_ascii_lowercase()
}

fn only_global(option: &Value) -> Option<&Value> {
    let [attribute] = option[0].as_array()?.as_slice() else {
        return None;
    };
    Some(attribute)
}

fn array(value: &Value) -> io::Result<&[Value]> {
    value
        .as_array()
        .map(Vec::as_slice)
        .ok_or_else(|| io::Error::other("weapon occurrence array unavailable"))
}
