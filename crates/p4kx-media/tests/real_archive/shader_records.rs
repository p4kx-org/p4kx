// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Exact source and declaration identity checks for produced shader rows.

use std::collections::BTreeSet;

use p4kx_media::{
    MediaRecord, ShaderDeclaration, ShaderDefinition, ShaderParentNamespace, ShaderPayload,
    ShaderProperty, Status,
};
use serde::Deserialize;

use super::TestResult;
use super::shader_accounting::require;
use super::shader_artifacts::Retained;
use super::shader_expected::{ExpectedCounts, ExpectedSource};

#[derive(Default)]
pub(super) struct Observed {
    pub(super) definitions: u64,
    pub(super) properties: u64,
    pub(super) failures: u64,
    pub(super) partial: u64,
    pub(super) complete: u64,
    nodes: BTreeSet<Vec<u64>>,
}

impl Observed {
    pub(super) fn record(
        &mut self,
        record: &MediaRecord,
        expected: &ExpectedSource,
        retained: &Retained,
    ) -> TestResult {
        check_source(record, expected, retained)?;
        match ShaderPayload::deserialize(&record.data)? {
            ShaderPayload::Definition(header) => {
                self.definition(record, &header, expected, retained)?;
            }
            ShaderPayload::Property(property) => {
                self.property(record, &property, expected, retained)?;
            }
            ShaderPayload::Declaration(declaration) => {
                require(
                    record.subresource.namespace == "shader-declaration",
                    "declaration namespace differs",
                )?;
                let counts = expected
                    .counts
                    .as_ref()
                    .ok_or("failed source has a declaration")?;
                self.node(record, counts, &declaration, &retained.bytes, false)?;
            }
            ShaderPayload::DefinitionFailure(failure) => {
                require(
                    record.subresource.namespace == "shader-definition-failure"
                        && record.subresource.indices.is_empty(),
                    "failed source identity differs",
                )?;
                require(
                    record.status == Status::Failed,
                    "source failure is not failed",
                )?;
                let span = expected
                    .failure
                    .ok_or("readable source was exported as failed")?;
                check_span(failure.span, span.start, span.end_exclusive)?;
                self.failures += 1;
            }
            _ => {
                return Err("unselected shader cache or unsupported row family was emitted".into());
            }
        }
        match record.status {
            Status::Complete => self.complete += 1,
            Status::Partial => self.partial += 1,
            Status::Failed => {}
        }
        Ok(())
    }

    fn definition(
        &mut self,
        record: &MediaRecord,
        header: &ShaderDefinition,
        expected: &ExpectedSource,
        retained: &Retained,
    ) -> TestResult {
        let counts = expected
            .counts
            .as_ref()
            .ok_or("failed source has a definition header")?;
        require(
            record.subresource.namespace == "shader-definition"
                && record.subresource.indices.is_empty(),
            "definition identity differs",
        )?;
        require(
            record.status == Status::Complete,
            "definition header is not complete",
        )?;
        require(
            header.counts.definitions == 1 && header.counts.compiled_caches == 0,
            "definition/cache units are mixed",
        )?;
        require(
            header.counts.properties == u64::try_from(counts.properties.len())?,
            "header Property count differs",
        )?;
        require(
            header.counts.declarations == u64::try_from(counts.nodes.len())?,
            "header declaration count differs",
        )?;
        require(
            header.counts.root_declarations == u64::try_from(counts.root_declarations)?,
            "header root count differs",
        )?;
        check_span(header.span, 0, retained.bytes.len())?;
        check_header_spelling(header, expected, &retained.bytes)?;
        self.definitions += 1;
        Ok(())
    }

    fn property(
        &mut self,
        record: &MediaRecord,
        property: &ShaderProperty,
        expected: &ExpectedSource,
        retained: &Retained,
    ) -> TestResult {
        require(
            record.subresource.namespace == "shader-property",
            "Property namespace differs",
        )?;
        require(
            record.status == Status::Complete,
            "Property declaration is not complete",
        )?;
        let counts = expected
            .counts
            .as_ref()
            .ok_or("failed source has a Property")?;
        let wanted = counts
            .properties
            .get(usize::try_from(property.ordinal)?)
            .ok_or("unexpected Property ordinal")?;
        require(
            property.ordinal == u64::try_from(wanted.ordinal)?,
            "Property ordinal differs",
        )?;
        require(
            record.subresource.indices == [u64::try_from(wanted.root_index)?],
            "Property source path differs",
        )?;
        check_span(
            property.declaration.span,
            wanted.span.start,
            wanted.span.end_exclusive,
        )?;
        self.node(record, counts, &property.declaration, &retained.bytes, true)?;
        self.properties += 1;
        Ok(())
    }

