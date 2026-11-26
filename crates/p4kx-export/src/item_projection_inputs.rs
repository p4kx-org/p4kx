// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{FieldOrigin, SourceUses, SourceValue, SourceValueState, SourcedValue};

use super::ItemPaintFacts;
use crate::catalog_v3::authored_fields::locale_input::AuthoredLocaleInput;
use crate::catalog_v3::catalog_item_components::item_definition::{
    ItemAttachmentPresentation, ItemDefinition, ItemDefinitionComponent,
    ItemDefinitionComponentKind, ItemLocalization, ItemPurchasable,
};
use crate::catalog_v3::catalog_item_components::selection::ItemSourceSelection;
use crate::{CatalogError, ExportAccountingError};

pub(super) struct SelectedItem<'read, 'source> {
    definition: &'read ItemDefinition<'source>,
    selection: &'read ItemSourceSelection,
    attachment: Option<&'read SourcedValue<Box<ItemAttachmentPresentation<'source>>>>,
    purchasable: Option<&'read ItemPurchasable<'source>>,
}

pub(super) struct ItemLocaleRead<'read, 'source> {
    pub(super) input: Option<&'read AuthoredLocaleInput<'source>>,
    pub(super) state: SourceValueState,
    pub(super) cause: &'read FieldOrigin,
    pub(super) parents: [&'read FieldOrigin; 3],
}

pub(super) fn unavailable_value<T>(
    state: SourceValueState,
) -> Result<Option<SourceValue<T>>, CatalogError> {
    Ok(match state {
        SourceValueState::Present | SourceValueState::Absent | SourceValueState::Null => None,
        SourceValueState::Unreadable => Some(SourceValue::Unreadable),
        SourceValueState::Unsupported => Some(SourceValue::Unsupported),
        SourceValueState::Invalid => Some(SourceValue::Invalid),
        _ => return Err(ExportAccountingError::ConflictingSourceDatum.into()),
    })
}

impl<'read, 'source> SelectedItem<'read, 'source> {
    pub(super) fn read(
        definition: &'read ItemDefinition<'source>,
        selection: &'read ItemSourceSelection,
    ) -> Result<Self, CatalogError> {
        let attachment = selected_attachment(definition, selection.attachment_component_ordinal())?;
        let purchasable = selected(definition, selection.purchasable_component_ordinal())?
            .map(|component| match component.body() {
                ItemDefinitionComponentKind::Purchasable(value) => Ok(value),
                _ => Err(ExportAccountingError::ConflictingSourceDatum),
            })
            .transpose()?;
        Ok(Self {
            definition,
            selection,
            attachment,
            purchasable,
        })
    }

    pub(super) fn paint(&self) -> ItemPaintFacts<'source> {
        ItemPaintFacts::selected(
            self.selection.attachment_component_ordinal(),
            self.attachment,
        )
    }

    pub(super) fn name(&self) -> ItemLocaleRead<'read, 'source> {
        self.attachment_locale(|localization| localization.name_input())
    }

    pub(super) fn description(&self) -> ItemLocaleRead<'read, 'source> {
        self.attachment_locale(|localization| localization.description_input())
    }

    pub(super) fn display_name(&self) -> Option<&'read AuthoredLocaleInput<'source>> {
        self.purchasable.map(ItemPurchasable::display_name_input)
    }

    pub(super) const fn purchasable_ordinal(&self) -> &'read SourcedValue<u32> {
        self.selection.purchasable_component_ordinal()
    }

    pub(super) const fn record_path(&self) -> &SourcedValue<&'source str> {
        &self.definition.source().record_path
    }

    fn attachment_locale(
        &self,
        select: impl FnOnce(&'read ItemLocalization<'source>) -> &'read AuthoredLocaleInput<'source>,
    ) -> ItemLocaleRead<'read, 'source> {
        let selected = self.selection.attachment_component_ordinal();
        let Some(attachment) = self.attachment else {
            return ItemLocaleRead::unavailable(selected, selected.origin());
        };
        let Some(value) = attachment.value() else {
            return ItemLocaleRead::unavailable(attachment, selected.origin());
        };
        let localization = value.localization();
        let Some(value) = localization.value() else {
            return ItemLocaleRead {
                input: None,
                state: localization.state(),
                cause: localization.origin(),
                parents: [
                    selected.origin(),
                    attachment.origin(),
                    localization.origin(),
                ],
            };
        };
        let input = select(value);
        ItemLocaleRead {
            input: Some(input),
            state: input.key().state(),
            cause: input.key().origin(),
            parents: [
                selected.origin(),
                attachment.origin(),
                localization.origin(),
            ],
        }
    }
}

impl<'read, 'source> ItemLocaleRead<'read, 'source> {
    pub(super) fn add_parents<T>(
        &self,
        value: SourcedValue<T>,
        uses: &SourceUses,
    ) -> Result<SourcedValue<T>, CatalogError> {
        let [component, definition, container] = self.parents;
        Ok(uses.add_value_origins(value, &[self.cause, component, definition, container])?)
    }

    const fn unavailable<T>(value: &'read SourcedValue<T>, selection: &'read FieldOrigin) -> Self {
        Self {
            input: None,
            state: value.state(),
            cause: value.origin(),
            parents: [selection, value.origin(), value.origin()],
        }
    }

    pub(super) fn key(&self) -> Option<&'source str> {
        self.input
            .and_then(|input| input.key().value().copied())
            .filter(|key| !key.is_empty())
    }
}

pub(super) fn selected_attachment<'read, 'source>(
    definition: &'read ItemDefinition<'source>,
    ordinal: &SourcedValue<u32>,
) -> Result<Option<&'read SourcedValue<Box<ItemAttachmentPresentation<'source>>>>, CatalogError> {
    Ok(selected(definition, ordinal)?
        .map(|component| match component.body() {
            ItemDefinitionComponentKind::Attachment(value) => Ok(value),
            _ => Err(ExportAccountingError::ConflictingSourceDatum),
        })
        .transpose()?)
}

fn selected<'read, 'source>(
    definition: &'read ItemDefinition<'source>,
    ordinal: &SourcedValue<u32>,
) -> Result<Option<&'read ItemDefinitionComponent<'source>>, CatalogError> {
    let Some(index) = ordinal.value() else {
        return Ok(None);
    };
    definition
        .components()
        .value()
        .and_then(|components| {
            components
                .iter()
                .find(|component| component.component_ordinal().value() == Some(index))
        })
        .map(Some)
        .ok_or_else(|| ExportAccountingError::ConflictingSourceDatum.into())
}
