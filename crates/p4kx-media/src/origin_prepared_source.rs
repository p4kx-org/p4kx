// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::FieldPath;

use crate::origin_contributions::OriginContributions;
use crate::origin_input::MediaOriginInput;
use crate::origin_prepared::PreparedRecordOrigins;
use crate::origin_resolver::OriginResolver;
use crate::origin_subject::OriginSubject;
use crate::{
    Identity, MediaError, MediaWriter, Source, origin_error, origin_source, origin_source_identity,
};

pub(crate) fn source(
    prepared: &mut PreparedRecordOrigins,
    prefix: &FieldPath,
    source: &Source,
    input: &MediaOriginInput,
    selector: &OriginContributions,
    writer: &MediaWriter<'_>,
) -> Result<(), MediaError> {
    let resolver = resolver(input, OriginSubject::Source(source), writer);
    prepared.insert(
        prefix,
        selected(&origin_source::whole(&resolver)?, selector, writer)?,
        writer,
    )?;
    for member in ["root", "size", "declared_integrity", "verification", "raw"] {
        let mut path = prefix.clone();
        path.push_field(member)
            .map_err(|cause| origin_error::address(&cause))?;
        let field = format!("/{member}");
        let contribution = origin_source::resolve(&resolver, &field)?.ok_or_else(|| {
            MediaError::Contract("original Source field has no owning origin mapping".into())
        })?;
        let contribution = if member == "root" {
            contribution
        } else {
            selected(&contribution, selector, writer)?
        };
        prepared.insert_subtree(&path, contribution, writer)?;
    }
    let mut path = prefix.clone();
    path.push_field("identity")
        .map_err(|cause| origin_error::address(&cause))?;
    identity(prepared, &path, &source.identity, input, selector, writer)
}

pub(crate) fn identity(
    prepared: &mut PreparedRecordOrigins,
    prefix: &FieldPath,
    identity: &Identity,
    input: &MediaOriginInput,
    selector: &OriginContributions,
    writer: &MediaWriter<'_>,
) -> Result<(), MediaError> {
    let resolver = resolver(input, OriginSubject::Artifact, writer);
    identity_field(prepared, prefix, "", identity, &resolver, selector, writer)?;
    identity_field(
        prepared, prefix, "type", identity, &resolver, selector, writer,
    )?;
    if let Identity::Entry { containers, .. } = identity {
        identity_field(
            prepared, prefix, "entry", identity, &resolver, selector, writer,
        )?;
        for member in ["entry_index", "path", "raw_path_hex"] {
            identity_field(
                prepared,
                prefix,
                &format!("entry/{member}"),
                identity,
                &resolver,
                selector,
                writer,
            )?;
        }
        identity_field(
            prepared,
            prefix,
            "containers",
            identity,
            &resolver,
            selector,
            writer,
        )?;
        for position in 0..containers.len() {
            let suffix = format!("containers/{position}");
            identity_field(
                prepared, prefix, &suffix, identity, &resolver, selector, writer,
            )?;
            for member in ["entry_index", "path", "raw_path_hex"] {
                identity_field(
                    prepared,
                    prefix,
                    &format!("{suffix}/{member}"),
                    identity,
                    &resolver,
                    selector,
                    writer,
                )?;
            }
        }
    }
    Ok(())
}

fn identity_field(
    prepared: &mut PreparedRecordOrigins,
    prefix: &FieldPath,
    suffix: &str,
    identity: &Identity,
    resolver: &OriginResolver<'_>,
    selector: &OriginContributions,
    writer: &MediaWriter<'_>,
) -> Result<(), MediaError> {
    let mut path = prefix.clone();
    for member in suffix.split('/').filter(|member| !member.is_empty()) {
        path.push_field(member)
            .map_err(|cause| origin_error::address(&cause))?;
    }
    let field = if suffix.is_empty() {
        "/identity".to_owned()
    } else {
        format!("/identity/{suffix}")
    };
    let contribution =
        origin_source_identity::resolve(resolver, identity, &field)?.ok_or_else(|| {
            MediaError::Contract("original Identity field has no owning origin mapping".into())
        })?;
    prepared.insert_subtree(&path, selected(&contribution, selector, writer)?, writer)
}

fn selected(
    value: &OriginContributions,
    selector: &OriginContributions,
    writer: &MediaWriter<'_>,
) -> Result<OriginContributions, MediaError> {
    let mut capture = writer.capture_origins();
    capture.include(value, writer)?;
    capture.include(selector, writer)?;
    Ok(capture.finish())
}

const fn resolver<'a>(
    input: &'a MediaOriginInput,
    subject: OriginSubject<'a>,
    writer: &'a MediaWriter<'_>,
) -> OriginResolver<'a> {
    OriginResolver {
        owner: &writer.origins.owner,
        root: writer.origins.root.as_ref(),
        source: Some(input),
        source_prefix: "",
        subject,
        artifact: None,
        builtin: None,
        subresource: None,
        detail_bytes: writer.limits().record_bytes,
    }
}
