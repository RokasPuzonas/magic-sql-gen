use std::rc::Rc;

use yew::{Properties, html, function_component, Html};

use crate::magicdraw_parser::SQLTable;

#[derive(Properties, PartialEq)]
pub struct SQLTableColumnInfoProps {
	pub table: Rc<SQLTable>
}

const CHECK_MARK: &str = "✔️";
const CROSS_MARK: &str = "❌";

fn bool_to_mark(value: bool) -> &'static str {
	if value { CHECK_MARK } else { CROSS_MARK }
}

#[function_component]
pub fn SQLTableColumnInfo(props: &SQLTableColumnInfoProps) -> Html {
	let table = &props.table;

	let rows = table.columns.iter()
		.map(|col| {
				 let foreign_key;
					if let Some((table_name, prop_name)) = &col.foreign_key {
						foreign_key = format!("{} {}", table_name, prop_name);
					} else {
						foreign_key = CROSS_MARK.into();
					}

					html! {
						<tr>
							<td> { &col.name } </td>
							<td> { &col.sql_type } </td>
							<td> { bool_to_mark(col.primary_key) } </td>
							<td> { bool_to_mark(col.nullable) } </td>
							<td> { foreign_key } </td>
						</tr>
					}
				}
			);

	html!{
		<div
			class="table-column-info flex-column inline-block"
			border="solid dark100 0.2rem collapse"
		>
			<p class="text-center"> { &table.name } </p>
			<table
				border="solid dark100 t-0.2rem collapse"
			>
				<tr>
					<th> { "Column" } </th>
					<th> { "Type" } </th>
					<th> { "Primary?" } </th>
					<th> { "Nullable?" } </th>
					<th> { "Foreign key?" } </th>
				</tr>
				{ for rows }
			</table>
		</div>
	}
}
