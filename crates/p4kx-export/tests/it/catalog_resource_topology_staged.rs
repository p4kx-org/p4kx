// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{
    CatalogOptions, CatalogPublicationMode, Compression, build_catalog, write_catalog_from,
};
use p4kx_extraction::{DataForgeEntry, ProvenanceMode};
use serde_json::Value;

use super::{TestResult, common, fixture, proof};

#[test]
fn staged_topology_links_replay_in_both_catalog_profiles() -> TestResult {
    let (_directory, archive) = fixture::graph_archive()?;
    let entry = DataForgeEntry::read(&archive)?;
    let options = CatalogOptions {
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..common::options()
    };
    for mode in [ProvenanceMode::MarkersOnly, ProvenanceMode::Full] {
        let options = options.with_provenance_mode(mode);
        let plain = build_catalog(&archive, options)?;
        let (summary, compressed) = write_catalog_from(
            &archive,
            &entry,
            options,
            Compression::Zstd { level: 3 },
            || Ok(Vec::new()),
        )?;
        let decoded = zstd::stream::decode_all(compressed.as_slice())?;
        let catalog: Value = serde_json::from_slice(&decoded)?;
        let body = catalog.get("catalog").unwrap_or(&catalog);
        let rows = proof::rows(body, "/placements/resourceNetworkLinkBindings/rows")?;
        assert!(!rows.is_empty(), "the topology suffix has rows to replay");
        assert_eq!(decoded, plain.json(), "both writers emit the same JSON");
        assert_eq!(summary.size(), u64::try_from(decoded.len())?);
    }
    Ok(())
}
