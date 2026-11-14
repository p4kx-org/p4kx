// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{ArchiveFiles, LoadoutFileError, LoadoutFileRead, LoadoutFiles};
use p4kx_extraction::{Archive, SourceEvidenceError};

use crate::catalog_read::original::{
    AuthoredEntryPath, EntryCapture, EntryDiagnosis, EntryRead, EntryXml,
};
use crate::entry_source::EntrySource;
use crate::export_diagnostic::ExportDiagnosticCode;
use crate::export_entry_ledger::EntryOperation;
use crate::export_ledger::{ExportLedger, SourceDisposition};
use crate::shop_summary::ShopSummary;
use crate::shops::REGISTRY;
use crate::starmap_xml::{Node, attribute};
use crate::{CatalogError, Kind};

/// One `ShopBackup` entry of the shop registry, as it is stored.
#[derive(Debug)]
pub(crate) struct RegistryShop {
    pub(crate) platform: String,
    pub(crate) id: RegistryId,
    pub(crate) super_name: String,
    pub(crate) default_value: String,
    pub(crate) shop_name: String,
}

#[derive(Debug)]
pub(crate) enum RegistryId {
    Authored(AuthoredEntryPath),
    Absent,
    #[cfg(test)]
    Fixture(String),
}

impl RegistryId {
    pub(crate) fn text(&self) -> &str {
        match self {
            Self::Authored(value) => value.as_str(),
            Self::Absent => "",
            #[cfg(test)]
            Self::Fixture(value) => value,
        }
    }

    pub(crate) const fn authored(&self) -> Option<&AuthoredEntryPath> {
        if let Self::Authored(value) = self {
            Some(value)
        } else {
            None
        }
    }
}

/// Whether the consuming catalog requires a registry to describe its starmap.
#[derive(Clone, Copy)]
pub(crate) enum RegistryNeed {
    Optional,
    Required,
}

/// Reads the registry once and retains its Entry identity until row projection finishes.
pub(crate) fn read_source(
    archive: &Archive,
    files: &ArchiveFiles<'_>,
    need: RegistryNeed,
    summary: &mut ShopSummary,
    ledger: &mut ExportLedger,
    capture: &EntryCapture<'_>,
) -> Result<Option<(Vec<RegistryShop>, EntrySource)>, CatalogError> {
    capture.check_archive(archive.input_origin())?;
    let observed = files.read_observed(REGISTRY);
    match observed.outcome() {
        Ok(_) => project_registry(observed, capture, summary, ledger),
        Err(
            error @ (LoadoutFileError::ObservationStopped
            | LoadoutFileError::SourceEvidenceUnavailable),
        ) => Err(CatalogError::LoadoutSourceEvidence(error.clone())),
        Err(error) => {
            let source =
                observed_source(capture, &observed)?.operation(EntryOperation::ShopRegistry);
            if matches!(error, LoadoutFileError::NotFound) {
                summary.registry_missing = true;
                missing(source, need, ledger);
            } else {
                read_failure(source, error, summary, ledger)?;
            }
            Ok(None)
        }
    }
}

fn project_registry(
    observed: LoadoutFileRead,
    capture: &EntryCapture<'_>,
    summary: &mut ShopSummary,
    ledger: &mut ExportLedger,
) -> Result<Option<(Vec<RegistryShop>, EntrySource)>, CatalogError> {
    let read = EntryRead::from_observed(capture, observed)?;
    let source = EntrySource::from_read(&read, EntryOperation::ShopRegistry);
    if let Ok(xml) = EntryXml::parse(&read, capture)? {
        return Ok(Some((collect(&xml)?, source)));
    }
    summary.registry_unreadable = true;
    ledger.incomplete_inventory(Kind::Shops);
    source.diagnosed(
        ledger,
        Kind::Shops,
        SourceDisposition::ProjectionFailed,
        EntryDiagnosis::technical(
            ExportDiagnosticCode::MalformedSource,
            "ShopBackup",
            "ShopSource",
        ),
    );
    Ok(None)
}

fn observed_source(
    capture: &EntryCapture<'_>,
    read: &LoadoutFileRead,
) -> Result<EntrySource, CatalogError> {
    let source = read.source();
    let original = source
        .archive_input_origin()
        .ok_or(SourceEvidenceError::ForeignEvidence)?;
    capture.check_archive(original)?;
    match source.entry_origin() {
        Some(_) => EntrySource::loadout(capture, read),
        None if source.is_missing_archive()
            && matches!(read.outcome(), Err(LoadoutFileError::NotFound)) =>
        {
            EntrySource::fixed_missing(capture, REGISTRY)
        }
        None => Err(SourceEvidenceError::ForeignEvidence.into()),
    }
}

