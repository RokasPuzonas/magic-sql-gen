use std::io::Read;

use anyhow::Result;
use thiserror::Error;
use xml::{attribute::OwnedAttribute, name::OwnedName, reader::XmlEvent, EventReader};

pub struct MyEventReader<R: Read> {
	depth: u32,
	event_reader: EventReader<R>,
}

impl<R: Read> MyEventReader<R> {
	pub fn next(&mut self) -> Result<XmlEvent> {
		let event = self.event_reader.next()?;
		if let XmlEvent::StartElement { .. } = event {
			self.depth += 1;
		} else if let XmlEvent::EndElement { .. } = event {
			self.depth -= 1;
		}
		Ok(event)
	}

	#[inline(always)]
	pub fn depth(&self) -> u32 {
		self.depth
	}
}

impl<R: Read> From<EventReader<R>> for MyEventReader<R> {
	fn from(event_reader: EventReader<R>) -> Self {
		MyEventReader {
			depth: 0,
			event_reader,
		}
	}
}

#[derive(Error, Debug, PartialEq)]
pub enum ParseProjectError {
	#[error("XML attribute '{}' not found", format_name_from_parts(.0, .1))]
	AttributeNotFound(Option<String>, String),

	#[error("Unexpected end of XML document")]
	EndOfDocument,
}

fn format_name_from_parts(prefix: &Option<String>, local_name: &str) -> String {
	if let Some(prefix) = &prefix {
		format!("{}:{}", prefix, local_name)
	} else {
		local_name.into()
	}
}

pub fn format_name(name: &OwnedName) -> String {
	format_name_from_parts(&name.prefix, &name.local_name)
}

pub fn check_name(name: &OwnedName, prefix: Option<&str>, local_name: &str) -> bool {
	name.local_name.eq(local_name) && name.prefix_ref().eq(&prefix)
}

pub fn get_attribute<'a>(
	attributes: &'a [OwnedAttribute],
	prefix: Option<&str>,
	name: &str,
) -> Result<&'a str, ParseProjectError> {
	Ok(attributes
		.iter()
		.find(|attr| check_name(&attr.name, prefix, name))
		.map(|attr| &attr.value[..])
		.ok_or_else(|| {
			ParseProjectError::AttributeNotFound(prefix.map(|s| s.to_owned()), name.to_owned())
		})?)
}

#[inline(always)]
pub fn check_attribute(
	attributes: &[OwnedAttribute],
	prefix: Option<&str>,
	name: &str,
	expected_value: &str,
) -> bool {
	if let Ok(attr) = get_attribute(attributes, prefix, name) {
		return attr.eq(expected_value);
	}
	false
}

pub fn get_element_characters<R: Read>(parser: &mut MyEventReader<R>) -> Result<String> {
	let mut parts = vec![];

	loop {
		match parser.next()? {
			XmlEvent::Characters(text) => {
				parts.push(text);
			}
			XmlEvent::EndElement { name } => {
				break;
			}
			_ => {}
		}
	}

	Ok(parts.join(" "))
}

pub fn parse_element<R: Read, F>(
	parser: &mut MyEventReader<R>,
	process_element: &mut F,
) -> Result<()>
where
	F: FnMut(&mut MyEventReader<R>, OwnedName, Vec<OwnedAttribute>) -> Result<()>,
{
	let starting_depth = parser.depth();
	loop {
		match parser.next()? {
			XmlEvent::StartElement {
				name, attributes, ..
			} => {
				process_element(parser, name, attributes)?;
				if parser.depth() == starting_depth - 1 {
					break;
				}
			}
			XmlEvent::EndElement { name } => {
				if parser.depth() == starting_depth - 1 {
					break;
				}
			}
			XmlEvent::EndDocument => Err(ParseProjectError::EndOfDocument)?,
			_ => {}
		}
	}

	return Ok(());
}

#[macro_export]
macro_rules! unwrap_err_continue {
	($res:expr) => {
		match $res {
			Ok(val) => val,
			Err(e) => {
				continue;
			}
		}
	};
}

#[macro_export]
macro_rules! unwrap_opt_continue {
	($res:expr) => {
		match $res {
			Some(val) => val,
			None => {
				continue;
			}
		}
	};
}
