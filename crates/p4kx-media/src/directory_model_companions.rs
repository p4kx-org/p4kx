// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{HashMap, TryReserveError};

use p4kx_extraction::{Archive, Entry};

use crate::MediaError;
use crate::lookup_work::LookupWork;

pub(super) struct ModelCompanionKeys {
    positions: HashMap<String, Vec<usize>>,
}

impl ModelCompanionKeys {
    pub(super) fn read(archive: &Archive, work: &LookupWork<'_>) -> Result<Self, MediaError> {
        let mut positions = HashMap::<String, Vec<usize>>::new();
        for entry in archive.entries() {
            work.charge(1)?;
            let Some(base) = base_key(entry.path()) else {
                continue;
            };
            if !positions.contains_key(&base) {
                positions
                    .try_reserve(1)
                    .map_err(|cause| allocation(&cause))?;
            }
            let group = positions.entry(base).or_default();
            group.try_reserve(1).map_err(|cause| allocation(&cause))?;
            group.push(entry.index());
        }
        Ok(Self { positions })
    }

    pub(super) fn matching<'a>(
        &'a self,
        archive: &'a Archive,
        model: &str,
        work: &LookupWork<'_>,
    ) -> Result<ModelCompanionMatches<'a>, MediaError> {
        let key = normalized(model);
        let positions = self.positions.get(&key).map_or(&[][..], Vec::as_slice);
        work.charge(1 + positions.len() as u64)?;
        Ok(ModelCompanionMatches { archive, positions })
    }
}

pub(crate) struct ModelCompanionMatches<'source> {
    archive: &'source Archive,
    positions: &'source [usize],
}

impl<'source> ModelCompanionMatches<'source> {
    pub(crate) fn entries(&self) -> impl Iterator<Item = Result<&'source Entry, MediaError>> + '_ {
        self.positions.iter().map(|&index| {
            self.archive.entries().get(index).ok_or_else(|| {
                MediaError::Contract("indexed model companion Entry disappeared".into())
            })
        })
    }
}

pub(crate) fn source_level(path: &str) -> Option<&str> {
    let (base, _) = extension(path)?;
    let (_, digits) = lod_suffix(base)?;
    Some(digits)
}

fn base_key(path: &str) -> Option<String> {
    let (stem, extension) = extension(path)?;
    let model = model_extension(extension)?;
    let (stem, level) = lod_suffix(stem).map_or((stem, None), |(stem, level)| (stem, Some(level)));
    if level.is_none() && extension.eq_ignore_ascii_case(model) {
        return None;
    }
    let base = format!("{stem}.{model}");
    Some(normalized(&base))
}

fn extension(path: &str) -> Option<(&str, &str)> {
    let (name, extension) = path.rsplit_once('.')?;
    (!extension.contains(['/', '\\'])).then_some((name, extension))
}

fn lod_suffix(stem: &str) -> Option<(&str, &str)> {
    let (base, suffix) = stem.rsplit_once('_')?;
    let marker = suffix.get(..3)?;
    let digits = suffix.get(3..)?;
    (marker.eq_ignore_ascii_case("lod")
        && !digits.is_empty()
        && digits.bytes().all(|byte| byte.is_ascii_digit()))
    .then_some((base, digits))
}

fn model_extension(extension: &str) -> Option<&'static str> {
    [
        ("cgf", "cgfm"),
        ("cga", "cgam"),
        ("chr", "chrm"),
        ("skin", "skinm"),
    ]
    .into_iter()
    .find_map(|(model, geometry)| {
        (extension.eq_ignore_ascii_case(model) || extension.eq_ignore_ascii_case(geometry))
            .then_some(model)
    })
}

pub(crate) fn normalized(path: &str) -> String {
    path.replace('\\', "/").to_ascii_lowercase()
}

fn allocation(cause: &TryReserveError) -> MediaError {
    MediaError::Contract(format!(
        "cannot allocate the model companion index: {cause}"
    ))
}
