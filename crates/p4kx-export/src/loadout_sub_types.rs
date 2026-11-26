// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use serde::{Serialize, Serializer};

#[derive(Clone, Debug)]
pub(crate) struct PortSubTypes {
    values: Vec<String>,
    origins: SubTypeOrigins,
}

#[derive(Clone, Debug)]
enum SubTypeOrigins {
    Shared(SourcedValue<()>),
    Separate(Vec<SourcedValue<()>>),
}

impl PortSubTypes {
    pub(crate) const fn shared(values: Vec<String>, origin: SourcedValue<()>) -> Self {
        Self {
            values,
            origins: SubTypeOrigins::Shared(origin),
        }
    }

    pub(crate) fn separate<E>(
        members: impl IntoIterator<Item = Result<(String, SourcedValue<()>), E>>,
    ) -> Result<Self, E> {
        let (values, origins) = members.into_iter().collect::<Result<_, _>>()?;
        Ok(Self {
            values,
            origins: SubTypeOrigins::Separate(origins),
        })
    }

    pub(super) fn values(&self) -> &[String] {
        &self.values
    }

    pub(super) fn members(&self) -> impl ExactSizeIterator<Item = SourcedValue<&str>> {
        self.values.iter().enumerate().map(|(index, value)| {
            let origin = match &self.origins {
                SubTypeOrigins::Shared(origin) => origin,
                SubTypeOrigins::Separate(origins) => &origins[index],
            };
            origin.map_ref(|_| value.as_str())
        })
    }
}

impl Serialize for PortSubTypes {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(self.members())
    }
}
