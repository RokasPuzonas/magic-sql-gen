use app::App;

mod magicdraw_parser;
mod app;
mod components;

// TODO: Make this work with enumation lookup tables
// TODO: Dark theme switch button
// TODO: Fix double rebuilding when on "trunk server". uno css triggers second build.
// TODO: Add simple versioning in frontend for data

fn main() {
	// let f = File::open("example.mdzip").unwrap();
	// let collections = parse_project(f)?;
	// dbg!(collections);

	yew::Renderer::<App>::new().render();
}