fn read_failure(
    source: EntrySource,
    error: &LoadoutFileError,
    summary: &mut ShopSummary,
    ledger: &mut ExportLedger,
) -> Result<(), CatalogError> {
    if matches!(error, LoadoutFileError::Unreadable { .. }) {
        source.receipt().attempted_payload()?;
    }
    summary.registry_unreadable = true;
    ledger.incomplete_inventory(Kind::Shops);
    let code = if matches!(error, LoadoutFileError::TooLarge { .. }) {
        ExportDiagnosticCode::FieldWalkLimit
    } else {
        ExportDiagnosticCode::SourceReadFailure
    };
    source.diagnosed(
        ledger,
        Kind::Shops,
        SourceDisposition::ReadFailed,
        EntryDiagnosis::technical(code, "ShopBackup", "ShopSource"),
    );
    Ok(())
}

fn missing(source: EntrySource, need: RegistryNeed, ledger: &mut ExportLedger) {
    if matches!(need, RegistryNeed::Required) {
        ledger.incomplete_inventory(Kind::Shops);
    }
    let (disposition, code) = match need {
        RegistryNeed::Required => (
            SourceDisposition::ReadFailed,
            ExportDiagnosticCode::DanglingReference,
        ),
        RegistryNeed::Optional => (
            SourceDisposition::SkippedBeforeRead,
            ExportDiagnosticCode::OptionalAbsent,
        ),
    };
    source.diagnosed(
        ledger,
        Kind::Shops,
        disposition,
        EntryDiagnosis::technical(code, "ShopBackup", "ShopSource"),
    );
}

fn named<'d, N: Node<'d>>(node: N, name: &'static str) -> impl Iterator<Item = N> {
    node.children()
        .filter(move |child| child.name().eq_ignore_ascii_case(name))
}

fn collect(xml: &EntryXml<'_, '_, '_>) -> Result<Vec<RegistryShop>, CatalogError> {
    let mut shops = Vec::new();
    for backups in named(xml.root(), "LayerBackups") {
        for layer in named(backups, "LayerBackup") {
            let platform = attribute(layer, "PlatformName").unwrap_or_default();
            for list in named(layer, "ShopBackups") {
                for shop in named(list, "ShopBackup") {
                    let text = |name| attribute(shop, name).unwrap_or_default().to_owned();
                    shops.push(RegistryShop {
                        platform: platform.to_owned(),
                        id: xml
                            .attribute(shop, "ID")?
                            .map_or(RegistryId::Absent, RegistryId::Authored),
                        super_name: text("SuperName"),
                        default_value: text("DefaultValue"),
                        shop_name: text("ShopName"),
                    });
                }
            }
        }
    }
    Ok(shops)
}

/// Returns whether `text` is a GUID in the 8-4-4-4-12 hexadecimal form.
pub(crate) fn is_guid(text: &str) -> bool {
    text.len() == 36
        && text.bytes().enumerate().all(|(at, byte)| {
            if matches!(at, 8 | 13 | 18 | 23) {
                byte == b'-'
            } else {
                byte.is_ascii_hexdigit()
            }
        })
}

/// Returns `name` without a trailing `_{GUID}` (or `_{GUID}_socpak`) and without surrounding
/// blanks; a braced tail that is no GUID stays.
pub(crate) fn clean(name: &str) -> &str {
    let name = name.trim();
    let base = name.strip_suffix("_socpak").unwrap_or(name);
    match base
        .strip_suffix('}')
        .and_then(|rest| rest.rsplit_once("_{"))
    {
        Some((head, guid)) if is_guid(guid) => head.trim_end(),
        _ => name,
    }
}

#[cfg(test)]
mod tests {
    use super::clean;

    #[test]
    fn a_container_name_loses_its_guid_and_its_blanks() {
        assert_eq!(
            clean("OOC_Stanton_4_Microtech_{B4795269-F681-4CE1-AFCC-FD027F3D3605}"),
            "OOC_Stanton_4_Microtech"
        );
        assert_eq!(
            clean("Drlct_Stlmnt_008_Megaton_RastarLE "),
            "Drlct_Stlmnt_008_Megaton_RastarLE"
        );
        assert_eq!(
            clean("SCShop_Bar_Drlct_Stanton"),
            "SCShop_Bar_Drlct_Stanton"
        );
        assert_eq!(clean("odd_{not a guid"), "odd_{not a guid");
        assert_eq!(clean("keeps_{braced}"), "keeps_{braced}");
        assert_eq!(
            clean("pbay_cluster_001_sand_{04E2D380-091E-449D-8B48-08CB81FBCC08}_socpak"),
            "pbay_cluster_001_sand"
        );
        assert_eq!(clean("plain_socpak"), "plain_socpak");
    }
}
