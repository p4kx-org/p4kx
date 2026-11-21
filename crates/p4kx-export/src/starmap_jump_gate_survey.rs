// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourceValue;

use super::{COMPONENT, GateFiles, META_PATH, marker};
use crate::CatalogError;
use crate::catalog_read::original::{EntryXml, PhysicalEntryRef};
use crate::export_ledger::ExportLedger;
use crate::starmap_xml::Node;

pub(crate) struct GateSurvey {
    pub(crate) marker: SourceValue<bool>,
    pub(crate) metadata: SourceValue<PhysicalEntryRef>,
    pub(crate) editor: SourceValue<PhysicalEntryRef>,
}

impl GateFiles<'_, '_> {
    pub(crate) fn survey(
        &self,
        main: &EntryXml<'_, '_, '_>,
        ledger: &mut ExportLedger,
        mut inspect: impl FnMut(&EntryXml<'_, '_, '_>) -> Result<(), CatalogError>,
    ) -> Result<GateSurvey, CatalogError> {
        let (marker, _) = marker(main)?;
        if !matches!(marker, SourceValue::Present(true)) {
            return Ok(GateSurvey {
                marker,
                metadata: SourceValue::Absent,
                editor: SourceValue::Absent,
            });
        }
        inspect(main)?;
        let (nested_meta, editor_path) = self.member_paths()?;
        let (metadata, _) = self.member(&[META_PATH, &nested_meta], ledger, |xml| {
            inspect_member(xml, COMPONENT, &mut inspect)
        })?;
        let (editor, _) = self.member(&[&editor_path], ledger, |xml| {
            inspect_member(xml, "ObjectContainer", &mut inspect)
        })?;
        Ok(GateSurvey {
            marker,
            metadata: metadata.state,
            editor: editor.state,
        })
    }
}

fn inspect_member(
    xml: &EntryXml<'_, '_, '_>,
    root: &str,
    inspect: &mut impl FnMut(&EntryXml<'_, '_, '_>) -> Result<(), CatalogError>,
) -> Result<Option<()>, CatalogError> {
    if xml.root().name() != root {
        return Ok(None);
    }
    inspect(xml)?;
    Ok(Some(()))
}
