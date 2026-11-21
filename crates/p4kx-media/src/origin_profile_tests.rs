// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde::Serialize;
use serde_json::Value;

use crate::origin_read_support::{self as support, Fixture, TestResult};
use crate::origin_verify::OriginAvailability;
use crate::{
    ArchiveProvider, Capability, MediaError, MediaWriter, Projection, Provider, ProviderRegistry,
    SourceContext, Status, Subresource,
};

struct ImpersonatingProvider;

impl Provider for ImpersonatingProvider {
    fn capability(&self) -> Capability {
        ArchiveProvider.capability()
    }

    fn source(
        &self,
        source: &SourceContext<'_>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<Projection, MediaError> {
        let ordinal = u64::try_from(source.entry.index())
            .map_err(|_| MediaError::Contract("synthetic entry ordinal".into()))?;
        writer.record(
            &Subresource {
                namespace: "archive.entry".into(),
                indices: vec![ordinal],
            },
            Status::Complete,
            &CustomPayload {
                nodes: vec![CustomNode {
                    nested: CustomLeaf { nullable: None },
                }],
            },
            &[],
        )?;
        Ok(Projection::Written)
    }
}

#[derive(Serialize)]
struct CustomPayload {
    #[serde(rename = "")]
    nodes: Vec<CustomNode>,
}

#[derive(Serialize)]
struct CustomNode {
    #[serde(rename = "a/b")]
    nested: CustomLeaf,
}

#[derive(Serialize)]
struct CustomLeaf {
    #[serde(rename = "~")]
    nullable: Option<bool>,
}

#[test]
fn custom_provider_cannot_claim_builtin_profile_by_kind_capability_or_namespace() -> TestResult {
    let registry = ProviderRegistry::try_new(vec![Box::new(ImpersonatingProvider)])?;
    let fixture = Fixture::with_registry(&registry)?;
    let rows = fixture.rows()?;
    let profiles: Vec<_> = rows
        .iter()
        .filter(|row| support::row_type(row) == Some("occurrence") && support::is_record(row))
        .map(|row| row.get("profile").and_then(Value::as_str))
        .collect();
    assert_eq!(
        profiles,
        [Some("unmigrated"), Some("unmigrated")],
        "public record writes cannot obtain a sealed builtin mapping profile"
    );
    assert_eq!(
        fixture.verify(&fixture.index)?,
        OriginAvailability::Current,
        "current transport can honestly carry an unmigrated custom provider"
    );
    Ok(())
}

#[test]
fn actual_custom_payload_keeps_escaped_empty_and_list_node_addresses() -> TestResult {
    let registry = ProviderRegistry::try_new(vec![Box::new(ImpersonatingProvider)])?;
    let fixture = Fixture::with_registry(&registry)?;
    let rows = fixture.rows()?;
    for address in [
        "/data",
        "/data/",
        "/data//0",
        "/data//0/a~1b",
        "/data//0/a~1b/~0",
    ] {
        let fields: Vec<_> = rows
            .iter()
            .filter(|row| {
                support::row_type(row) == Some("field")
                    && support::is_record(row)
                    && row.get("field").and_then(Value::as_str) == Some(address)
            })
            .collect();
        assert_eq!(
            fields.len(),
            2,
            "every original row occurrence addresses {address}"
        );
        assert!(
            fields
                .iter()
                .all(|row| row.get("containsEncryptedSource") == Some(&Value::Null)),
            "unmigrated data node {address} remains unknown"
        );
    }
    assert_eq!(
        fixture.verify(&fixture.index)?,
        OriginAvailability::Current,
        "actual consumer coverage"
    );
    Ok(())
}
