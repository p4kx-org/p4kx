// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::{ResourceFloat32, ResourceFloat64, ResourceNumber};
use p4kx_extraction::{Guid, SourceInventory, SourceValueState, SourcedValue};
use serde::Serialize;
use serde_json::Value;

pub(super) type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

#[derive(Clone, Copy)]
pub(super) struct Wire<'a> {
    expected: &'a Value,
    inventory: &'a SourceInventory,
}

impl<'a> Wire<'a> {
    pub(super) const fn new(expected: &'a Value, inventory: &'a SourceInventory) -> Self {
        Self {
            expected,
            inventory,
        }
    }

    pub(super) fn field(self, name: &str) -> TestResult<Self> {
        Ok(Self {
            expected: self
                .expected
                .get(name)
                .ok_or_else(|| format!("wire field {name}"))?,
            ..self
        })
    }

    pub(super) fn raw<T: Serialize + ?Sized>(self, value: &T) -> TestResult {
        assert_eq!(
            serde_json::to_value(value)?,
            *self.expected,
            "typed getter equals canonical wire"
        );
        Ok(())
    }

    pub(super) fn tag(self, kind: &str) -> TestResult {
        self.field("type")?.raw(kind)
    }

    pub(super) fn cell<T>(
        self,
        cell: &SourcedValue<T>,
        visit: impl FnOnce(&T, Self) -> TestResult,
    ) -> TestResult {
        visit_leaf(cell, self.inventory)?;
        self.field("state")?.raw(&cell.state())?;
        self.field("containsEncryptedSource")?
            .raw(&cell.origin().contains_encrypted_source())?;
        self.field("sourceUsesRefs")?
            .raw(cell.origin().source_uses_refs())?;
        self.field("sourceSetComplete")?
            .raw(&cell.origin().source_set_complete())?;
        let payload = self.field("value")?;
        if let Some(value) = cell.value() {
            visit(value, payload)?;
        } else {
            assert!(
                payload.expected.is_null(),
                "unavailable getter has no wire payload"
            );
        }
        Ok(())
    }

    pub(super) fn leaf<T: Serialize>(self, cell: &SourcedValue<T>) -> TestResult {
        self.cell(cell, |value, payload| payload.raw(value))
    }

    pub(super) fn guid(self, cell: &SourcedValue<Guid>) -> TestResult {
        self.cell(cell, |guid, payload| payload.raw(&guid.to_string()))
    }

    pub(super) fn array<T>(
        self,
        cells: &[SourcedValue<T>],
        visit: impl Fn(&T, Self) -> TestResult,
    ) -> TestResult {
        let expected = self.expected.as_array().ok_or("wire array")?;
        assert_eq!(cells.len(), expected.len(), "original array slots survive");
        for (cell, expected) in cells.iter().zip(expected) {
            Self::new(expected, self.inventory).cell(cell, &visit)?;
        }
        Ok(())
    }

    pub(super) fn float32(self, number: &ResourceFloat32) -> TestResult {
        self.field("rawBits")?
            .raw(&format!("{:08x}", number.raw_bits()))?;
        self.field("finite")?.raw(&number.finite())
    }

    pub(super) fn float64(self, number: &ResourceFloat64) -> TestResult {
        self.field("rawBits")?
            .raw(&format!("{:016x}", number.raw_bits()))?;
        self.field("finite")?.raw(&number.finite())
    }

    pub(super) fn number(self, number: &ResourceNumber) -> TestResult {
        match number {
            ResourceNumber::Int8 { value } => self.integer("int8", value),
            ResourceNumber::Int16 { value } => self.integer("int16", value),
            ResourceNumber::Int32 { value } => self.integer("int32", value),
            ResourceNumber::Int64 { value } => self.integer("int64", &value.get().to_string()),
            ResourceNumber::UInt8 { value } => self.integer("uint8", value),
            ResourceNumber::UInt16 { value } => self.integer("uint16", value),
            ResourceNumber::UInt32 { value } => self.integer("uint32", value),
            ResourceNumber::UInt64 { value } => self.integer("uint64", &value.get().to_string()),
            ResourceNumber::Single(value) => {
                self.tag("f32")?;
                self.float32(value)
            }
            ResourceNumber::Double(value) => {
                self.tag("f64")?;
                self.float64(value)
            }
            _ => Err("a new source numeric type requires a typed traversal".into()),
        }
    }

    fn integer<T: Serialize>(self, kind: &str, value: &T) -> TestResult {
        self.tag(kind)?;
        self.field("value")?.raw(value)
    }
}

pub(super) fn visit_leaf<T>(cell: &SourcedValue<T>, inventory: &SourceInventory) -> TestResult {
    assert_eq!(
        cell.state() == SourceValueState::Present,
        cell.value().is_some(),
        "present cells have one payload"
    );
    for reference in cell.origin().source_uses_refs() {
        let mut matches = inventory
            .iter_uses()
            .filter(|description| description.id() == reference);
        let description = matches.next().ok_or("cell use is not in this inventory")?;
        assert!(
            matches.next().is_none(),
            "a use has one identity in this inventory"
        );
        let mut physical = inventory
            .iter_sources()
            .filter(|source| source.id() == description.source());
        let source = physical
            .next()
            .ok_or("cell use has no source in this inventory")?;
        assert!(physical.next().is_none(), "a use has one physical source");
        for ancestor in source.ancestors() {
            assert_eq!(
                inventory
                    .iter_sources()
                    .filter(|source| source.id() == ancestor)
                    .count(),
                1,
                "source ancestry stays in the same inventory"
            );
        }
    }
    Ok(())
}
