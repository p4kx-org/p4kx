// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{ArchiveFiles, LoadoutFileError, LoadoutFiles};
use p4kx_extraction::Archive;
use p4kx_extraction::cryxml::{self, Xml};

use crate::Kind;
use crate::entry_source::EntrySource;
use crate::export_diagnostic::ExportDiagnosticCode;
use crate::export_entry_ledger::EntryOperation;
use crate::export_ledger::{ExportLedger, SourceDisposition};
use crate::shop_entry_read;
use crate::shop_summary::ShopSummary;
use crate::shops::REGISTRY;
use crate::starmap_xml::{Node, attribute};

/// One `ShopBackup` entry of the shop registry, as it is stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RegistryShop {
    pub(crate) platform: String,
    pub(crate) id: String,
    pub(crate) super_name: String,
    pub(crate) default_value: String,
    pub(crate) shop_name: String,
}

/// Whether the consuming catalog requires a registry to describe its starmap.
#[derive(Clone, Copy)]
pub(crate) enum RegistryNeed {
    Optional,
    Required,
}

/// Reads the `ShopBackup` entries of the registry `bytes`, text XML or `CryXml`, in document order;
/// `None` when the bytes are neither.
pub(crate) fn read(bytes: &[u8]) -> Option<Vec<RegistryShop>> {
    match cryxml::read(bytes).ok()? {
        Xml::CryXml(document) => Some(collect(document.root())),
        Xml::Text(text) => {
            let text = std::str::from_utf8(text)
                .ok()?
                .trim_start_matches('\u{feff}');
            cryxml::check_text_depth(text).ok()?;
            let document = roxmltree::Document::parse(text).ok()?;
            Some(collect(document.root_element()))
        }
    }
}

/// Reads the registry once and retains its Entry identity until row projection finishes.
pub(crate) fn read_source(
    archive: &Archive,
    files: &ArchiveFiles<'_>,
    need: RegistryNeed,
    summary: &mut ShopSummary,
    ledger: &mut ExportLedger,
) -> Option<(Vec<RegistryShop>, EntrySource)> {
    let wanted = crate::starmap_index::key(REGISTRY);
    let source = archive
        .entries()
        .iter()
        .rev()
        .find(|entry| crate::starmap_index::key(entry.path()) == wanted)
        .map_or_else(|| EntrySource::missing(REGISTRY), EntrySource::outer)
        .operation(EntryOperation::ShopRegistry);
    match files.read(REGISTRY) {
        Ok(bytes) => {
            if let Some(parsed) = read(&bytes) {
                Some((parsed, source))
            } else {
                summary.registry_unreadable = true;
                ledger.incomplete_inventory(Kind::Shops);
                source.diagnosed(
                    ledger,
                    Kind::Shops,
                    SourceDisposition::ProjectionFailed,
                    shop_entry_read::diagnosis(ExportDiagnosticCode::MalformedSource, "ShopBackup"),
                );
                None
            }
        }
        Err(LoadoutFileError::NotFound) => {
            summary.registry_missing = true;
            missing(source, need, ledger);
            None
        }
        Err(error) => {
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
                shop_entry_read::diagnosis(code, "ShopBackup"),
            );
            None
        }
    }
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
        shop_entry_read::diagnosis(code, "ShopBackup"),
    );
}

fn named<'d, N: Node<'d>>(node: N, name: &'static str) -> impl Iterator<Item = N> {
    node.children()
        .filter(move |child| child.name().eq_ignore_ascii_case(name))
}

fn collect<'d, N: Node<'d>>(root: N) -> Vec<RegistryShop> {
    let mut shops = Vec::new();
    for backups in named(root, "LayerBackups") {
        for layer in named(backups, "LayerBackup") {
            let platform = attribute(layer, "PlatformName").unwrap_or_default();
            for list in named(layer, "ShopBackups") {
                for shop in named(list, "ShopBackup") {
                    let text = |name| attribute(shop, name).unwrap_or_default().to_owned();
                    shops.push(RegistryShop {
                        platform: platform.to_owned(),
                        id: text("ID"),
                        super_name: text("SuperName"),
                        default_value: text("DefaultValue"),
                        shop_name: text("ShopName"),
                    });
                }
            }
        }
    }
    shops
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
