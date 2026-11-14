// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;
use serde_json::value::RawValue;

use crate::artifact::BoundedLine;
use crate::material_parts_origins::{payload, prepared};
use crate::material_payload::StreamMaterialWire;
use crate::observed_write::ObservedWrite;
use crate::origin_prepared::PreparedRecordOrigins;
use crate::origin_reference::{child, independent};
use crate::{MediaError, MediaWriter, SourceContext, Status, Subresource, origin_error};

pub(crate) const PART: &str = "material-library-part";
pub(crate) const REFERENCE: &str = "material-reference";
pub(crate) const SHADER_REFERENCE: &str = "material-shader-reference";

#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct BatchItems<'a, T> {
    #[schemars(length(min = 1))]
    items: &'a [T],
}

#[derive(Clone, Copy)]
pub(crate) enum Band {
    Part,
    Reference,
    Shader,
}

impl Band {
    const fn namespace(self) -> &'static str {
        match self {
            Self::Part => PART,
            Self::Reference => REFERENCE,
            Self::Shader => SHADER_REFERENCE,
        }
    }

    const fn envelope(self, items: &[Box<RawValue>]) -> StreamMaterialWire<'_, '_> {
        let payload = BatchItems { items };
        match self {
            Self::Part => StreamMaterialWire::Part(payload),
            Self::Reference => StreamMaterialWire::Reference(payload),
            Self::Shader => StreamMaterialWire::ShaderReference(payload),
        }
    }
}

pub(crate) struct Batch<'a, 'source> {
    context: &'a SourceContext<'source>,
    band: Band,
    position: u64,
    bytes: u64,
    origins: Option<PreparedRecordOrigins>,
    items: Vec<Box<RawValue>>,
}

impl<'a, 'source> Batch<'a, 'source> {
    pub(crate) const fn new(context: &'a SourceContext<'source>, band: Band) -> Self {
        Self {
            context,
            band,
            position: 0,
            bytes: 0,
            origins: None,
            items: Vec::new(),
        }
    }

    pub(crate) fn push(
        &mut self,
        item: &impl Serialize,
        origins: PreparedRecordOrigins,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let limit = writer.limits().record_bytes;
        let value = measure(item, writer, limit)?;
        let length = crate::shader_values::count(value.bytes.len())?;
        let json = String::from_utf8(value.bytes)
            .map_err(|_| MediaError::Contract("material serializer returned non-UTF-8".into()))?;
        let value = RawValue::from_string(json)?;
        let existing_increase = length
            .checked_add(u64::from(!self.items.is_empty()))
            .ok_or_else(|| limit_error(limit))?;
        if self.origins.is_some()
            && self
                .bytes
                .checked_add(existing_increase)
                .is_none_or(|sum| sum > limit)
        {
            self.flush(writer)?;
        }
        self.start(writer)?;
        let extra = if self.items.is_empty() {
            length
        } else {
            existing_increase
        };
        self.bytes = self
            .bytes
            .checked_add(extra)
            .filter(|length| *length <= limit)
            .ok_or_else(|| limit_error(limit))?;
        let mut destination = child(&payload()?, "items")?;
        destination
            .push_index(self.items.len())
            .map_err(|cause| origin_error::address(&cause))?;
        self.origins
            .as_mut()
            .ok_or_else(|| MediaError::Contract("missing packed material origins".into()))?
            .mount_payload(origins, &payload()?, &destination, writer)?;
        self.items.push(value);
        Ok(())
    }

    fn start(&mut self, writer: &MediaWriter<'_>) -> Result<(), MediaError> {
        if self.origins.is_none() {
            self.bytes = writer.measure_record(&self.identity(), &self.band.envelope(&[]))?;
            let mut origins = prepared(self.context, writer)?;
            independent(&mut origins, &payload()?, writer)?;
            self.origins = Some(origins);
        }
        Ok(())
    }

    fn flush(&mut self, writer: &mut MediaWriter<'_>) -> Result<(), MediaError> {
        if let Some(origins) = self.origins.take() {
            let count = crate::shader_values::count(self.items.len())?;
            let next = self
                .position
                .checked_add(count)
                .ok_or_else(|| MediaError::Contract("material item count overflow".into()))?;
            writer.record_with_origins(
                &self.identity(),
                Status::Complete,
                &self.band.envelope(&self.items),
                &[],
                origins,
            )?;
            self.items.clear();
            self.position = next;
        }
        Ok(())
    }

    pub(crate) fn finish(&mut self, writer: &mut MediaWriter<'_>) -> Result<u64, MediaError> {
        self.flush(writer)?;
        Ok(self.position)
    }

    fn identity(&self) -> Subresource {
        Subresource::new(self.band.namespace(), vec![self.position])
    }
}

fn measure(
    item: &impl Serialize,
    writer: &MediaWriter<'_>,
    limit: u64,
) -> Result<BoundedLine, MediaError> {
    let mut line = BoundedLine::new(limit);
    // The envelope consumes three object positions and one array.
    let depth = writer.limits().value_depth.checked_sub(4).ok_or_else(|| {
        MediaError::Contract("material part exceeds the record value_depth budget".into())
    })?;
    let actual = item.serialize(crate::finite_serializer::Finite(
        &mut serde_json::Serializer::new(ObservedWrite::new(&mut line, &writer.checkpoint)),
        depth,
    ));
    crate::json_output::finish(actual, writer.checkpoint)?;
    Ok(line)
}

const fn limit_error(limit: u64) -> MediaError {
    MediaError::Limit {
        budget: "record_bytes",
        limit,
    }
}