    fn node(
        &mut self,
        record: &MediaRecord,
        counts: &ExpectedCounts,
        declaration: &ShaderDeclaration,
        bytes: &[u8],
        property: bool,
    ) -> TestResult {
        let path = &record.subresource.indices;
        let wanted = counts
            .nodes
            .get(path)
            .ok_or("unknown source declaration ordinal path")?;
        require(
            wanted.property == property,
            "Property and declaration identities were conflated",
        )?;
        require(
            self.nodes.insert(path.clone()),
            "source declaration was emitted more than once",
        )?;
        require(
            declaration.child_count == u64::try_from(wanted.children)?,
            "source child count differs",
        )?;
        check_span(
            declaration.span,
            wanted.span.start,
            wanted.span.end_exclusive,
        )?;
        check_tokens(declaration, bytes)?;
        check_parent(counts, declaration, path)
    }

    pub(super) fn finish(&self, expected: &ExpectedSource) -> TestResult {
        match &expected.counts {
            Some(counts) => {
                require(
                    self.definitions == 1 && self.failures == 0,
                    "readable source header accounting differs",
                )?;
                require(
                    self.properties == u64::try_from(counts.properties.len())?,
                    "source Property row accounting differs",
                )?;
                require(
                    self.nodes.len() == counts.nodes.len(),
                    "source declaration row is missing",
                )
            }
            None => require(
                self.failures == 1 && self.definitions == 0 && self.nodes.is_empty(),
                "failed source accounting is not explicit",
            ),
        }
    }
}

fn check_parent(
    counts: &ExpectedCounts,
    declaration: &ShaderDeclaration,
    path: &[u64],
) -> TestResult {
    if path.len() == 1 {
        return require(
            declaration.parent.is_none(),
            "root declaration has a fabricated parent",
        );
    }
    let parent = declaration
        .parent
        .as_ref()
        .ok_or("nested declaration lacks parent")?;
    let parent_path = path.get(..path.len() - 1).ok_or("source parent path")?;
    require(
        parent.indices == parent_path,
        "source parent ordinal path differs",
    )?;
    let wanted = counts
        .nodes
        .get(parent_path)
        .ok_or("source parent missing")?;
    let namespace = if wanted.property {
        ShaderParentNamespace::ShaderProperty
    } else {
        ShaderParentNamespace::ShaderDeclaration
    };
    require(
        parent.namespace == namespace,
        "source parent namespace differs",
    )
}

fn check_tokens(declaration: &ShaderDeclaration, bytes: &[u8]) -> TestResult {
    let text = std::str::from_utf8(bytes)?;
    for token in [&declaration.name, &declaration.value, &declaration.literal]
        .into_iter()
        .flatten()
    {
        let start = usize::try_from(token.span.start)?;
        let end = usize::try_from(token.span.end_exclusive)?;
        require(
            token.span.basis == p4kx_media::ShaderOffsetBasis::Entry,
            "token offset basis differs",
        )?;
        require(
            text.get(start..end) == Some(token.text.as_str()),
            "token no longer matches retained source bytes",
        )?;
    }
    Ok(())
}

fn check_span(span: p4kx_media::ShaderSpan, start: usize, end: usize) -> TestResult {
    require(
        span.basis == p4kx_media::ShaderOffsetBasis::Entry,
        "source offset basis differs",
    )?;
    require(
        span.start == u64::try_from(start)? && span.end_exclusive == u64::try_from(end)?,
        "source half-open byte span differs",
    )
}

fn check_header_spelling(
    header: &ShaderDefinition,
    expected: &ExpectedSource,
    bytes: &[u8],
) -> TestResult {
    let p4kx_media::Identity::Entry { entry, .. } = &expected.identity else {
        return Err("shader definition lacks physical Entry identity".into());
    };
    let file = entry
        .path
        .rsplit(['/', '\\'])
        .next()
        .ok_or("source filename missing")?;
    let stem = file.rsplit_once('.').map_or(file, |(stem, _)| stem);
    require(
        header.file_name == file && header.shader_name == stem,
        "source shader filename spelling changed",
    )?;
    require(
        header.name_origin == p4kx_media::ShaderNameOrigin::FileStem,
        "shader name has an invented origin",
    )?;
    let encoding = if bytes.starts_with(b"\xef\xbb\xbf") {
        p4kx_media::ShaderEncoding::Utf8Bom
    } else {
        p4kx_media::ShaderEncoding::Utf8
    };
    require(
        header.encoding == encoding,
        "original shader encoding differs",
    )
}

fn check_source(
    record: &MediaRecord,
    expected: &ExpectedSource,
    retained: &Retained,
) -> TestResult {
    require(
        record.source.identity == retained.source.identity,
        "row source identity differs",
    )?;
    require(
        record.source.root == retained.source.root,
        "row source root differs",
    )?;
    require(
        record.source.raw == retained.source.raw,
        "row raw receipt differs",
    )?;
    require(
        record.source.verification == retained.source.verification,
        "row verification differs",
    )?;
    require(
        record.source.declared_integrity == retained.source.declared_integrity,
        "row declared integrity differs",
    )?;
    require(
        record.source.size == expected.size,
        "row source size differs",
    )?;
    Ok(())
}
