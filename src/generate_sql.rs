use std::{rc::Rc, collections::HashSet};

use anyhow::{Result, bail};
use rand::{seq::SliceRandom, Rng, rngs::ThreadRng};
use chrono::{Local, NaiveDateTime, Days};
use fake::{faker::{lorem::en::*, name::en::{FirstName, LastName, Name}, phone_number::en::PhoneNumber, internet::en::{DomainSuffix, FreeEmail}, company::en::BsNoun, address::{en::{CityName, StreetName}}}, Fake};

use crate::magicdraw_parser::{SQLTable, SQLColumn, SQLType, SQLCheckConstraint};

const INDENT: &str = "  ";

#[derive(Debug, PartialEq)]
pub enum SQLIntValueGuess {
	Range(i32, i32),
	AutoIncrement
}

#[derive(Debug, PartialEq)]
pub enum SQLTimeValueGuess {
	Now,
	Future,
	Past
}

#[derive(Debug, PartialEq)]
pub enum SQLStringValueGuess {
	LoremIpsum,
	FirstName,
	LastName,
	FullName,
	Empty,
	PhoneNumber,
	CityName,
	Address,
	Email,
	URL,
	RandomEnum(Vec<String>),
}

#[derive(Debug, PartialEq)]
pub enum SQLBoolValueGuess {
	True,
	False,
	Random,
}

#[derive(Debug, PartialEq)]
pub enum SQLValueGuess {
	Int(SQLIntValueGuess),
	Date(SQLTimeValueGuess),
	Time(SQLTimeValueGuess),
	Datetime(SQLTimeValueGuess),
	Float(f32, f32),
	Bool(SQLBoolValueGuess),
	String(usize, SQLStringValueGuess),
}

// TODO: Check primary key constraint
pub fn generate_fake_entries(
		tables: &[Rc<SQLTable>],
		value_guessess: &Vec<Vec<SQLValueGuess>>,
		rows_per_table: u32
	) -> Result<String> {
	let mut lines = vec![];

	let mut rng = rand::thread_rng();

	let mut all_foreign_columns = vec![];
	let mut all_entries = vec![];
	for table in tables {
		let mut entries = vec![];
		for _ in 0..rows_per_table {
			entries.push(vec![]);
		}
		all_entries.push(entries);

		let mut foreign_columns = vec![];
		for (i, column) in table.columns.iter().enumerate() {
			if let Some((table_name, column_name)) = &column.foreign_key {
				let (table_idx, table) = tables.iter()
					.enumerate()
					.find(|(_, table)| table.name.eq(table_name))
					.expect("Foreign table not found");
				let (column_idx, _) = table.columns
					.iter()
					.enumerate()
					.find(|(_, column)| column.name.eq(column_name))
					.expect("Foreign column not found");
				foreign_columns.push((i, table_idx, column_idx));
			}
		}
		all_foreign_columns.push(foreign_columns);
	}

	let mut entries_with_foreign_keys = HashSet::new();
	for (table_idx, table) in tables.iter().enumerate() {
		let entries = &mut all_entries[table_idx];

		for (column_idx, column) in table.columns.iter().enumerate() {
			let mut auto_increment_counter = 0;
			let value_guess = &value_guessess[table_idx][column_idx];
			for entry_idx in 0..(rows_per_table as usize) {
				if let Some(_) = &column.foreign_key {
					entries_with_foreign_keys.insert((table_idx, entry_idx));
					entries[entry_idx].push("".into());
				} else {
					entries[entry_idx].push(generate_value(&mut rng, &value_guess, &mut auto_increment_counter));
				}
			}
		}
	}

	while !entries_with_foreign_keys.is_empty() {
		let entries_with_foreign_keys_copy = entries_with_foreign_keys.clone();
		let before_retain = entries_with_foreign_keys.len();

		entries_with_foreign_keys.retain(|(table_idx, entry_idx)| {
			for (column_idx, foreign_table_idx, foreign_column_idx) in &all_foreign_columns[*table_idx] {
				let available_values: Vec<&str>;

				// If the foreign column, is also a foreign of the other table, ...
				// Then we need to filter out available options which have not been filled in
				if all_foreign_columns[*foreign_table_idx].iter().find(|(idx, _, _)| idx == foreign_column_idx).is_some() {
					available_values = all_entries[*foreign_table_idx].iter()
						.enumerate()
						.filter(|(i, _)| entries_with_foreign_keys_copy.contains(&(*foreign_table_idx, *i)))
						.map(|(_, entry)| entry[*foreign_column_idx].as_str())
						.collect();
				} else {
					available_values = all_entries[*foreign_table_idx].iter()
						.map(|entry| entry[*foreign_column_idx].as_str())
						.collect();
				}

				if let Some(chosen_value) = available_values.choose(&mut rng) {
					all_entries[*table_idx][*entry_idx][*column_idx] = chosen_value.to_string();
				} else {
					// Early break, thre are no currently available options
					// Try next time
					return true;
				}
			}

			false
		});

		// This is to stop infnite loop, where during each iteration nothing gets removed
		if before_retain == entries_with_foreign_keys.len() {
			bail!("Failed to resolve foreign keys")
		}
	}

	for (i, table) in tables.iter().enumerate() {
		let mut column_names = vec![];
		for column in &table.columns {
			column_names.push(column.name.as_str());
		}

		let entries = &all_entries[i];
		lines.push(format!("INSERT INTO {}", table.name));
		lines.push(format!("{}({})", INDENT, column_names.join(", ")));
		lines.push("VALUES".into());
		let entries_str = entries.iter()
			.map(|entry| format!("{}({})", INDENT, entry.join(", ")))
			.collect::<Vec<_>>()
			.join(",\n");
		lines.push(format!("{};\n", entries_str));
	}

	Ok(lines.join("\n"))
}

