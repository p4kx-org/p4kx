// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;
use std::collections::{HashMap, TryReserveError};

use p4kx_extraction::DecimalU64;
use schemars::JsonSchema;
use serde::Serialize;

use crate::audio_control_payload::AtlPosition;
use crate::origin_contributions::OriginContributions;
use crate::origin_input::MediaOriginInput;
use crate::{MediaError, MediaWriter};

#[path = "audio_control_graph_rows.rs"]
pub(crate) mod rows;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum NamedKind {
    PreloadRequest,
    Rtpc,
    ConfigGroup,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum NameScope {
    Directory,
    Preload { entry: usize, element: u64 },
}

#[derive(Clone, Copy)]
pub(crate) struct Site {
    pub(crate) entry: usize,
    pub(crate) indices: [u64; 2],
    pub(crate) position: AtlPosition,
}

#[derive(Clone, Copy)]
pub(crate) struct Named<'document> {
    pub(crate) kind: NamedKind,
    pub(crate) scope: NameScope,
    pub(crate) name: &'document str,
}

#[derive(Default)]
pub(crate) struct Scope {
    index: RefCell<Index>,
}

#[derive(Default)]
struct Index {
    documents: HashMap<usize, Document>,
    targets: HashMap<Key, HashMap<String, Vec<Target>>>,
    queries: Vec<Query>,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct Key {
    kind: NamedKind,
    scope: NameScope,
}

struct Target {
    site: Site,
    input: MediaOriginInput,
}

struct Query {
    key: Key,
    name: String,
    source: Site,
    input: MediaOriginInput,
    evidence: OriginContributions,
}

struct Document {
    complete: bool,
    evidence: OriginContributions,
}

impl Scope {
    pub(crate) fn document(
        &self,
        entry: usize,
        complete: bool,
        evidence: OriginContributions,
    ) -> Result<(), MediaError> {
        let mut index = self.index.try_borrow_mut().map_err(|_| borrowed())?;
        index
            .documents
            .try_reserve(1)
            .map_err(|cause| allocation(&cause))?;
        if index
            .documents
            .insert(entry, Document { complete, evidence })
            .is_some()
        {
            return Err(MediaError::Contract(
                "ATL source was observed more than once".into(),
            ));
        }
        Ok(())
    }

    pub(crate) fn target(
        &self,
        named: Named<'_>,
        site: Site,
        input: &MediaOriginInput,
    ) -> Result<(), MediaError> {
        let mut index = self.index.try_borrow_mut().map_err(|_| borrowed())?;
        index
            .targets
            .try_reserve(1)
            .map_err(|cause| allocation(&cause))?;
        let by_name = index.targets.entry(Key::from(named)).or_default();
        by_name.try_reserve(1).map_err(|cause| allocation(&cause))?;
        let targets = if let Some(targets) = by_name.get_mut(named.name) {
            targets
        } else {
            by_name.entry(owned_name(named.name)?).or_default()
        };
        targets.try_reserve(1).map_err(|cause| allocation(&cause))?;
        targets.push(Target {
            site,
            input: input.clone(),
        });
        Ok(())
    }

    pub(crate) fn request(
        &self,
        named: Named<'_>,
        source: Site,
        input: &MediaOriginInput,
        evidence: OriginContributions,
        writer: &MediaWriter<'_>,
    ) -> Result<DecimalU64, MediaError> {
        writer.checkpoint()?;
        let mut index = self.index.try_borrow_mut().map_err(|_| borrowed())?;
        index
            .queries
            .try_reserve(1)
            .map_err(|cause| allocation(&cause))?;
        let query_index = DecimalU64::new(index.queries.len() as u64);
        index.queries.push(Query {
            key: Key::from(named),
            name: owned_name(named.name)?,
            source,
            input: input.clone(),
            evidence,
        });
        Ok(query_index)
    }
}

impl From<Named<'_>> for Key {
    fn from(named: Named<'_>) -> Self {
        Self {
            kind: named.kind,
            scope: named.scope,
        }
    }
}

fn owned_name(name: &str) -> Result<String, MediaError> {
    let mut owned = String::new();
    owned
        .try_reserve_exact(name.len())
        .map_err(|cause| allocation(&cause))?;
    owned.push_str(name);
    Ok(owned)
}

fn borrowed() -> MediaError {
    MediaError::Contract("ATL directory inventory is already in use".into())
}

fn allocation(cause: &TryReserveError) -> MediaError {
    MediaError::Contract(format!("cannot allocate ATL directory metadata: {cause}"))
}
