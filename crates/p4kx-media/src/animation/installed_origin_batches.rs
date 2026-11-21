// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;
use std::collections::HashSet;
use std::io;

use p4kx_extraction::{
    Archive, Entry, FieldOrigin, InputScope, PhysicalSourceRef, SourceLocation,
    SourceUseDescription, SourceUseRef,
};

use crate::origin_contract::use_sets::SetId;
use crate::origin_input::MediaOriginInput;
use crate::{MediaWriter, RootSource};

use super::{TestResult, require};

#[derive(Default)]
pub(super) struct OriginBatches {
    references: RefCell<HashSet<SourceUseRef>>,
}

impl OriginBatches {
    pub(super) fn field(&self, origin: &FieldOrigin, shared: Option<SetId>) -> io::Result<()> {
        if shared.is_some() {
            return Err(io::Error::other(
                "animation census does not observe shared use sets",
            ));
        }
        let mut references = self.references.try_borrow_mut().map_err(io::Error::other)?;
        for reference in origin.source_uses_refs() {
            if !references.contains(reference) {
                references.try_reserve(1).map_err(io::Error::other)?;
                references.insert(reference.clone());
            }
        }
        Ok(())
    }

    pub(super) fn finish_entry(
        &self,
        archive: &Archive,
        entry: &Entry,
        input: &MediaOriginInput,
        writer: &MediaWriter<'_>,
    ) -> TestResult {
        let observed = archive
            .observed_entry(entry)
            .ok_or("original animation Entry")?;
        require(
            input.scope.matches_entry(&observed)?,
            "animation source scope is bound to the original physical Entry",
        )?;
        let root = writer
            .origins
            .root
            .as_ref()
            .ok_or("verified root Archive")?;
        let mut ancestors = Vec::new();
        for position in 0..observed.binding().ancestor_count() {
            let scope = input
                .ancestor(position)?
                .ok_or("original animation container input")?;
            ancestors.push(scope.scope.source_ref().clone());
        }
        let source = input.scope.source_ref();
        let mut matched_source = false;
        self.drain(writer, |description| {
            let actual = description.source();
            if actual != source && root.scope.source_ref() != actual && !ancestors.contains(actual)
            {
                return Err(io::Error::other(
                    "animation origin refers to another physical source",
                ));
            }
            matched_source |= actual == source;
            check_animation_range(description, source, entry.uncompressed_size())
        })?;
        require(
            matched_source,
            "animation receipt refers to the original physical Entry",
        )
    }

    pub(super) fn finish_build(&self, root: &RootSource, writer: &MediaWriter<'_>) -> TestResult {
        let build = root
            .build_origin
            .as_ref()
            .and_then(|bound| bound.input.as_ref())
            .map(|input| input.scope.source_ref());
        self.drain(writer, |description| {
            if Some(description.source()) != build {
                return Err(io::Error::other(
                    "animation Build origin refers to another physical source",
                ));
            }
            Ok(())
        })
    }

    fn drain(
        &self,
        writer: &MediaWriter<'_>,
        mut check: impl FnMut(&SourceUseDescription) -> io::Result<()>,
    ) -> TestResult {
        let mut references = self.references.try_borrow_mut()?;
        writer.origins.owner.drain_use_batch(|description| {
            if references.remove(description.id()) {
                writer.checkpoint().map_err(io::Error::other)?;
                check(description)?;
            }
            Ok(())
        })?;
        require(
            references.is_empty(),
            "each emitted animation origin has a use in its batch",
        )
    }
}

fn check_animation_range(
    description: &SourceUseDescription,
    entry: &PhysicalSourceRef,
    size: u64,
) -> io::Result<()> {
    if let SourceLocation::Subresource {
        namespace, indices, ..
    } = description.location()
        && namespace == "animation-byte-range"
        && (description.source() != entry
            || description.scope() != InputScope::Payload
            || !matches!(indices.as_slice(), [start, end] if start <= end && *end <= size))
    {
        return Err(io::Error::other(
            "animation coordinate is outside its original payload",
        ));
    }
    Ok(())
}
