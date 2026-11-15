// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;
use std::io;

use p4kx_extraction::model::{MaterialError, MaterialLibrary};
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::artifact::StreamObserver;
use crate::{
    Artifact, Identity, MediaError, MediaKind, MediaRecord, Source, material_record_schema,
    reconstruct_material_library,
};

use super::fields::{self, Counts};
use super::input::Inputs;
use super::{TestResult, require};

pub(super) struct Sink {
    schema: jsonschema::Validator,
    state: RefCell<State>,
}

struct State {
    inputs: Inputs,
    counts: Counts,
    sources: Rows<Source>,
    records: Rows<MediaRecord>,
    artifacts: Rows<Artifact>,
}

impl Sink {
    pub(super) fn new(inputs: Inputs) -> TestResult<Self> {
        let counts = Counts {
            enumerated: inputs.enumerated,
            ..Counts::default()
        };
        Ok(Self {
            schema: jsonschema::validator_for(&material_record_schema()?)?,
            state: RefCell::new(State {
                inputs,
                counts,
                records: Rows::new(),
                sources: Rows::new(),
                artifacts: Rows::new(),
            }),
        })
    }

    pub(super) fn check_entry(
        &self,
        expected: &Source,
        library: &Result<MaterialLibrary, MaterialError>,
        size: u64,
    ) -> Result<(), MediaError> {
        self.state
            .try_borrow_mut()
            .map_err(|error| MediaError::Contract(error.to_string()))?
            .check_entry(expected, library, size)
            .map_err(|error| MediaError::Contract(error.to_string()))
    }

    pub(super) fn finish(self) -> TestResult<Counts> {
        let state = self.state.into_inner();
        require(state.inputs.selected.is_empty(), "missing material sources")?;
        require(
            state.sources.ready.is_empty()
                && state.records.ready.is_empty()
                && state.artifacts.ready.is_empty(),
            "unexpected trailing material rows",
        )?;
        Ok(state.counts)
    }
}

impl StreamObserver for Sink {
    fn write(&self, relative: &str, bytes: &[u8]) -> io::Result<()> {
        let mut state = self.state.try_borrow_mut().map_err(io::Error::other)?;
        match relative {
            "sources.jsonl" => state
                .sources
                .push(bytes, |line| serde_json::from_slice(line)),
            "artifacts.jsonl" => state
                .artifacts
                .push(bytes, |line| serde_json::from_slice(line)),
            "material.jsonl" => state.records.push(bytes, |line| self.record(line)),
            _ => Ok(()),
        }
    }

    fn finish(&self, relative: &str) -> io::Result<()> {
        let state = self.state.try_borrow().map_err(io::Error::other)?;
        let complete = match relative {
            "sources.jsonl" => state.sources.pending.is_empty(),
            "artifacts.jsonl" => state.artifacts.pending.is_empty(),
            "material.jsonl" => state.records.pending.is_empty(),
            _ => true,
        };
        if complete {
            Ok(())
        } else {
            Err(io::Error::other("unterminated material observation row"))
        }
    }
}

impl Sink {
    fn record(&self, line: &[u8]) -> io::Result<MediaRecord> {
        let wire: Value = serde_json::from_slice(line)?;
        if !self.schema.is_valid(&wire) {
            return Err(io::Error::other(
                "actual material record failed schema validation",
            ));
        }
        Ok(serde_json::from_value(wire)?)
    }
}

impl State {
    fn check_entry(
        &mut self,
        expected: &Source,
        library: &Result<MaterialLibrary, MaterialError>,
        size: u64,
    ) -> TestResult {
        let [source] = self.sources.ready.as_slice() else {
            return Err("one manifest row per selected material".into());
        };
        let [artifact] = self.artifacts.ready.as_slice() else {
            return Err("one raw receipt per selected material".into());
        };
        let identity = self
            .inputs
            .selected
            .pop_front()
            .ok_or("extra material source")?;
        require(
            source.identity == identity,
            "complete original container and Entry identity",
        )?;
        fields::check_source(source, expected)?;
        fields::check_artifact(artifact, source, size)?;
        for row in &self.records.ready {
            require(
                row.kind == MediaKind::Material,
                "actual material record kind",
            )?;
            fields::check_source(&row.source, source)?;
        }
        let combined = reconstruct_material_library(&self.records.ready)?;
        self.counts.account(&combined, library.as_ref())?;
        self.counts.selected += 1;
        self.counts.nested += u64::from(
            matches!(identity, Identity::Entry { containers, .. } if !containers.is_empty()),
        );
        self.counts.records += u64::try_from(self.records.ready.len())?;
        self.sources.ready.clear();
        self.artifacts.ready.clear();
        self.records.ready.clear();
        Ok(())
    }
}

struct Rows<T> {
    pending: Vec<u8>,
    ready: Vec<T>,
}

impl<T: DeserializeOwned> Rows<T> {
    const fn new() -> Self {
        Self {
            pending: Vec::new(),
            ready: Vec::new(),
        }
    }

    fn push<E: Into<io::Error>>(
        &mut self,
        bytes: &[u8],
        mut parse: impl FnMut(&[u8]) -> Result<T, E>,
    ) -> io::Result<()> {
        for piece in bytes.split_inclusive(|&byte| byte == b'\n') {
            if piece.last() != Some(&b'\n') {
                self.pending.extend_from_slice(piece);
            } else if self.pending.is_empty() {
                self.ready.push(parse(piece).map_err(Into::into)?);
            } else {
                self.pending.extend_from_slice(piece);
                self.ready.push(parse(&self.pending).map_err(Into::into)?);
                self.pending.clear();
            }
        }
        Ok(())
    }
}
