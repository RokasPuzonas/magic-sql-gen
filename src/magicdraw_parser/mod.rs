mod ddl_parser;
mod sql_types_parser;
mod uml_model_parser;
mod utils;
use serde::{Deserialize, Serialize};

use anyhow::{Context, Result};
use lazy_regex::regex_captures;
use std::{
	collections::HashSet,
	fmt::Display,
	io::{Read, Seek},
};
use zip::ZipArchive;

use crate::unwrap_opt_continue;

use self::{
	ddl_parser::parse_ddl_scripts,
	sql_types_parser::{parse_sql_types, SQLTypeName},
	uml_model_parser::{
		parse_uml_model, UMLClass, UMLForeignKeyModifier, UMLModel, UMLModifier,
		UMLNullableModifier, UMLPrimaryKeyModifier, UMLTypeModifier,
	},
};

#[derive(Debug, PartialEq, Deserialize, Serialize)]
pub enum SQLType {
	Int,
	Decimal,
	Date,
	Time,
	Datetime,
	Float,
	Bool,
	Char(u8),
	Varchar(u16),
}

impl Display for SQLType {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			SQLType::Int => write!(f, "INT"),
			SQLType::Decimal => write!(f, "DECIMAL"),
			SQLType::Date => write!(f, "DATE"),
			SQLType::Time => write!(f, "TIME"),
			SQLType::Datetime => write!(f, "DATETIME"),
			SQLType::Float => write!(f, "FLOAT"),
			SQLType::Bool => write!(f, "BOOL"),
			SQLType::Char(size) => write!(f, "CHAR({})", size),
			SQLType::Varchar(size) => write!(f, "VARCHAR({})", size),
		}
	}
}

#[derive(Debug, PartialEq, Deserialize, Serialize)]
pub enum SQLCheckConstraint {
	OneOf(Vec<String>),
	Freeform(String),
}

#[derive(Debug, PartialEq, Deserialize, Serialize)]
pub struct SQLColumn {
	pub name: String,
	pub sql_type: SQLType,
	pub primary_key: bool,
	pub nullable: bool,
	pub foreign_key: Option<(String, String)>,
	pub check_constraint: Option<SQLCheckConstraint>,
}

#[derive(Debug, PartialEq, Deserialize, Serialize)]
pub struct SQLTable {
	pub name: String,
	pub columns: Vec<SQLColumn>,
}

#[derive(Debug, PartialEq, Deserialize, Serialize)]
pub struct SQLTableCollection {
	pub tables: Vec<SQLTable>,
}

fn find_class_by_id<'a>(models: &'a [UMLModel], id: &str) -> Option<&'a UMLClass> {
	for model in models {
		for package in &model.packages {
			if let Some(class) = package.classess.iter().find(|t| t.id.eq(id)) {
				return Some(class);
			}
		}
	}
	None
}

fn is_nullabe(modifiers: &[UMLModifier], property: &str) -> bool {
	for modifier in modifiers {
		if let UMLModifier::Nullable(UMLNullableModifier {
			property_id,
			nullable,
		}) = modifier
		{
			if property_id.eq(property) {
				return *nullable;
			}
		}
	}
	false
}

fn is_primary_key(modifiers: &[UMLModifier], property: &str) -> bool {
	for modifier in modifiers {
		if let UMLModifier::PirmaryKey(UMLPrimaryKeyModifier { property_id }) = modifier {
			if property_id.eq(property) {
				return true;
			}
		}
	}
	false
}

fn get_type_modifier<'a>(modifiers: &'a [UMLModifier], property: &str) -> Option<&'a str> {
	for modifier in modifiers {
		if let UMLModifier::Type(UMLTypeModifier {
			property_id,
			modifier,
		}) = modifier
		{
			if property_id.eq(property) {
				return Some(modifier);
			}
		}
	}
	None
}

fn get_foreign_key_constraint<'a>(modifiers: &'a [UMLModifier], from_id: &str) -> Option<&'a str> {
	for modifier in modifiers {
		if let UMLModifier::ForeignKey(UMLForeignKeyModifier {
			from_property_id,
			to_property_id,
		}) = modifier
		{
			if from_property_id.eq(from_id) {
				return Some(&to_property_id);
			}
		}
	}
	None
}

fn get_foreign_key(
	modifiers: &[UMLModifier],
	classess: &[&UMLClass],
	property: &str,
) -> Result<Option<(String, String)>> {
	let to_id = get_foreign_key_constraint(modifiers, property);
	if to_id.is_none() {
		return Ok(None);
	}
	let to_id = to_id.unwrap();

	for class in classess {
		for property in &class.properties {
			if property.id.eq(to_id) {
				let property_name = property.name.clone().context("Missing property name")?;
				let class_name = class.name.clone().context("Missing class name")?;
				return Ok(Some((class_name, property_name)));
			}
		}
	}

	Ok(None)
}

