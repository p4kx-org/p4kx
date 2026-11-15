// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::{AiName, DraftNpcPool, NpcSpawnGroup, NpcSpawnOption, NpcSpawnPool, PublishedNpcPool};
use crate::catalog_projection::{CatalogProjection, ProtectedKey};
use crate::catalog_text_contract::GameTextRef;
use crate::contract::every_field_required;
use crate::guid_text::CatalogGuid;
use crate::{CatalogError, ExportAccountingError};

/// Key-only form of an embedded name used to identify an NPC spawn pool.
#[derive(Debug, Clone, Serialize)]
#[serde(transparent)]
pub(crate) struct PendingNpcName<'source> {
    pub(crate) legacy: AiName<'source>,
    #[serde(skip)]
    pub(crate) keys: Option<[ProtectedKey<'source>; 3]>,
}

/// Protected text references and settings of an NPC name.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(rename = "NpcSpawnName", transform = every_field_required)]
pub(crate) struct NpcSpawnName<Text = SourcedValue<GameTextRef>> {
    pub(crate) random_name: Option<bool>,
    pub(crate) random_last_name: Option<bool>,
    pub(crate) random_nick_name: Option<bool>,
    /// The original `characterGivenName`.
    #[schemars(extend("x-gameText" = true))]
    pub(crate) given_name: Text,
    /// The original `characterGivenLastName`.
    #[schemars(extend("x-gameText" = true))]
    pub(crate) given_last_name: Text,
    /// The original `characterGivenNickName`.
    #[schemars(extend("x-gameText" = true))]
    pub(crate) given_nick_name: Text,
    pub(crate) chance_of_nick_name: Option<f32>,
    pub(crate) name_data_guid: Option<CatalogGuid>,
}

impl<'source> PendingNpcName<'source> {
    fn finish(
        self,
        projection: &CatalogProjection<'_, 'source>,
    ) -> Result<NpcSpawnName, CatalogError> {
        let [given, family, nickname] = self
            .keys
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        Ok(NpcSpawnName {
            random_name: self.legacy.random_name,
            random_last_name: self.legacy.random_last_name,
            random_nick_name: self.legacy.random_nick_name,
            given_name: projection.finish_protected_key(given)?,
            given_last_name: projection.finish_protected_key(family)?,
            given_nick_name: projection.finish_protected_key(nickname)?,
            chance_of_nick_name: self.legacy.chance_of_nick_name,
            name_data_guid: self.legacy.name_data_guid,
        })
    }
}

impl<'source> DraftNpcPool<'source> {
    /// Registers names in the selected pool in authored group and option order.
    pub(crate) fn finish(
        self,
        projection: &CatalogProjection<'_, 'source>,
    ) -> Result<PublishedNpcPool, CatalogError> {
        let groups = self
            .groups
            .into_iter()
            .map(|group| {
                Ok(NpcSpawnGroup {
                    name: group.name,
                    options: group
                        .options
                        .into_iter()
                        .map(|option| option.finish(projection))
                        .collect::<Result<_, CatalogError>>()?,
                })
            })
            .collect::<Result<_, CatalogError>>()?;
        Ok(NpcSpawnPool {
            guid: self.guid,
            groups,
        })
    }
}

impl<'source> NpcSpawnOption<PendingNpcName<'source>> {
    fn finish(
        self,
        projection: &CatalogProjection<'_, 'source>,
    ) -> Result<NpcSpawnOption<NpcSpawnName>, CatalogError> {
        let Self {
            priority,
            include_location_ai_spawn_tags,
            weight,
            auto_spawn_settings,
            identifier_tags,
            name_property_variable,
            embedded_name,
        } = self;
        Ok(NpcSpawnOption {
            priority,
            include_location_ai_spawn_tags,
            weight,
            auto_spawn_settings,
            identifier_tags,
            name_property_variable,
            embedded_name: embedded_name
                .map(|name| name.finish(projection))
                .transpose()?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::LocalisedText;

    #[test]
    fn pending_name_preserves_the_untranslated_pool_identity() -> Result<(), serde_json::Error> {
        let legacy = AiName {
            random_name: Some(false),
            random_last_name: None,
            random_nick_name: Some(true),
            given_name: LocalisedText {
                key: Some("@fixture_given"),
                ..LocalisedText::default()
            },
            given_last_name: LocalisedText::default(),
            given_nick_name: LocalisedText::default(),
            chance_of_nick_name: Some(0.25),
            name_data_guid: None,
        };
        let pending = PendingNpcName { legacy, keys: None };
        assert_eq!(
            serde_json::to_string(&pending)?,
            concat!(
                r#"{"randomName":false,"randomLastName":null,"randomNickName":true,"#,
                r#""givenName":{"key":"@fixture_given","en":null,"de":null},"#,
                r#""givenLastName":{"key":null,"en":null,"de":null},"#,
                r#""givenNickName":{"key":null,"en":null,"de":null},"#,
                r#""chanceOfNickName":0.25,"nameDataGuid":null}"#,
            ),
            "the content identifier uses the exact key-only name bytes",
        );
        Ok(())
    }
}
