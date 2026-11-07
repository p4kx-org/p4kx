// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;

use p4kx_shader::{Declaration, Definition, Encoding, Property};

use crate::shader_declaration::{
    ShaderMeaning, ShaderParent, ShaderParentNamespace, ShaderProperty,
};
use crate::shader_payload::{
    ShaderDefinition, ShaderDefinitionCounts, ShaderEncoding, ShaderNameOrigin, ShaderPayload,
};
use crate::shader_span::ShaderSpan;
use crate::{MediaError, MediaWriter, Projection, SourceContext, Status, Subresource};
use crate::{shader_diagnostic, shader_source, shader_values};

pub(crate) const DEFINITION_NAMESPACE: &str = "shader-definition";
pub(crate) const DECLARATION_NAMESPACE: &str = "shader-declaration";
pub(crate) const PROPERTY_NAMESPACE: &str = "shader-property";
pub(crate) const CACHE_NAMESPACE: &str = "shader-cache-inventory";
pub(crate) const FAILURE_NAMESPACE: &str = "shader-definition-failure";

pub(crate) fn project(
    context: &SourceContext<'_>,
    definition: &Definition<'_>,
    writer: &mut MediaWriter<'_>,
) -> Result<Projection, MediaError> {
    header(context, definition, writer)?;
    let mut emitter = Emitter {
        text: definition.text(),
        context,
        writer,
        path: Vec::new(),
    };
    let mut properties = definition.properties().enumerate().peekable();
    for (index, field) in definition.fields().enumerate() {
        emitter.enter(index)?;
        if let Some((ordinal, property)) = properties
            .next_if(|(_, property)| property.declaration().span == field.declaration().span)
        {
            emitter.property(property, ordinal)?;
        } else {
            emitter.node(
                field.declaration(),
                shader_values::root_meaning(field),
                None,
            )?;
        }
        emitter.path.pop();
    }
    Ok(Projection::Written)
}

fn header(
    context: &SourceContext<'_>,
    definition: &Definition<'_>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let file_name = shader_source::file_name(context.entry.path());
    let shader_name = file_name
        .rsplit_once('.')
        .map_or(file_name, |(stem, _)| stem);
    let encoding = match definition.encoding() {
        Encoding::Utf8 => ShaderEncoding::Utf8,
        Encoding::Utf8Bom => ShaderEncoding::Utf8Bom,
        _ => {
            return Err(MediaError::Contract(
                "unsupported successful shader source encoding".into(),
            ));
        }
    };
    let counts = ShaderDefinitionCounts {
        definitions: 1,
        properties: shader_values::count(definition.properties().count())?,
        declarations: declaration_count(definition.declarations(), writer)?,
        root_declarations: shader_values::count(definition.declarations().len())?,
        compiled_caches: 0,
    };
    let data = ShaderPayload::Definition(ShaderDefinition {
        file_name: file_name.to_owned(),
        shader_name: shader_name.to_owned(),
        name_origin: ShaderNameOrigin::FileStem,
        encoding,
        span: ShaderSpan::new(0, definition.source().len())?,
        counts,
    });
    writer.record(
        &Subresource::new(DEFINITION_NAMESPACE, Vec::new()),
        Status::Complete,
        &data,
        &[],
    )
}

fn declaration_count(nodes: &[Declaration], writer: &MediaWriter<'_>) -> Result<u64, MediaError> {
    let mut count = 0_u64;
    for node in nodes {
        writer.checkpoint()?;
        let children = declaration_count(&node.children, writer)?;
        count = count
            .checked_add(1)
            .and_then(|count| count.checked_add(children))
            .ok_or_else(|| MediaError::Contract("shader declaration count exceeds u64".into()))?;
    }
    Ok(count)
}

struct Emitter<'text, 'context, 'entry, 'writer, 'observer> {
    text: &'text str,
    context: &'context SourceContext<'entry>,
    writer: &'writer mut MediaWriter<'observer>,
    path: Vec<u64>,
}

impl Emitter<'_, '_, '_, '_, '_> {
    fn property(&mut self, property: Property<'_>, ordinal: usize) -> Result<(), MediaError> {
        let declaration = shader_values::declaration(
            self.text,
            property.declaration(),
            ShaderMeaning::Property,
            None,
        )?;
        let data = ShaderPayload::Property(ShaderProperty {
            ordinal: shader_values::count(ordinal)?,
            declaration,
        });
        let path = self.copy_path()?;
        self.writer.record(
            &Subresource::new(PROPERTY_NAMESPACE, path),
            Status::Complete,
            &data,
            &[],
        )?;
        for (index, field) in property.fields().enumerate() {
            let parent = self.parent(ShaderParentNamespace::ShaderProperty)?;
            self.enter(index)?;
            self.node(
                field.declaration(),
                shader_values::property_meaning(field),
                Some(parent),
            )?;
            self.path.pop();
        }
        Ok(())
    }

    fn node(
        &mut self,
        node: &Declaration,
        meaning: ShaderMeaning,
        parent: Option<ShaderParent>,
    ) -> Result<(), MediaError> {
        self.writer.checkpoint()?;
        let (status, issues) = shader_diagnostic::mask_status(&meaning);
        let data = ShaderPayload::Declaration(shader_values::declaration(
            self.text, node, meaning, parent,
        )?);
        let path = self.copy_path()?;
        self.writer.record(
            &Subresource::new(DECLARATION_NAMESPACE, path),
            status,
            &data,
            &issues,
        )?;
        for (index, child) in node.children.iter().enumerate() {
            let parent = self.parent(ShaderParentNamespace::ShaderDeclaration)?;
            self.enter(index)?;
            self.node(child, ShaderMeaning::Unknown, Some(parent))?;
            self.path.pop();
        }
        Ok(())
    }

    fn enter(&mut self, index: usize) -> Result<(), MediaError> {
        self.writer.checkpoint()?;
        self.path
            .try_reserve(1)
            .map_err(|error| self.allocation_error(error))?;
        self.path.push(shader_values::count(index)?);
        Ok(())
    }

    fn copy_path(&self) -> Result<Vec<u64>, MediaError> {
        let mut path = Vec::new();
        path.try_reserve_exact(self.path.len())
            .map_err(|error| self.allocation_error(error))?;
        path.extend_from_slice(&self.path);
        Ok(path)
    }

    fn parent(&self, namespace: ShaderParentNamespace) -> Result<ShaderParent, MediaError> {
        Ok(ShaderParent {
            namespace,
            indices: self.copy_path()?,
        })
    }

    fn allocation_error(&self, error: std::collections::TryReserveError) -> MediaError {
        MediaError::Io {
            path: self.context.payload.to_path_buf(),
            source: io::Error::new(io::ErrorKind::OutOfMemory, error),
        }
    }
}
