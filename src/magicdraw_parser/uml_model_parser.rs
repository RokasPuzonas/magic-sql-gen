use std::io::{Read, Seek};

use anyhow::{Context, Result};
use xml::{attribute::OwnedAttribute, name::OwnedName, reader::XmlEvent, EventReader};
use zip::ZipArchive;

use crate::{unwrap_err_continue, unwrap_opt_continue};

use super::utils::{
	check_attribute, check_name, get_attribute, get_element_characters, parse_element,
	MyEventReader, ParseProjectError,
};

#[derive(Debug)]
pub struct UMLProperty {
	pub id: String,
	pub name: Option<String>,
	pub is_id: bool,
	pub type_href: Option<String>,
}

// TODO: Make this an enum? Because from what I have seen there were only 2 cases,
// * Constraint applied to property
// * Constraint applied to class
#[derive(Debug)]
pub struct UMLConstraint {
	pub id: String,
	pub class_id: Option<String>,
	pub property_id: Option<String>,
	pub property_name: Option<String>,
	pub body: Option<String>,
}

#[derive(Debug)]
pub struct UMLClass {
	pub id: String,
	pub name: Option<String>,
	pub properties: Vec<UMLProperty>,
	pub constraints: Vec<UMLConstraint>,
}

#[derive(Debug)]
pub struct UMLPackage {
	pub id: String,
	pub name: Option<String>,
	pub classess: Vec<UMLClass>,
}

#[derive(Debug)]
pub struct UMLModel {
	pub id: String,
	pub name: String,
	pub packages: Vec<UMLPackage>,
}

#[derive(Debug)]
pub struct UMLPrimaryKeyModifier {
	pub property_id: String,
}

#[derive(Debug)]
pub struct UMLNullableModifier {
	pub property_id: String,
	pub nullable: bool,
}

#[derive(Debug)]
pub struct UMLForeignKeyModifier {
	pub from_property_id: String,
	pub to_property_id: String,
}

#[derive(Debug)]
pub struct UMLUniqueModifier {
	pub property_id: String,
}

#[derive(Debug)]
pub struct UMLTypeModifier {
	pub property_id: String,
	pub modifier: String,
}

#[derive(Debug)]
pub enum UMLModifier {
	Unique(UMLUniqueModifier),
	PirmaryKey(UMLPrimaryKeyModifier),
	Nullable(UMLNullableModifier),
	ForeignKey(UMLForeignKeyModifier),
	Type(UMLTypeModifier),
}

fn parse_property<R: Read>(
	parser: &mut MyEventReader<R>,
	attrs: &[OwnedAttribute],
) -> Result<UMLProperty> {
	let id = get_attribute(attrs, Some("xmi"), "id")?.into();
	let name = get_attribute(attrs, None, "name").ok().map(str::to_string);
	let is_id = get_attribute(attrs, None, "isID")
		.unwrap_or("false")
		.eq("true");
	let mut type_href = None;

	parse_element(parser, &mut |p, name, attrs| {
		if check_name(&name, None, "type") && type_href.is_none() {
			if let Ok(value) = get_attribute(&attrs, None, "href") {
				type_href = Some(value.to_string());
			}
		}
		Ok(())
	})?;

	Ok(UMLProperty {
		id,
		name,
		is_id,
		type_href,
	})
}

fn parse_constraint<R: Read>(
	parser: &mut MyEventReader<R>,
	attrs: &[OwnedAttribute],
) -> Result<Option<UMLConstraint>> {
	let id = get_attribute(attrs, Some("xmi"), "id")?.into();
	let mut constrainted_element_id = None;
	let mut language = None;
	let mut body = None;

	parse_element(parser, &mut |p, name, attrs| {
		if check_name(&name, None, "constrainedElement") && constrainted_element_id.is_none() {
			constrainted_element_id = get_attribute(&attrs, Some("xmi"), "idref")
				.ok()
				.map(str::to_string);
		} else if check_name(&name, None, "body") && body.is_none() {
			let contents = get_element_characters(p)?;
			if contents.len() > 0 {
				body = Some(contents);
			}
		} else if check_name(&name, None, "language") && language.is_none() {
			language = Some(get_element_characters(p)?);
		}
		Ok(())
	})?;

	if language.eq(&Some("SQL".into())) && body.is_some() {
		if let Some((prop_name, check_body)) = body.unwrap().split_once(" in ") {
			return Ok(Some(UMLConstraint {
				id,
				class_id: Some(constrainted_element_id.context("Missing constraint class id")?),
				body: Some(format!("in {}", check_body)),
				property_id: None,
				property_name: Some(prop_name.into()),
			}));
		}
	}

	if constrainted_element_id.is_none() {
		return Ok(None);
	}

	return Ok(Some(UMLConstraint {
		id,
		property_id: Some(constrainted_element_id.unwrap()),
		body: None,
		class_id: None,
		property_name: None,
	}));
}

