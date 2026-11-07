// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_shader::{Declaration, Field, PropertyFieldKind, RootFieldKind};

use crate::MediaError;
use crate::shader_declaration::{ShaderDeclaration, ShaderForm, ShaderMeaning, ShaderParent};
use crate::shader_mask::ShaderMask;
use crate::shader_span::{ShaderSpan, ShaderText};

pub(crate) fn declaration(
    text: &str,
    node: &Declaration,
    meaning: ShaderMeaning,
    parent: Option<ShaderParent>,
) -> Result<ShaderDeclaration, MediaError> {
    Ok(ShaderDeclaration {
        span: ShaderSpan::from_reader(node.span)?,
        name: node.name.map(|span| token(text, span)).transpose()?,
        form: form(node.form),
        value: node.value.map(|span| token(text, span)).transpose()?,
        literal: node.literal.map(|span| token(text, span)).transpose()?,
        meaning,
        child_count: count(node.children.len())?,
        parent,
    })
}

pub(crate) fn count(value: usize) -> Result<u64, MediaError> {
    u64::try_from(value)
        .map_err(|_| MediaError::Contract("shader ordinal or count exceeds u64".into()))
}

fn token(text: &str, span: p4kx_shader::Span) -> Result<ShaderText, MediaError> {
    let original = span.text(text).ok_or_else(|| {
        MediaError::Contract("shader token range is not valid original UTF-8".into())
    })?;
    Ok(ShaderText {
        span: ShaderSpan::from_reader(span)?,
        text: original.to_owned(),
    })
}

const fn form(form: p4kx_shader::Form) -> ShaderForm {
    use p4kx_shader::Form;

    match form {
        Form::Marker => ShaderForm::Marker,
        Form::Equals => ShaderForm::Equals,
        Form::Parentheses => ShaderForm::Parentheses,
        Form::Block => ShaderForm::Block,
        Form::Unknown | _ => ShaderForm::Unknown,
    }
}

pub(crate) fn root_meaning(field: Field<'_>) -> ShaderMeaning {
    match field.root_kind() {
        Some(RootFieldKind::Version) => ShaderMeaning::Version,
        Some(RootFieldKind::UsesCommonGlobalFlags) => ShaderMeaning::UsesCommonGlobalFlags,
        None | Some(_) => ShaderMeaning::Unknown,
    }
}

pub(crate) fn property_meaning(field: Field<'_>) -> ShaderMeaning {
    match field.property_kind() {
        Some(PropertyFieldKind::Name) => ShaderMeaning::Name,
        Some(PropertyFieldKind::Mask) => ShaderMeaning::Mask {
            interpretation: ShaderMask::from_reader(field.mask()),
        },
        Some(PropertyFieldKind::Label) => ShaderMeaning::Label,
        Some(PropertyFieldKind::Description) => ShaderMeaning::Description,
        Some(PropertyFieldKind::DependencySet) => ShaderMeaning::DependencySet,
        Some(PropertyFieldKind::DependencyReset) => ShaderMeaning::DependencyReset,
        Some(PropertyFieldKind::Hidden) => ShaderMeaning::Hidden,
        None | Some(_) => ShaderMeaning::Unknown,
    }
}