fn parse_check_constraint(str: &str) -> SQLCheckConstraint {
	fn try_parse_one_of(str: &str) -> Option<SQLCheckConstraint> {
		let (_, inner) = regex_captures!(r#"^in \((.+)\)$"#, str)?;
		let mut variants = vec![];
		for part in inner.split(", ") {
			let (_, variant) = regex_captures!(r#"^'(.+)'$"#, part)?;
			variants.push(variant.to_string());
		}

		Some(SQLCheckConstraint::OneOf(variants))
	}

	try_parse_one_of(str).unwrap_or(SQLCheckConstraint::Freeform(str.to_string()))
}

// TODO: Refactor this function, less nesting would be good
fn get_sql_check_constraint<'a>(
	models: &'a [UMLModel],
	property_name: &str,
) -> Option<SQLCheckConstraint> {
	for model in models {
		for package in &model.packages {
			for class in &package.classess {
				for constraint in &class.constraints {
					let prop_name = unwrap_opt_continue!(&constraint.property_name);
					let body = unwrap_opt_continue!(&constraint.body);

					if prop_name.eq(property_name) && constraint.body.is_some() {
						return Some(parse_check_constraint(body));
					}
				}
			}
		}
	}
	None
}

fn get_sql_type(
	modifiers: &[UMLModifier],
	type_name: SQLTypeName,
	property: &str,
) -> Result<SQLType> {
	Ok(match type_name {
		SQLTypeName::Int => SQLType::Int,
		SQLTypeName::Date => SQLType::Date,
		SQLTypeName::Datetime => SQLType::Datetime,
		SQLTypeName::Time => SQLType::Time,
		SQLTypeName::Float => SQLType::Float,
		SQLTypeName::Bool => SQLType::Bool,
		SQLTypeName::Decimal => SQLType::Decimal,
		SQLTypeName::Char => {
			if let Some(type_modifier) = get_type_modifier(modifiers, property) {
				let (_, size) = regex_captures!(r#"^\((\d+)\)$"#, type_modifier)
					.context("Type modifier doesn't match format")?;
				SQLType::Char(size.parse()?)
			} else {
				// TODO: Add better error message to say which table is missing type modifier
				// For now just pick a defautl arbitrarily
				SQLType::Char(31)
			}
		}
		SQLTypeName::Varchar => {
			if let Some(type_modifier) = get_type_modifier(modifiers, property) {
				let (_, size) = regex_captures!(r#"^\((\d+)\)$"#, type_modifier)
					.context("Type modifier doesn't match format")?;
				SQLType::Varchar(size.parse()?)
			} else {
				// TODO: Add better error message to say which table is missing type modifier
				// For now just pick a defautl arbitrarily
				SQLType::Varchar(255)
			}
		}
	})
}

fn get_used_types<'a>(models: &'a [UMLModel]) -> HashSet<&'a String> {
	models
		.iter()
		.flat_map(|model| &model.packages)
		.flat_map(|package| &package.classess)
		.flat_map(|class| &class.properties)
		.filter_map(|property| property.type_href.as_ref())
		.collect::<HashSet<_>>()
}

pub fn parse_project<R: Read + Seek>(project_file: R) -> Result<Vec<SQLTableCollection>> {
	let mut zip = ZipArchive::new(project_file).unwrap();

	let (models, modifiers) = parse_uml_model(&mut zip)?;
	let ddl_scripts = parse_ddl_scripts(&mut zip)?;
	let sql_type_names = parse_sql_types(&mut zip, &get_used_types(&models))?;

	let mut collections = vec![];
	for ddl_project in ddl_scripts {
		for ddl_script in ddl_project.scripts {
			let mut tables = vec![];

			let mut model_classess = vec![];
			for ddl_class in &ddl_script.classess {
				let model_class = find_class_by_id(&models, &ddl_class.class_id)
					.context("UML class not found")?;
				model_classess.push(model_class);
			}

			for (ddl_class, model_class) in ddl_script.classess.iter().zip(&model_classess) {
				let name = model_class
					.name
					.clone()
					.context("UML class name not found")?;

				let mut columns = vec![];
				for property_id in &ddl_class.property_ids {
					let property = model_class
						.properties
						.iter()
						.find(|p| p.id.eq(property_id))
						.context("Property not found")?;
					let prop_name = unwrap_opt_continue!(&property.name).clone();

					let type_href = unwrap_opt_continue!(&property.type_href);
					let type_name = sql_type_names
						.get(type_href)
						.context("Property type name conversion not found")?;

					let check_constraint = get_sql_check_constraint(&models, &prop_name);
					let foreign_key = get_foreign_key(&modifiers, &model_classess, property_id)?;

					columns.push(SQLColumn {
						name: prop_name,
						sql_type: get_sql_type(&modifiers, *type_name, property_id)?,
						primary_key: is_primary_key(&modifiers, property_id),
						nullable: is_nullabe(&modifiers, property_id),
						foreign_key,
						check_constraint,
					})
				}

				tables.push(SQLTable { name, columns })
			}
			collections.push(SQLTableCollection { tables })
		}
	}

	Ok(collections)
}
