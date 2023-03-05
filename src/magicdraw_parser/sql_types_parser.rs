use std::{
	collections::{HashMap, HashSet},
	io::{Read, Seek},
};

use anyhow::{bail, Context, Result};
use xml::{attribute::OwnedAttribute, name::OwnedName, reader::XmlEvent, EventReader};
use zip::ZipArchive;

use crate::unwrap_opt_continue;

use super::utils::{check_attribute, check_name, get_attribute, parse_element, MyEventReader};

#[derive(Debug)]
struct UsedPackage {
	share_point_id: String,
	name: String,
	needed_types: Vec<String>,
}

#[derive(Debug, Clone, Copy)]
pub enum SQLTypeName {
	Int,
	Decimal,
	Date,
	Float,
	Bool,
	Char,
	Varchar,
}

fn get_used_project_name(attrs: &[OwnedAttribute]) -> Option<&str> {
	let project_uri = get_attribute(&attrs, None, "usedProjectURI").ok()?;
	project_uri.split("/").last()
}

fn parse_used_package<R: Read>(
	parser: &mut MyEventReader<R>,
	attrs: &[OwnedAttribute],
	needed_types: &[&str],
) -> Result<UsedPackage> {
	let mut share_point_id = None;
	let project_uri = get_attribute(&attrs, None, "usedProjectURI")?;
	let name = project_uri.split("/").last().unwrap();

	parse_element(parser, &mut |p, name, attrs| {
		if share_point_id.is_none() && check_name(&name, None, "mountPoints") {
			share_point_id = get_attribute(&attrs, None, "sharePointID")
				.ok()
				.map(str::to_string);
		}
		Ok(())
	})?;

	Ok(UsedPackage {
		name: name.to_string(),
		share_point_id: share_point_id.context("Share point id not found")?,
		needed_types: needed_types.iter().map(|s| s.to_string()).collect(),
	})
}

fn list_used_packages<R: Read>(
	file: R,
	needed_types: &HashSet<&String>,
) -> Result<Vec<UsedPackage>> {
	let mut packages = vec![];

	let mut needed_types_per_package = HashMap::new();
	for needed_type in needed_types.iter() {
		let (package_name, type_id) = unwrap_opt_continue!(needed_type.split_once("#"));
		let ids = needed_types_per_package
			.entry(package_name)
			.or_insert(vec![]);
		ids.push(type_id);
	}

	let mut parser: MyEventReader<_> = EventReader::new(file).into();
	loop {
		match parser.next()? {
			XmlEvent::StartElement {
				name, attributes, ..
			} => {
				if check_name(&name, None, "projectUsages") {
					let project_name = unwrap_opt_continue!(get_used_project_name(&attributes));
					if let Some(needed_types_for_package) =
						needed_types_per_package.get(&project_name)
					{
						packages.push(parse_used_package(
							&mut parser,
							&attributes,
							needed_types_for_package,
						)?);
					}
				}
			}
			XmlEvent::EndDocument => {
				break;
			}
			_ => {}
		}
	}

	Ok(packages)
}

fn is_umodel_snapshot_file(filename: &str) -> bool {
	filename.ends_with("_resource_com$dnomagic$dmagicdraw$duml_umodel$dshared_umodel$dsnapshot")
}

fn parse_type_name(str: &str) -> Result<SQLTypeName> {
	use SQLTypeName::*;
	Ok(match str {
		"decimal" | "dec" => Decimal,
		"char" => Char,
		"varchar" => Varchar,
		"float" => Float,
		"Integer" | "integer" | "int" => Int,
		"date" => Date,
		"Boolean" => Bool,
		_ => bail!("Unknown SQL type: '{}'", str),
	})
}

fn parse_types_package<R: Read>(
	parser: &mut MyEventReader<R>,
) -> Result<Vec<(String, SQLTypeName)>> {
	let mut types = vec![];

	fn is_primitive_type_element(name: &OwnedName, attrs: &[OwnedAttribute]) -> bool {
		check_name(&name, None, "packagedElement")
			&& check_attribute(&attrs, Some("xsi"), "type", "uml:PrimitiveType")
	}

	parse_element(parser, &mut |p, name, attrs| {
		if is_primitive_type_element(&name, &attrs) {
			let type_name = get_attribute(&attrs, None, "name")?;
			if !type_name.eq("StructuredExpression") {
				types.push((
					get_attribute(&attrs, Some("xmi"), "id")?.to_string(),
					parse_type_name(type_name)?,
				));
			}
		}
		Ok(())
	})?;

	Ok(types)
}

fn parse_primitive_types<R: Read>(
	reader: R,
	used_packages: &[UsedPackage],
) -> Result<Vec<(String, SQLTypeName)>> {
	let mut types = vec![];

	let mut parser: MyEventReader<_> = EventReader::new(reader).into();
	loop {
		match parser.next()? {
			XmlEvent::StartElement {
				name, attributes, ..
			} => {
				if check_name(&name, Some("uml"), "Package") {
					if let Some(id) = get_attribute(&attributes, None, "ID").ok() {
						if let Some(package) =
							used_packages.iter().find(|p| p.share_point_id.eq(id))
						{
							let package_types = parse_types_package(&mut parser)?
								.into_iter()
								.filter(|t| package.needed_types.contains(&t.0))
								.map(|(id, type_name)| {
									(format!("{}#{}", package.name, id), type_name)
								});
							types.extend(package_types);
						}
					}
				}
			}
			XmlEvent::EndDocument => {
				break;
			}
			_ => {}
		}
	}

	Ok(types)
}

pub fn parse_sql_types<R: Read + Seek>(
	project: &mut ZipArchive<R>,
	needed_types: &HashSet<&String>,
) -> Result<HashMap<String, SQLTypeName>> {
	let mut type_names = HashMap::new();

	let meta_model_file = project.by_name("com.nomagic.ci.metamodel.project")?;
	let used_packages = list_used_packages(meta_model_file, needed_types)?;

	let snapshot_files = project
		.file_names()
		.filter(|f| is_umodel_snapshot_file(f))
		.map(|f| f.to_string())
		.collect::<Vec<_>>();

	for filename in &snapshot_files {
		let f = project.by_name(filename).unwrap();
		for (id, type_name) in parse_primitive_types(f, &used_packages)? {
			type_names.insert(id, type_name);
		}
	}

	Ok(type_names)
}
