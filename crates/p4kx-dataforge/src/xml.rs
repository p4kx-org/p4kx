// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

#[path = "xml_classify.rs"]
mod classification;
#[path = "xml_observation.rs"]
mod observation;
#[path = "record_xml.rs"]
mod record_xml;
#[path = "xml_sink.rs"]
mod sink;
#[path = "xml_tasks.rs"]
mod tasks;
#[path = "xml_write.rs"]
mod writer;

pub(crate) use classification::classify;
pub use observation::{RecordXmlHeaderMember, RecordXmlSite, RecordXmlSpan, RecordXmlValueMember};
pub use record_xml::{RECORD_XML_NAMESPACE, RecordXml};