fn generate_time_value(rng: &mut ThreadRng, guess: &SQLTimeValueGuess) -> NaiveDateTime {
	let now = Local::now().naive_local();

	match guess {
		SQLTimeValueGuess::Now => now,
		SQLTimeValueGuess::Future => {
			let days = rng.gen_range(1..=30);
			now.checked_add_days(Days::new(days)).unwrap()
		},
		SQLTimeValueGuess::Past => {
			let days = rng.gen_range(7..=365);
			now.checked_sub_days(Days::new(days)).unwrap()
		}
	}
}

fn generate_value(rng: &mut ThreadRng, guess: &SQLValueGuess, auto_increment_counter: &mut u32) -> String {
	match guess {
    SQLValueGuess::Int(int_guess) => {
			match int_guess {
				SQLIntValueGuess::Range(min, max) => {
					rng.gen_range((*min)..=(*max)).to_string()
				},
				SQLIntValueGuess::AutoIncrement => {
					let str = auto_increment_counter.to_string();
					*auto_increment_counter += 1;
					str
				},
			}
		},
    SQLValueGuess::Date(time_gues) => {
			let datetime = generate_time_value(rng, &time_gues);
			format!("'{}'", datetime.format("%Y-%m-%d"))
		},
    SQLValueGuess::Time(time_gues) => {
			let datetime = generate_time_value(rng, &time_gues);
			format!("'{}'", datetime.format("%H:%M:%S"))
		},
    SQLValueGuess::Datetime(time_gues) => {
			let datetime = generate_time_value(rng, &time_gues);
			format!("'{}'", datetime.format("%Y-%m-%d %H:%M:%S"))
		},
    SQLValueGuess::Bool(bool_guess) => {
			match bool_guess {
				SQLBoolValueGuess::True => "1".into(),
				SQLBoolValueGuess::False => "0".into(),
				SQLBoolValueGuess::Random => rng.gen_range(0..=1).to_string(),
			}
		},
    SQLValueGuess::Float(min, max) => {
			let value = rng.gen_range((*min)..(*max));
			((value * 100.0 as f32).round() / 100.0).to_string()
		},
    SQLValueGuess::String(max_size, string_guess) => {
			let mut str = match string_guess {
				SQLStringValueGuess::LoremIpsum => {

					let mut current_len = 0;
					let mut text = vec![];
					let words: Vec<String> = Words(3..10).fake_with_rng(rng);
					for word in words {
						current_len += word.len() + 1;
						text.push(word);
						if current_len > *max_size { break; }
					}
					text.join(" ").to_string()
				},
				SQLStringValueGuess::FirstName => {
					FirstName().fake_with_rng(rng)
				},
				SQLStringValueGuess::LastName => {
					LastName().fake_with_rng(rng)
				},
				SQLStringValueGuess::FullName => {
					Name().fake_with_rng(rng)
				},
				SQLStringValueGuess::PhoneNumber => {
					PhoneNumber().fake_with_rng(rng)
				},
				SQLStringValueGuess::CityName => {
					CityName().fake_with_rng(rng)
				},
				SQLStringValueGuess::Address => {
					StreetName().fake_with_rng(rng)
				},
				SQLStringValueGuess::Email => {
					FreeEmail().fake_with_rng(rng)
				},
				SQLStringValueGuess::URL => {
					let suffix: String = DomainSuffix().fake_with_rng(rng);
					let noun: String = BsNoun().fake_with_rng(rng);
					let noun: String = noun.to_lowercase()
						.chars()
						.map(|c| if c.is_whitespace() { '-' } else { c })
						.collect();
					format!("www.{}.{}", noun, suffix)
				},
				SQLStringValueGuess::RandomEnum(options) => {
					options.choose(rng).unwrap().to_string()
				},
				SQLStringValueGuess::Empty => {
					"".into()
				}
			};

			str.truncate(*max_size);
			format!("'{}'", str)
		}
	}
}

