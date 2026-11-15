// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{HashMap, HashSet};
use std::io::Read;

use p4kx_extraction::cryxml::{
    SourceAttribute, SourceAttributePosition, SourceElement, SourceElementPosition,
};
use p4kx_extraction::{
    Archive, DataForgeEntry, DataForgeOrigin, Entry, InputScope, RecordTree, SourceEncryption,
    SourceInput, SourceUses, StructValue,
};
use serde_json::{Value, json};

use crate::real_archive_item_durability::{self as durability, indexed, number, rows, text};

use super::{TestResult, unique};

pub(super) struct Inventory<'catalog> {
    archive: &'catalog Archive,
    sources: HashMap<&'catalog str, &'catalog Value>,
    uses: HashMap<&'catalog str, &'catalog Value>,
}

#[derive(Clone, Copy)]
pub(super) struct XmlProof<'catalog, 'inventory> {
    inventory: &'inventory Inventory<'catalog>,
    pub(super) source: &'catalog str,
}

impl<'catalog> Inventory<'catalog> {
    pub(super) fn read(
        provenance: &'catalog Value,
        archive: &'catalog Archive,
    ) -> TestResult<Self> {
        let sources = indexed(provenance, "/sources", "id")?;
        let uses = indexed(provenance, "/uses", "id")?;
        assert!(
            sources.len() == rows(provenance, "/sources")?.len()
                && uses.len() == rows(provenance, "/uses")?.len(),
            "provenance inventory identifiers are unique",
        );
        Ok(Self {
            archive,
            sources,
            uses,
        })
    }

    pub(super) fn verify_entry(&self, source: &str, entry: &Entry) -> TestResult {
        let row = self.sources.get(source).ok_or("physical proof source")?;
        let index = u64::try_from(entry.index())?;
        assert!(
            entry_index(row)? == index,
            "proof identifies the exact original archive Entry",
        );
        Ok(())
    }

    pub(super) fn xml<'inventory>(
        &'inventory self,
        file: &'catalog Value,
        entry: &Entry,
    ) -> TestResult<XmlProof<'catalog, 'inventory>> {
        let source = text(file, "/source/inputRef")?;
        self.verify_entry(source, entry)?;
        assert!(
            number(file, "/source/entryIndex")? == u64::try_from(entry.index())?,
            "document carries the physical Entry index",
        );
        assert!(
            text(file, "/source/sourcePath/value")? == entry.path(),
            "document carries the original admitted archive path",
        );
        Ok(XmlProof {
            inventory: self,
            source,
        })
    }

    pub(super) fn dcb_fields<'source>(
        &self,
        dcb: &DataForgeEntry,
        tree: &RecordTree<'source>,
        original: &StructValue<'source>,
        projected: &Value,
        fields: &[(&str, &str)],
    ) -> TestResult {
        let DataForgeOrigin::Archive(origin) = dcb.origin() else {
            return Err("original archive DataForge receipt".into());
        };
        let sources = SourceUses::new();
        let scope = sources.observe(SourceInput::Archive(origin), InputScope::Payload)?;
        let trace = durability::Trace::of(tree, &self.uses, &scope, marker(origin.encryption()));
        for (field, key) in fields {
            let cell = projected.get(key).ok_or("sourced DataForge leaf")?;
            let value = original.field(field).ok_or("original DataForge leaf")?;
            trace.check(cell, value, "payload")?;
            self.closure(cell)?;
        }
        Ok(())
    }

    pub(super) fn declaration(
        &self,
        source: &str,
        declaration: &super::scopes::Declaration,
        cell: &Value,
    ) -> TestResult {
        let entry = self
            .archive
            .entries()
            .get(declaration.entry)
            .ok_or("actual TagDef Entry")?;
        self.verify_entry(source, entry)?;
        XmlProof {
            inventory: self,
            source,
        }
        .leaf(cell, &declaration.position, "selector")
    }

    pub(super) fn closure(&self, cell: &Value) -> TestResult {
        let refs = rows(cell, "/sourceUsesRefs")?;
        let mut seen = HashSet::new();
        let complete = cell["sourceSetComplete"]
            .as_bool()
            .ok_or("source set completeness")?;
        let mut encrypted = false;
        let mut unknown = !complete;
        for id in refs {
            let id = id.as_str().ok_or("source-use identifier")?;
            assert!(seen.insert(id), "source-use references are distinct");
            let used = self.uses.get(id).ok_or("source-use inventory closure")?;
            let source = self
                .sources
                .get(text(used, "/source")?)
                .ok_or("physical source closure")?;
            let effective = self.encryption(used, source)?;
            encrypted |= effective == Some(true);
            unknown |= effective.is_none();
        }
        let expected = if encrypted {
            Some(true)
        } else if unknown {
            None
        } else {
            Some(false)
        };
        assert!(
            cell.get("containsEncryptedSource") == Some(&json!(expected)),
            "source marker agrees with original source contributions",
        );
        Ok(())
    }

    fn encryption(&self, used: &Value, source: &Value) -> TestResult<Option<bool>> {
        let role = text(used, "/role")?;
        assert!(
            matches!(role, "payload" | "selector" | "conversion"),
            "only contributors enter the field proof",
        );
        let expected = match (text(used, "/scope")?, text(source, "/kind")?) {
            ("archiveDirectoryMetadata", _) => Some(false),
            ("payload", "archiveEntry" | "archiveEntryProof") => {
                let index = usize::try_from(entry_index(source)?)?;
                let entry = self
                    .archive
                    .entries()
                    .get(index)
                    .ok_or("original source entry")?;
                let observed = self
                    .archive
                    .observed_entry(entry)
                    .ok_or("actual archive entry")?;
                marker(observed.stored_encryption())
            }
            _ => None,
        };
        let label = match expected {
            Some(true) => "encrypted",
            Some(false) => "unencrypted",
            None => "unknown",
        };
        assert!(
            text(used, "/effectiveEncryption")? == label,
            "source-use encryption matches the original byte domain",
        );
        Ok(expected)
    }
}

