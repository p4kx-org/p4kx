// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::{
    CanonicalRecordHeader, LocationClassification, LocationName, MissionLocationTag,
    MissionLocationTagTerm, MissionLocationTemplate,
};
use serde_json::{Value, json};

use super::{TestResult, proof::View};

impl View<'_> {
    pub(super) fn row(&mut self, row: &MissionLocationTemplate<'_>) -> TestResult<Value> {
        let mut result = self.header(row.source())?;
        let Value::Object(fields) = json!({
            "disabled": self.scalar(row.disabled())?,
            "classification": self.cell(row.classification(), Self::classification)?,
            "configuration": self.cell(row.configuration(), Self::configuration)?,
            "isSecurityNetworkHost": self.scalar(row.is_security_network_host())?,
            "tags": self.tag_list(row.tags())?,
            "producesTags": self.cell(row.produces_tags(), Self::term)?,
            "consumesTags": self.cell(row.consumes_tags(), Self::term)?,
            "aiSpawnTags": self.tag_list(row.ai_spawn_tags())?,
            "names": self.sequence(row.names(), Self::name)?,
            "entityClusterMember": self.cell(row.entity_cluster_member(), |view, value| view.reference(value))?,
        }) else {
            return Err("typed location fields object".into());
        };
        result
            .as_object_mut()
            .ok_or("typed header object")?
            .extend(fields);
        Ok(result)
    }

    fn header(&mut self, source: &CanonicalRecordHeader<'_>) -> TestResult<Value> {
        Ok(json!({
            "guid": self.cell(&source.guid(), |_, value| Ok(json!(value.to_string())))?,
            "recordName": self.scalar(source.record_name())?,
            "className": self.scalar(source.class_name())?,
            "recordPath": self.scalar(source.record_path())?,
            "recordIndex": self.scalar(source.record_index())?,
            "sourceType": self.scalar(source.source_type())?,
            "fileName": self.scalar(source.file_name())?,
            "metadataTag": self.scalar(source.metadata_tag())?,
        }))
    }

    fn name(&mut self, value: &LocationName) -> TestResult<Value> {
        Ok(json!({"tag": self.tag(&value.tag)?, "displayName": self.scalar(&value.display_name)?}))
    }

    fn classification(&mut self, value: &LocationClassification<'_>) -> TestResult<Value> {
        Ok(json!({
            "locationClass": self.cell(value.location_class(), |view, value| view.reference(value))?,
            "locationFunction": self.cell(value.location_function(), |view, value| view.reference(value))?,
            "locationSize": self.cell(value.location_size(), |view, value| view.reference(value))?,
        }))
    }

    fn tag(&mut self, value: &MissionLocationTag) -> TestResult<Value> {
        let path = value.path()?;
        let dangling = value
            .dangling_guid_source()
            .map(|value| self.cell(value, |_, value| Ok(json!(value.get().to_string()))))
            .transpose()?;
        Ok(json!({
            "path": self.cell(&path, |_, value| Ok(json!(value.to_string())))?,
            "danglingGuid": dangling,
        }))
    }

    fn tag_list(
        &mut self,
        value: &p4kx_extraction::SourcedValue<Vec<MissionLocationTag>>,
    ) -> TestResult<Value> {
        self.cell(value, |view, tags| {
            tags.iter()
                .map(|tag| view.tag(tag))
                .collect::<TestResult<Vec<_>>>()
                .map(Value::Array)
        })
    }

    fn term(&mut self, value: &MissionLocationTagTerm) -> TestResult<Value> {
        Ok(json!({
            "positiveTags": self.tag_list(&value.positive_tags)?,
            "negativeTags": self.tag_list(&value.negative_tags)?,
        }))
    }
}
