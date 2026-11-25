// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;
use std::ops::Range;

use p4kx_audio::EmbeddedWemSource;

use crate::bank_wem_evidence;
use crate::sound::WemOriginSite;
use crate::sound::wem_origin_plan::wem_location;
use crate::{
    FieldOrigin, InputScope, SharedFieldOrigin, SourceInput, SourceLocation, SourceScope,
    SourceUse, SourceUseRole, SourceUses,
};

#[derive(Clone, Copy)]
pub(crate) struct SoundOriginInput<'a> {
    source: SourceInput<'a>,
    member: Option<EmbeddedWemSource>,
}

impl<'a> SoundOriginInput<'a> {
    pub(crate) const fn loose(source: SourceInput<'a>) -> Self {
        Self {
            source,
            member: None,
        }
    }

    pub(crate) const fn source(self) -> SourceInput<'a> {
        self.source
    }

    pub(crate) const fn embedded(source: SourceInput<'a>, member: EmbeddedWemSource) -> Self {
        Self {
            source,
            member: Some(member),
        }
    }
}

pub(in crate::sound) struct SoundPayload {
    scope: SourceScope,
    name_scope: Option<InputScope>,
    member: Option<EmbeddedWemSource>,
    selection: Option<SharedFieldOrigin>,
}

impl SoundPayload {
    pub(super) fn observe(input: SoundOriginInput<'_>, uses: &SourceUses) -> io::Result<Self> {
        let scope = uses
            .observe(input.source, InputScope::Payload)
            .map_err(io::Error::other)?;
        let name_scope = match input.source {
            SourceInput::Archive(_) | SourceInput::ArchiveView(_) => {
                Some(InputScope::ArchiveDirectoryMetadata)
            }
            SourceInput::File(_) => Some(InputScope::FileMetadata),
            _ => None,
        };
        let selection = input
            .member
            .map(|member| {
                bank_wem_evidence::member_selectors(&scope, member)
                    .map(|selectors| uses.shared_origin(&selectors, true))
            })
            .transpose()
            .map_err(io::Error::other)?;
        Ok(Self {
            scope,
            name_scope,
            member: input.member,
            selection,
        })
    }

    pub(super) fn origin(
        &self,
        uses: &SourceUses,
        fields: &[SourceUse],
    ) -> io::Result<FieldOrigin> {
        self.selection.as_ref().map_or_else(
            || Ok(uses.origin(fields, true)),
            |selection| {
                uses.origin_with_shared(selection, fields, true)
                    .map_err(io::Error::other)
            },
        )
    }

    pub(super) fn raw_member_origin(
        &self,
        uses: &SourceUses,
        length: u64,
    ) -> io::Result<FieldOrigin> {
        let member = self
            .member
            .ok_or_else(|| io::Error::other("the raw audio payload has no selected bank member"))?;
        let range = member.byte_range();
        if range.end - range.start != length {
            return Err(io::Error::other(
                "the copied bytes differ in length from the selected bank member",
            ));
        }
        let payload =
            bank_wem_evidence::member_payload(&self.scope, member).map_err(io::Error::other)?;
        self.origin(uses, &[payload])
    }

    pub(super) fn name_origin(&self, uses: &SourceUses) -> io::Result<FieldOrigin> {
        let Some(scope) = self.name_scope else {
            return Ok(uses.origin(&[], false));
        };
        let original = self
            .scope
            .for_scope(scope)
            .at(
                SourceLocation::Metadata {
                    field: "path".to_owned(),
                },
                SourceUseRole::Payload,
            )
            .map_err(io::Error::other)?;
        self.origin(uses, &[original])
    }

    pub(super) fn planned(&self, site: &WemOriginSite) -> io::Result<SourceUse> {
        self.member
            .map_or_else(
                || site.bind(&self.scope),
                |member| site.bind_embedded(&self.scope, member.byte_range()),
            )
            .map_err(io::Error::other)
    }

    pub(in crate::sound) fn at(
        &self,
        namespace: &str,
        indices: &[u64],
        field: &str,
        role: SourceUseRole,
    ) -> io::Result<SourceUse> {
        let member = self.member.map(|member| member.byte_range());
        self.scope
            .at(wem_location(namespace, indices, field, member), role)
            .map_err(io::Error::other)
    }

    pub(in crate::sound) fn range(
        &self,
        namespace: &str,
        field: &str,
        range: &Range<u64>,
        role: SourceUseRole,
    ) -> io::Result<SourceUse> {
        if let Some(member) = self.member {
            let extent = member.byte_range();
            if range.end > extent.end - extent.start {
                return Err(io::Error::other(
                    "WEM source range exceeds the selected bank member",
                ));
            }
        }
        self.at(namespace, &[range.start, range.end], field, role)
    }
}