impl XmlProof<'_, '_> {
    pub(super) fn position(self, cell: &Value, original: SourceElement<'_>) -> TestResult {
        let expected = node_location(original, "$node")?;
        assert!(
            cell.get("value") == Some(&expected),
            "original XML node coordinate"
        );
        self.leaf(cell, &expected, "selector")
    }

    pub(super) fn name(self, cell: &Value, original: SourceElement<'_>) -> TestResult {
        assert!(
            text(cell, "/value")? == original.name(),
            "original XML element name"
        );
        self.leaf(cell, &node_location(original, "$name")?, "payload")
    }

    pub(super) fn attributes(self, row: &Value, original: SourceElement<'_>) -> TestResult {
        let observed = rows(row, "/attributes")?;
        let mut expected = original.attributes();
        for projected in observed {
            let attr = expected.next().ok_or("extra XML attribute row")?;
            self.attribute(projected, &attr)?;
        }
        assert!(
            expected.next().is_none(),
            "every original XML attribute is retained"
        );
        Ok(())
    }

    fn attribute(self, row: &Value, original: &SourceAttribute<'_>) -> TestResult {
        let location = attribute_location(original)?;
        assert!(
            text(row, "/name/value")? == original.name()
                && text(row, "/value/value")? == original.value()
                && row.pointer("/namespace/value") == Some(&json!(original.namespace())),
            "original attribute name, namespace and lexical value",
        );
        self.leaf(&row["name"], &location, "selector")?;
        self.leaf(&row["namespace"], &location, "selector")?;
        self.leaf(&row["value"], &location, "payload")
    }

    fn leaf(self, cell: &Value, location: &Value, role: &str) -> TestResult {
        let [reference] = rows(cell, "/sourceUsesRefs")? else {
            return Err("each exact XML leaf has one contributor".into());
        };
        let used = self
            .inventory
            .uses
            .get(reference.as_str().ok_or("XML use ID")?)
            .ok_or("XML leaf was admitted into source inventory")?;
        assert!(
            text(used, "/source")? == self.source
                && text(used, "/scope")? == "payload"
                && text(used, "/role")? == role
                && used.get("location") == Some(location),
            "original XML leaf references its exact physical byte coordinate",
        );
        self.inventory.closure(cell)
    }
}

fn entry_index(row: &Value) -> TestResult<u64> {
    if let Some(index) = row["entryIndex"].as_u64() {
        return Ok(index);
    }
    Ok(text(row, "/entryIndex")?.parse()?)
}

const fn marker(encryption: SourceEncryption) -> Option<bool> {
    match encryption {
        SourceEncryption::Encrypted => Some(true),
        SourceEncryption::Unencrypted => Some(false),
        _ => None,
    }
}

pub(super) fn node_location(node: SourceElement<'_>, field: &str) -> TestResult<Value> {
    let (syntax, indices) = match node.position() {
        SourceElementPosition::Text(span) => {
            ("text", vec![span.start.to_string(), span.end.to_string()])
        }
        SourceElementPosition::CryXml(node) => ("cryXml", vec![node.to_string()]),
        _ => return Err("supported original XML element coordinate".into()),
    };
    Ok(location(syntax, &indices, field))
}

fn attribute_location(attr: &SourceAttribute<'_>) -> TestResult<Value> {
    let (syntax, indices) = match attr.position() {
        SourceAttributePosition::Text(span) => {
            ("text", vec![span.start.to_string(), span.end.to_string()])
        }
        SourceAttributePosition::CryXml {
            node_index,
            attribute_index,
        } => (
            "cryXml",
            vec![node_index.to_string(), attribute_index.to_string()],
        ),
        _ => return Err("supported original XML attribute coordinate".into()),
    };
    Ok(location(syntax, &indices, &format!("@{}", attr.name())))
}

fn location(syntax: &str, indices: &[String], field: &str) -> Value {
    json!({"type":"subresource", "namespace":format!("xmlSource.{syntax}"), "indices":indices, "field":field})
}

pub(super) fn read_bytes(archive: &Archive, entry: &Entry) -> TestResult<Vec<u8>> {
    let mut read = archive
        .open_entry_verified(entry)
        .map_err(|_| "open original verified Entry")?
        .reject_overlong();
    let mut bytes = Vec::new();
    read.read_to_end(&mut bytes)
        .map_err(|_| "read verified original Entry to EOF")?;
    assert!(
        u64::try_from(bytes.len())? == entry.uncompressed_size(),
        "complete original entry"
    );
    Ok(bytes)
}

pub(super) fn resolve<'archive>(
    archive: &'archive Archive,
    authored: &str,
) -> TestResult<&'archive Entry> {
    let canonical = authored.replace('\\', "/");
    let path = if canonical
        .get(..5)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("data/"))
    {
        canonical
    } else {
        format!("Data/{canonical}")
    };
    unique(
        archive.entries().iter().filter(|entry| {
            entry
                .path()
                .bytes()
                .map(|unit| {
                    if unit == b'\\' {
                        b'/'
                    } else {
                        unit.to_ascii_lowercase()
                    }
                })
                .eq(path.bytes().map(|unit| unit.to_ascii_lowercase()))
        }),
        "unique original archive selector",
    )
}