fn generate_string_guess(column: &SQLColumn) -> SQLStringValueGuess {
	if let Some(constraint) = &column.check_constraint {
		if let SQLCheckConstraint::OneOf(options) = constraint {
			return SQLStringValueGuess::RandomEnum(options.clone())
		} else {
			return SQLStringValueGuess::LoremIpsum
		}
	}

	let name = column.name.to_lowercase();
	if name.contains("first") && name.contains("name") {
		 SQLStringValueGuess::FirstName
	} else if (name.contains("last") && name.contains("name")) || name.contains("surname") {
		 SQLStringValueGuess::LastName
	} else if name.contains("phone") && name.contains("number") {
		 SQLStringValueGuess::PhoneNumber
	} else if name.contains("city") {
		 SQLStringValueGuess::CityName
	} else if name.contains("address") {
		 SQLStringValueGuess::Address
	} else if name.contains("email") {
		 SQLStringValueGuess::Email
	} else if name.contains("homepage") || name.contains("website") || name.contains("url") {
		 SQLStringValueGuess::URL
	} else {
		 SQLStringValueGuess::LoremIpsum
	}
}

pub fn generate_guess(column: &SQLColumn) -> SQLValueGuess {
	match column.sql_type {
    SQLType::Int => {
			if column.primary_key {
				SQLValueGuess::Int(SQLIntValueGuess::AutoIncrement)
			} else {
				SQLValueGuess::Int(SQLIntValueGuess::Range(0, 100))
			}
		},
    SQLType::Float | SQLType::Decimal => {
			SQLValueGuess::Float(0.0, 100.0)
		},
    SQLType::Date => {
			let name = column.name.to_lowercase();
			if name.contains("create") || name.contains("update") {
				SQLValueGuess::Date(SQLTimeValueGuess::Past)
			} else {
				SQLValueGuess::Date(SQLTimeValueGuess::Now)
			}
		},
    SQLType::Time => {
			let name = column.name.to_lowercase();
			if name.contains("create") || name.contains("update") {
				SQLValueGuess::Time(SQLTimeValueGuess::Past)
			} else {
				SQLValueGuess::Time(SQLTimeValueGuess::Now)
			}
		},
    SQLType::Datetime => {
			let name = column.name.to_lowercase();
			if name.contains("create") || name.contains("update") {
				SQLValueGuess::Datetime(SQLTimeValueGuess::Past)
			} else {
				SQLValueGuess::Datetime(SQLTimeValueGuess::Now)
			}
		},
    SQLType::Bool => {
			SQLValueGuess::Bool(SQLBoolValueGuess::Random)
		},
		SQLType::Varchar(max_size) => {
			SQLValueGuess::String(max_size as usize, generate_string_guess(column))
		},
		SQLType::Char(max_size) => {
			SQLValueGuess::String(max_size as usize, generate_string_guess(column))
		}
	}
}

pub fn generate_table_guessess(table: &SQLTable) -> Vec<SQLValueGuess> {
	table.columns.iter()
		.map(|column| generate_guess(column))
		.collect()
}
