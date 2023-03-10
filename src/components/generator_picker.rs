use std::str::FromStr;

use web_sys::{Event, HtmlInputElement};
use yew::{html, AttrValue, Callback, Html, TargetCast};

use crate::{
	generate_sql::{
		SQLBoolValueGuess, SQLIntValueGuess, SQLStringValueGuess, SQLTimeValueGuess, SQLValueGuess,
	},
	magicdraw_parser::{SQLCheckConstraint, SQLColumn},
};

fn show_dropdown_picker(selected: &str, options: &[AttrValue], onchange: Callback<String>) -> Html {
	html! {
		<select onchange={onchange.reform(move |e: Event| {
			let value = e.target_unchecked_into::<HtmlInputElement>().value();
			value
		})}>
			{
				options.iter().map(|value| {
					html! { <option selected={value.eq(&selected)} value={value.clone()}>{ value }</option> }
				}).collect::<Html>()
			}
		</select>
	}
}

fn show_enum_dropdown< T: PartialEq + Clone + 'static>(
	selected: &T,
	options: &Vec<(AttrValue, T)>,
	onchange: Callback<T>,
) -> Html {
	let keys = options.iter().map(|(opt, _)| opt.clone()).collect::<Vec<_>>();
	let guess_str = options
		.iter()
		.find(|(_, v)| v.eq(&selected))
		.unwrap()
		.0
		.clone();

	let options = options.clone();
	show_dropdown_picker(
		&guess_str,
		&keys,
		onchange.reform(move |value_str: String| {
			let enum_value = &options.iter()
				.find(|(v, _)| v.eq(&value_str))
				.unwrap()
				.1;
			enum_value.clone()
		})
	)
}

fn show_range_picker<T: FromStr + ToString + Clone + 'static>(
	min: T,
	max: T,
	default_min: T,
	default_max: T,
	onchange: Callback<(T, T)>,
) -> Html {
	let onchange_min = {
		let onchange = onchange.clone();
		let default_min = default_min.clone();
		let max = max.clone();
		Callback::from(move |e: Event| {
			let value = e.target_unchecked_into::<HtmlInputElement>().value();
			let min_value = value.parse().unwrap_or(default_min.clone());
			onchange.emit((min_value, max.clone()))
		})
	};

	let onchange_max = {
		let onchange = onchange.clone();
		let default_max = default_max.clone();
		let min = min.clone();
		Callback::from(move |e: Event| {
			let value = e.target_unchecked_into::<HtmlInputElement>().value();
			let max_value = value.parse().unwrap_or(default_max.clone());
			onchange.emit((min.clone(), max_value))
		})
	};

	html! {
		<div class="flex flex-row">
			<input
				value={min.to_string()}
				class="w-5rem"
				type="number"
				placeholder={default_min.to_string()}
				onchange={onchange_min}
			/>
			<div class="ml-1 mr-1">{ ".." }</div>
			<input
				value={max.to_string()}
				class="w-5rem"
				type="number"
				placeholder={default_max.to_string()}
				onchange={onchange_max}
			/>
		</div>
	}
}

pub fn generator_picker(
	column: &SQLColumn,
	value: &SQLValueGuess,
	onchange: Callback<SQLValueGuess>,
) -> Html {
	// TODO: Refacotr 'time', 'datetime', and 'date'. They are very similar
	match value {
		SQLValueGuess::Int(guess) => {
			if column.primary_key {
				return html!("Auto increment");
			}

			let mut min = 0;
			let mut max = 0;
			if let SQLIntValueGuess::Range(range_min, range_max) = guess {
				min = *range_min;
				max = *range_max;
			}

			// TODO: Disallow entering floating point numbers
			show_range_picker(
				min,
				max,
				0,
				100,
				onchange.reform(|(min, max)| SQLValueGuess::Int(SQLIntValueGuess::Range(min, max))),
			)
		}
		SQLValueGuess::Float(min, max) => show_range_picker(
			*min,
			*max,
			0.0,
			100.0,
			onchange.reform(|(min, max)| SQLValueGuess::Float(min, max)),
		),
		SQLValueGuess::Date(guess) => {
			let options = vec![
				("Now".into(), SQLTimeValueGuess::Now),
				("Future".into(), SQLTimeValueGuess::Future),
				("Past".into(), SQLTimeValueGuess::Past),
			];

			show_enum_dropdown(
				guess,
				&options,
				onchange.reform(|enum_value| SQLValueGuess::Date(enum_value)),
			)
		}
		SQLValueGuess::Time(guess) => {
			let options = vec![
				("Now".into(), SQLTimeValueGuess::Now),
				("Future".into(), SQLTimeValueGuess::Future),
				("Past".into(), SQLTimeValueGuess::Past),
			];

			show_enum_dropdown(
				guess,
				&options,
				onchange.reform(|enum_value| SQLValueGuess::Time(enum_value)),
			)
		}
		SQLValueGuess::Datetime(guess) => {
			let options = vec![
				("Now".into(), SQLTimeValueGuess::Now),
				("Future".into(), SQLTimeValueGuess::Future),
				("Past".into(), SQLTimeValueGuess::Past),
			];

			show_enum_dropdown(
				guess,
				&options,
				onchange.reform(|enum_value| SQLValueGuess::Datetime(enum_value)),
			)
		}
		SQLValueGuess::Bool(guess) => {
			let options = vec![
				("Random".into(), SQLBoolValueGuess::Random),
				("True".into(), SQLBoolValueGuess::True),
				("False".into(), SQLBoolValueGuess::False),
			];

			show_enum_dropdown(
				guess,
				&options,
				onchange.reform(|enum_value| SQLValueGuess::Bool(enum_value)),
			)
		}
		SQLValueGuess::String(max_size, guess) => {
			if let Some(constraint) = &column.check_constraint {
				if let SQLCheckConstraint::OneOf(_) = constraint {
					return html!("Random Enum");
				}
			}

			let options = vec![
				("Lorem Ipsum".into(), SQLStringValueGuess::LoremIpsum),
				("Empty".into(), SQLStringValueGuess::Empty),
				("First Name".into(), SQLStringValueGuess::FirstName),
				("Last Name".into(), SQLStringValueGuess::LastName),
				("Full Name".into(), SQLStringValueGuess::FullName),
				("Phone number".into(), SQLStringValueGuess::PhoneNumber),
				("City name".into(), SQLStringValueGuess::CityName),
				("Address".into(), SQLStringValueGuess::Address),
				("Email".into(), SQLStringValueGuess::Email),
				("URL".into(), SQLStringValueGuess::URL),
			];

			let max_size = *max_size;
			show_enum_dropdown(
				guess,
				&options,
				onchange.reform(move |enum_value| SQLValueGuess::String(max_size, enum_value)),
			)
		}
	}
}
