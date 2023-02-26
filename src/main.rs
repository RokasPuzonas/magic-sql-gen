use magicdraw_parser::parse_project;
use yew::prelude::*;
use std::fs::File;
use anyhow::Result;

mod magicdraw_parser;

// TODO: Make this work with enumation lookup tables

#[function_component]
fn App() -> Html {
	html! {
		<main>
				<img class="logo" src="https://yew.rs/img/logo.png" alt="Yew logo" />
				<h1>{ "Hello World!" }</h1>
				<span class="subtitle">{ "from Yew with " }<i class="heart" /></span>
		</main>
	}
}

fn main() -> Result<()> {
	let f = File::open("example.mdzip").unwrap();

	let collections = parse_project(f)?;
	dbg!(collections);

	// yew::Renderer::<App>::new().render();
	Ok(())
}