fn parse_class<R: Read>(
	parser: &mut MyEventReader<R>,
	attrs: &[OwnedAttribute],
) -> Result<UMLClass> {
	let mut properties = vec![];
	let mut consraints = vec![];
	let id = get_attribute(attrs, Some("xmi"), "id")?.into();
	let name = get_attribute(attrs, None, "name").ok().map(str::to_string);

	fn is_property_element(name: &OwnedName, attrs: &[OwnedAttribute]) -> bool {
		check_name(name, None, "ownedAttribute")
			&& check_attribute(&attrs, Some("xmi"), "type", "uml:Property")
	}

	fn is_constraint_element(name: &OwnedName, attrs: &[OwnedAttribute]) -> bool {
		check_name(name, None, "ownedRule")
			&& check_attribute(&attrs, Some("xmi"), "type", "uml:Constraint")
	}

	parse_element(parser, &mut |p, name, attrs| {
		if is_property_element(&name, &attrs) {
			properties.push(parse_property(p, &attrs)?);
		} else if is_constraint_element(&name, &attrs) {
			if let Some(constraint) = parse_constraint(p, &attrs)? {
				consraints.push(constraint);
			}
		}
		Ok(())
	})?;

	Ok(UMLClass {
		id,
		name,
		properties,
		constraints: consraints,
	})
}

fn parse_package<R: Read>(
	parser: &mut MyEventReader<R>,
	attrs: &[OwnedAttribute],
) -> Result<UMLPackage> {
	let mut classess = vec![];
	let id = get_attribute(attrs, Some("xmi"), "id")?.into();
	let name = get_attribute(attrs, None, "name").ok().map(str::to_string);

	fn is_class_element(name: &OwnedName, attrs: &[OwnedAttribute]) -> bool {
		check_name(name, None, "packagedElement")
			&& check_attribute(&attrs, Some("xmi"), "type", "uml:Class")
	}

	parse_element(parser, &mut |p, name, attrs| {
		if is_class_element(&name, &attrs) {
			classess.push(parse_class(p, &attrs)?);
		}
		Ok(())
	})?;

	Ok(UMLPackage { id, name, classess })
}

fn parse_model<R: Read>(
	parser: &mut MyEventReader<R>,
	attrs: &[OwnedAttribute],
) -> Result<UMLModel> {
	let mut packages = vec![];
	let id = get_attribute(attrs, Some("xmi"), "id")?.into();
	let name = get_attribute(attrs, None, "name")?.into();

	fn is_package_element(name: &OwnedName, attrs: &[OwnedAttribute]) -> bool {
		check_name(name, None, "packagedElement")
			&& check_attribute(&attrs, Some("xmi"), "type", "uml:Package")
	}

	parse_element(parser, &mut |p, name, attrs| {
		if is_package_element(&name, &attrs) {
			packages.push(parse_package(p, &attrs)?);
		}
		Ok(())
	})?;

	Ok(UMLModel { id, name, packages })
}

fn find_constraint_by_id<'a>(models: &'a [UMLModel], id: &str) -> Option<&'a UMLConstraint> {
	for model in models {
		for package in &model.packages {
			for class in &package.classess {
				for constraint in &class.constraints {
					if constraint.id.eq(id) {
						return Some(constraint);
					}
				}
			}
		}
	}

	None
}

pub fn parse_uml_model<R: Read + Seek>(
	project: &mut ZipArchive<R>,
) -> Result<(Vec<UMLModel>, Vec<UMLModifier>)> {
	let mut models = vec![];
	let mut modifiers = vec![];

	let file = project.by_name("com.nomagic.magicdraw.uml_model.model")?;
	let mut parser: MyEventReader<_> = EventReader::new(file).into();

	loop {
		match parser.next()? {
			XmlEvent::StartElement {
				name, attributes, ..
			} => {
				if check_name(&name, Some("uml"), "Model") {
					models.push(parse_model(&mut parser, &attributes)?);
				} else if check_name(&name, Some("SQLProfile"), "PrimaryKey") {
					let constraint_id =
						unwrap_err_continue!(get_attribute(&attributes, None, "base_Constraint"));
					let constraint =
						unwrap_opt_continue!(find_constraint_by_id(&models, constraint_id));
					let property_id = unwrap_opt_continue!(&constraint.property_id).clone();
					modifiers.push(UMLModifier::PirmaryKey(UMLPrimaryKeyModifier {
						property_id,
					}));
				} else if check_name(&name, Some("SQLProfile"), "PKMember") {
					let property_id =
						unwrap_err_continue!(get_attribute(&attributes, None, "base_Property"))
							.to_string();
					modifiers.push(UMLModifier::PirmaryKey(UMLPrimaryKeyModifier {
						property_id,
					}));
				} else if check_name(&name, Some("SQLProfile"), "Column") {
					let property_id =
						unwrap_err_continue!(get_attribute(&attributes, None, "base_Property"))
							.to_string();
					let nullable =
						unwrap_err_continue!(get_attribute(&attributes, None, "nullable"))
							.eq("true");
					modifiers.push(UMLModifier::Nullable(UMLNullableModifier {
						property_id,
						nullable,
					}));
				} else if check_name(&name, Some("MagicDraw_Profile"), "typeModifier") {
					let property_id =
						unwrap_err_continue!(get_attribute(&attributes, None, "base_Element"))
							.into();
					let modifier =
						unwrap_err_continue!(get_attribute(&attributes, None, "typeModifier"))
							.into();
					modifiers.push(UMLModifier::Type(UMLTypeModifier {
						property_id,
						modifier,
					}));
				} else if check_name(&name, Some("SQLProfile"), "FK") {
					let from_property_id =
						unwrap_err_continue!(get_attribute(&attributes, None, "members")).into();
					let to_property_id =
						unwrap_err_continue!(get_attribute(&attributes, None, "referencedMembers"))
							.into();
					modifiers.push(UMLModifier::ForeignKey(UMLForeignKeyModifier {
						from_property_id,
						to_property_id,
					}));
				}
			}
			XmlEvent::EndDocument => {
				break;
			}
			_ => {}
		}
	}

	Ok((models, modifiers))
}
