// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{InputScope, SourceUseRole, SourceUses};

use crate::origin_aggregate;
use crate::origin_capture::{OriginCapture, OriginSite};
use crate::origin_contract::OriginProfile;
use crate::origin_contributions::OriginContributions;
use crate::origin_input::MediaOriginInput;
use crate::origin_prepared::PreparedRecordOrigins;
use crate::origin_subject::OriginSubject;
use crate::{MediaError, Subresource, origin_data, origin_source};

/// A sealed built-in mapping selected only by the owning producer.
#[derive(Debug)]
pub(crate) struct BuiltinOrigins {
    pub(crate) profile: OriginProfile,
    pub(crate) input: MediaOriginInput,
    pub(crate) scope: InputScope,
}

/// Original input context for one payload occurrence.
pub(crate) struct OriginResolver<'a> {
    pub(crate) owner: &'a SourceUses,
    pub(crate) root: Option<&'a MediaOriginInput>,
    pub(crate) source: Option<&'a MediaOriginInput>,
    pub(crate) source_prefix: &'a str,
    pub(crate) subject: OriginSubject<'a>,
    pub(crate) artifact: Option<(&'a MediaOriginInput, InputScope)>,
    pub(crate) builtin: Option<&'a BuiltinOrigins>,
    pub(crate) subresource: Option<&'a Subresource>,
    pub(crate) detail_bytes: u64,
}

impl OriginResolver<'_> {
    pub(crate) fn resolve(
        &self,
        pointer: &str,
        prepared: Option<&PreparedRecordOrigins>,
    ) -> Result<OriginContributions, MediaError> {
        if let Some(aggregate) = origin_aggregate::resolve(self, pointer, prepared)? {
            if let Some(prepared) = prepared {
                let _actual = prepared.visit(pointer);
            }
            return Ok(aggregate);
        }
        self.direct(pointer, prepared)
    }

    pub(crate) fn direct(
        &self,
        pointer: &str,
        prepared: Option<&PreparedRecordOrigins>,
    ) -> Result<OriginContributions, MediaError> {
        if let Some(actual) = prepared.and_then(|prepared| prepared.visit(pointer)) {
            return Ok(actual.clone());
        }
        if let Some((input, scope)) = self.artifact
            && ["/path", "/size", "/digest"]
                .iter()
                .any(|prefix| within(pointer, prefix))
        {
            return self.capture_input(input, scope, pointer, SourceUseRole::Conversion);
        }
        if let Some(actual) = origin_source::resolve(self, pointer)? {
            return Ok(actual);
        }
        if let Some(builtin) = self.builtin
            && origin_data::independent(builtin.profile, pointer)
        {
            return Ok(OriginContributions::independent(self.owner));
        }
        if let Some(builtin) = self.builtin {
            let scope = match origin_data::scope(builtin.profile, pointer) {
                origin_data::ScopeMapping::Unmapped => None,
                origin_data::ScopeMapping::BuiltinScope => Some(builtin.scope),
                origin_data::ScopeMapping::SpecificScope(scope) => Some(scope),
            };
            if let Some(scope) = scope {
                return self.capture_input(
                    &builtin.input,
                    scope,
                    pointer,
                    origin_data::role(builtin.profile, pointer),
                );
            }
        }
        if let Some(builtin) = self.builtin
            && within(pointer, "/subresource")
            && pointer != "/subresource/namespace"
        {
            return self.capture_input(
                &builtin.input,
                builtin.scope,
                pointer,
                SourceUseRole::Conversion,
            );
        }
        if self.builtin.is_some() && is_record_control(pointer) {
            return Ok(OriginContributions::independent(self.owner));
        }
        Ok(OriginContributions::unknown(self.owner))
    }

    pub(crate) fn capture_input(
        &self,
        input: &MediaOriginInput,
        scope: InputScope,
        field: &str,
        role: SourceUseRole,
    ) -> Result<OriginContributions, MediaError> {
        let empty = Subresource::new("media-source", Vec::new());
        let site = self.subresource.unwrap_or(&empty);
        let mut capture = OriginCapture::with_limit(self.owner, self.detail_bytes);
        capture.add_original(
            input,
            scope,
            OriginSite::new(&site.namespace, &site.indices, input_field(field)),
            role,
        )?;
        Ok(capture.finish())
    }
}

fn is_record_control(pointer: &str) -> bool {
    ["/kind", "/subresource/namespace", "/status"]
        .iter()
        .any(|prefix| within(pointer, prefix))
}

pub(crate) fn within(pointer: &str, prefix: &str) -> bool {
    pointer == prefix
        || pointer
            .strip_prefix(prefix)
            .is_some_and(|rest| rest.starts_with('/'))
}
fn input_field(pointer: &str) -> &str {
    pointer
        .strip_prefix("/data/payload")
        .or_else(|| pointer.strip_prefix("/data"))
        .or_else(|| pointer.strip_prefix("/source"))
        .unwrap_or(pointer)
}
