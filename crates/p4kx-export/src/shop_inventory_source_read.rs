// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, DecimalU64, SourceUseRole, SourceUses, SourceValue};

use crate::catalog_read::parsed_entry_json::{ParsedEntryJson, SourceNodeValue, SourceString};
pub(super) use crate::catalog_read::parsed_entry_json::{check_cancel, push};
use crate::shop_inventory_source::{JsonMember, JsonString, JsonValue, ShopInventorySource};
use crate::{CatalogError, ExportAccountingError};

#[derive(Clone, Copy)]
enum TextDomain {
    Technical,
    Unclassified,
}

impl SourceString<'_> {
    fn into_wire(self, domain: TextDomain) -> JsonString {
        match (self, domain) {
            (Self::Text(text), TextDomain::Technical) => JsonString::Public {
                text: text.into_owned(),
            },
            (Self::Text(text), TextDomain::Unclassified) => JsonString::GameText {
                text: text.into_owned(),
            },
            (Self::Raw(token), _) => JsonString::Raw {
                token: token.to_owned(),
            },
        }
    }
}

impl ParsedEntryJson<'_> {
    pub(super) fn into_wire(
        self,
        sources: &SourceUses,
        public_strings: &[usize],
        cancel: Option<&CancelToken>,
    ) -> Result<ShopInventorySource, CatalogError> {
        let mut values = Vec::new();
        values
            .try_reserve_exact(self.nodes.len())
            .map_err(CatalogError::JsonSourceAllocation)?;
        let mut public = public_strings.iter().copied().peekable();
        for (index, node) in self.nodes.into_iter().enumerate() {
            check_cancel(cancel)?;
            let domain = if public.next_if_eq(&index).is_some() {
                TextDomain::Technical
            } else {
                TextDomain::Unclassified
            };
            let value = node.value.into_wire(sources, domain)?;
            values.push(sources.value(value, std::slice::from_ref(&node.payload), true));
        }
        Ok(ShopInventorySource {
            root: address(self.root)?,
            values,
        })
    }
}

impl SourceNodeValue<'_> {
    fn into_wire(
        self,
        sources: &SourceUses,
        domain: TextDomain,
    ) -> Result<SourceValue<JsonValue>, CatalogError> {
        let value = match self {
            Self::Null => return Ok(SourceValue::Null),
            Self::Boolean(value) => JsonValue::Boolean { value },
            Self::Number(token) => JsonValue::Number {
                token: token.to_owned(),
            },
            Self::String(value) => JsonValue::String {
                value: value.into_wire(domain),
            },
            Self::Array(elements) => JsonValue::Array {
                elements: convert(elements, address)?,
            },
            Self::Object(members) => JsonValue::Object {
                members: convert(members, |member| {
                    let used = member.name_token.observe(SourceUseRole::Payload)?;
                    Ok(JsonMember {
                        name: sources.value(
                            SourceValue::Present(member.name.into_wire(TextDomain::Technical)),
                            &[used],
                            true,
                        ),
                        value: address(member.value)?,
                    })
                })?,
            },
        };
        Ok(SourceValue::Present(value))
    }
}

pub(super) fn address(index: usize) -> Result<DecimalU64, CatalogError> {
    let index = u64::try_from(index).map_err(|_| ExportAccountingError::ConflictingSourceDatum)?;
    Ok(DecimalU64::new(index))
}

pub(super) fn index(reference: DecimalU64) -> Result<usize, ExportAccountingError> {
    usize::try_from(reference.get()).map_err(|_| ExportAccountingError::ConflictingSourceDatum)
}

fn convert<T, U>(
    input: Vec<T>,
    mut project: impl FnMut(T) -> Result<U, CatalogError>,
) -> Result<Vec<U>, CatalogError> {
    let mut result = Vec::new();
    result
        .try_reserve_exact(input.len())
        .map_err(CatalogError::JsonSourceAllocation)?;
    for value in input {
        result.push(project(value)?);
    }
    Ok(result)
}
