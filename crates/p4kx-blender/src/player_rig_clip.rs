// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::BodyVariant;
use p4kx_extraction::{CancelToken, PathIndex, SourceUse, SourceUses, SourceValue, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use crate::player_source::{PlayerSourceError, check};
use crate::soc_scene_class_resources::{ResourceKind, resource_path};
use crate::soc_scene_xml::XmlDocuments;

#[path = "player_rig_clip_bank.rs"]
mod bank;
#[path = "player_rig_clip_image.rs"]
mod image;
#[path = "player_rig_clip_input.rs"]
mod input;
#[path = "player_rig_clip_rules.rs"]
mod rules;

pub(crate) use bank::Bank;
pub(crate) use rules::Rule;

use bank::{BankMatch, Banks, Cache};
use image::Image;
use input::{Inputs, Lookup};
use rules::Kind;

#[derive(Debug, Default, Serialize)]
pub(crate) struct RigClips {
    pub(crate) rigs: Vec<Rig>,
    pub(crate) rules: Vec<(usize, usize, Rule)>,
    pub(crate) banks: Vec<(usize, usize, Bank)>,
    pub(crate) clips: Vec<(usize, Clip)>,
    pub(crate) image: Option<ImageInventory>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Rig {
    body_variant: &'static str,
    skeleton: SourcedValue<Lookup>,
    parameters: Option<SourcedValue<Lookup>>,
    animation_list: Option<SourcedValue<usize>>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ImageInventory {
    target: SourcedValue<Lookup>,
    table: Option<SourcedValue<()>>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Clip {
    rule: usize,
    chunk: usize,
    record: usize,
    path: SourcedValue<String>,
    alias_key: SourcedValue<String>,
    bank_crc32: SourcedValue<u32>,
    source: SourcedValue<ClipSources>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "camelCase")]
enum ClipSources {
    Loose { target: SourcedValue<Lookup> },
    Database { candidates: Vec<BankMatch> },
}

struct RigRules {
    rig: Rig,
    rules: Vec<Rule>,
    banks: Banks,
}

pub(crate) fn read(
    paths: &PathIndex<'_>,
    variants: &[BodyVariant],
    sources: &SourceUses,
    directory: &SourceUse,
    cancel: &CancelToken,
    documents: &mut XmlDocuments<'_>,
) -> Result<RigClips, PlayerSourceError> {
    let inputs = Inputs {
        paths,
        sources,
        directory,
        cancel,
    };
    let mut cache = Cache::default();
    let rigs = variants
        .iter()
        .map(|&variant| RigRules::read(&inputs, variant, &mut cache, documents))
        .collect::<Result<Vec<_>, _>>()?;
    let mut capture = RigClips::default();
    if rigs
        .iter()
        .flat_map(|rig| &rig.rules)
        .any(|rule| matches!(rule.kind.value(), Some(Kind::RecursiveCaf { .. })))
    {
        let (target, table) = image::read(&inputs, |image| {
            for (index, rig) in rigs.iter().enumerate() {
                rig.project(&inputs, &cache, &image, index, &mut capture.clips)?;
            }
            Ok(())
        })?;
        capture.image = Some(ImageInventory { target, table });
    }
    for (index, rig) in rigs.into_iter().enumerate() {
        capture.rigs.push(rig.rig);
        capture.rules.extend(
            rig.rules
                .into_iter()
                .enumerate()
                .map(|(ordinal, rule)| (index, ordinal, rule)),
        );
        capture.banks.extend(
            rig.banks
                .rows
                .into_iter()
                .enumerate()
                .map(|(ordinal, bank)| (index, ordinal, bank)),
        );
    }
    Ok(capture)
}

impl RigRules {
    fn read(
        inputs: &Inputs<'_, '_>,
        variant: BodyVariant,
        cache: &mut Cache,
        documents: &mut XmlDocuments<'_>,
    ) -> Result<Self, PlayerSourceError> {
        let path = resource_path(variant.skeleton(), ResourceKind::DataRelative)
            .ok_or(PlayerSourceError::BodyVariant)?;
        let stem = path
            .strip_suffix(".chr")
            .ok_or(PlayerSourceError::BodyVariant)?;
        let entries = inputs.entries(&path)?;
        let skeleton = inputs.lookup(&entries)?;
        let mut rig = Rig {
            body_variant: variant.code(),
            skeleton,
            parameters: None,
            animation_list: None,
        };
        let mut rules = Vec::new();
        if matches!(entries.as_slice(), [_]) {
            let entries = inputs.entries(&format!("{stem}.chrparams"))?;
            rig.parameters = Some(inputs.lookup(&entries)?);
            if let [entry] = entries.as_slice() {
                let parsed = rules::read(inputs, entry, documents)?;
                rig.animation_list = Some(parsed.state);
                rules = parsed.rules;
            }
        }
        let banks = Banks::read(inputs, &rules, cache)?;
        Ok(Self { rig, rules, banks })
    }

    fn project(
        &self,
        inputs: &Inputs<'_, '_>,
        cache: &Cache,
        image: &Image<'_>,
        index: usize,
        clips: &mut Vec<(usize, Clip)>,
    ) -> Result<(), PlayerSourceError> {
        let alias = image
            .canonical
            .rsplit('/')
            .next()
            .and_then(|file| file.strip_suffix(".caf"))
            .and_then(alias_key);
        for (rule, authored) in self.rules.iter().enumerate() {
            check(inputs.cancel)?;
            let Some(Kind::RecursiveCaf {
                canonical: folder, ..
            }) = authored.kind.value()
            else {
                continue;
            };
            if !image
                .canonical
                .strip_prefix(folder)
                .is_some_and(|suffix| suffix.starts_with('/'))
            {
                continue;
            }
            clips.push((
                index,
                self.clip(inputs, cache, image, (rule, authored), alias.clone())?,
            ));
        }
        Ok(())
    }

    fn clip(
        &self,
        inputs: &Inputs<'_, '_>,
        cache: &Cache,
        image: &Image<'_>,
        (rule, authored): (usize, &Rule),
        alias: Option<String>,
    ) -> Result<Clip, PlayerSourceError> {
        let path = inputs.bytes(
            image.source,
            image.clip.path_field_range(),
            "cafImagePath",
            SourceValue::Present(image.path.to_owned()),
            true,
        )?;
        let word = image
            .clip
            .dba_path_crc32()
            .ok_or(PlayerSourceError::Binding)?;
        let bank_crc32 = inputs.bytes(
            image.source,
            word.range(),
            "cafImageDbaPathCrc32",
            SourceValue::Present(word.value),
            true,
        )?;
        let alias_key = inputs.sources.value_with_origins(
            alias.map_or(SourceValue::Unsupported, SourceValue::Present),
            &[path.origin(), authored.kind.origin()],
        )?;
        let origins = &[path.origin(), bank_crc32.origin(), authored.kind.origin()];
        let source = if word.value == 0 {
            let target = inputs.lookup(&inputs.entries(&image.canonical)?)?;
            let mut evidence = origins.to_vec();
            evidence.push(target.origin());
            let origins = inputs
                .sources
                .value_with_origins(SourceValue::Present(()), &evidence)?;
            origins.map(|()| ClipSources::Loose { target })
        } else {
            self.banks
                .matches(
                    inputs,
                    cache,
                    (authored.list, word.value),
                    &image.canonical,
                    origins,
                )?
                .map(|candidates| ClipSources::Database { candidates })
        };
        Ok(Clip {
            rule,
            chunk: image.chunk,
            record: image.record,
            path,
            alias_key,
            bank_crc32,
            source,
        })
    }
}

pub(crate) fn alias_key(name: &str) -> Option<String> {
    (!name.is_empty()
        && name == name.trim()
        && name.is_ascii()
        && !name.contains(['/', '\\', '*', '?', ':'])
        && !name.bytes().any(|byte| byte.is_ascii_control()))
    .then(|| name.to_ascii_lowercase())
}
#[cfg(test)]
#[path = "player_rig_clip_witness.rs"]
mod witness;
