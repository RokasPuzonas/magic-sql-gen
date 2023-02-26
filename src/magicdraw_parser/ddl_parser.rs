use std::io::{Read, Seek};

use xml::attribute::OwnedAttribute;
use xml::name::OwnedName;
use xml::{EventReader, reader::XmlEvent};
use zip::ZipArchive;
use anyhow::{Result, Context, Ok};

use crate::magicdraw_parser::utils::get_attribute;

use super::utils::{check_name, check_attribute, MyEventReader, parse_element};

#[derive(Debug)]
pub struct DDLClass {
	pub class_id: String,
	pub property_ids: Vec<String>
}

#[derive(Debug)]
pub struct DDLScript {
	pub script_id: String,
	pub classess: Vec<DDLClass>
}

#[derive(Debug)]
pub struct DDLProject {
	pub model_id: String,
	pub scripts: Vec<DDLScript>
}

fn get_id_from_href(attrs: &[OwnedAttribute]) -> Option<String> {
	let href = get_attribute(attrs, None, "href").ok()?;
	let parts = href.split_once("#")?;
	Some(parts.1.to_string())
}

fn parse_class<R: Read>(parser: &mut MyEventReader<R>, attrs: &[OwnedAttribute]) -> Result<DDLClass> {
	let mut property_ids = vec![];
	let mut class_id = None;

	fn is_model_element(name: &OwnedName, attributes: &[OwnedAttribute]) -> bool {
		check_name(name, None, "modelElement") && check_attribute(&attributes, Some("xsi"), "type", "uml:Class")
	}

	fn is_property_element(name: &OwnedName, attributes: &[OwnedAttribute]) -> bool {
		check_name(name, None, "modelElement") && check_attribute(&attributes, Some("xsi"), "type", "uml:Property")
	}

	parse_element(parser, &mut |p, name, attrs| {
		if is_model_element(&name, &attrs) && class_id.is_none() {
			class_id = get_id_from_href(&attrs);
		} else if is_property_element(&name, &attrs) {
			property_ids.push(get_id_from_href(&attrs).context("Property id not found")?);
		}
		Ok(())
	})?;

	Ok(DDLClass {
		class_id: class_id.context("Missing class id")?,
		property_ids
	})
}

fn parse_script<R: Read>(parser: &mut MyEventReader<R>, attrs: &[OwnedAttribute]) -> Result<DDLScript> {
	let mut classess = vec![];
	let mut script_id = None;

	fn is_model_element(name: &OwnedName, attributes: &[OwnedAttribute]) -> bool {
		check_name(name, None, "modelElement") && check_attribute(&attributes, Some("xsi"), "type", "uml:Component")
	}

	fn is_class_element(name: &OwnedName, attributes: &[OwnedAttribute]) -> bool {
		check_name(name, None, "objects") && check_attribute(&attributes, Some("xsi"), "type", "md.ce.rt.objects:RTClassObject")
	}

	parse_element(parser, &mut |p, name, attrs| {
		if is_model_element(&name, &attrs) && script_id.is_none() {
			script_id = get_id_from_href(&attrs);
		} else if is_class_element(&name, &attrs) {
			classess.push(parse_class(p, &attrs)?);
		}
		Ok(())
	})?;

	Ok(DDLScript {
		script_id: script_id.context("Missing script id")?,
		classess
	})
}

fn parse_project<R: Read>(parser: &mut MyEventReader<R>, attrs: &[OwnedAttribute]) -> Result<DDLProject> {
	let mut scripts = vec![];
	let mut model_id = None;

	fn is_model_element(name: &OwnedName, attributes: &[OwnedAttribute]) -> bool {
		check_name(name, None, "modelElement") && check_attribute(&attributes, Some("xsi"), "type", "uml:Model")
	}

	fn is_component_element(name: &OwnedName, attributes: &[OwnedAttribute]) -> bool {
		check_name(name, None, "objects") && check_attribute(&attributes, Some("xsi"), "type", "md.ce.rt.objects:RTComponent")
	}

	parse_element(parser, &mut |p, name, attrs| {
		if is_model_element(&name, &attrs) && model_id.is_none() {
			model_id = get_id_from_href(&attrs);
		} else if is_component_element(&name, &attrs) {
			scripts.push(parse_script(p, &attrs)?);
		}
		Ok(())
	})?;

	Ok(DDLProject {
		model_id: model_id.context("Missing model id")?,
		scripts
	})
}

pub fn parse_ddl_scripts<R: Read + Seek>(project: &mut ZipArchive<R>) -> Result<Vec<DDLProject>> {
	let mut ddl_scripts = vec![];

	let file = project.by_name("personal-com.nomagic.magicdraw.ce.dmn.personaldmncodeengineering")?;
	let mut parser: MyEventReader<_> = EventReader::new(file).into();

	fn is_project_element(name: &OwnedName, attributes: &[OwnedAttribute]) -> bool {
		check_name(name, None, "contents") && check_attribute(&attributes, Some("xsi"), "type", "md.ce.ddl.rt.objects:DDLProjectObject")
	}

	loop {
		match parser.next()? {
			XmlEvent::StartElement { name, attributes, .. } => {
				if is_project_element(&name, &attributes) {
					ddl_scripts.push(parse_project(&mut parser, &attributes)?);
				}
			},
			XmlEvent::EndDocument => { break; },
			_ => {}
		}
	}

	Ok(ddl_scripts)
}
