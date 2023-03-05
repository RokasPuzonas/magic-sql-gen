use anyhow::Result;

use app::App;

mod app;
mod components;
mod generate_sql;
mod magicdraw_parser;

// TODO: Make this work with enumation lookup tables
// TODO: Dark theme switch button
// TODO: Fix double rebuilding when on "trunk server". uno css triggers second build.
// TODO: Add simple versioning in frontend for data

fn main() -> Result<()> {
	yew::Renderer::<App>::new().render();
	Ok(())
}
