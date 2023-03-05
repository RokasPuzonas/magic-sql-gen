use gloo::file::callbacks::FileReader;
use gloo::file::File;
use gloo::storage::{LocalStorage, Storage};
use std::cell::RefCell;
use std::collections::HashMap;
use std::io::Cursor;
use std::rc::Rc;
use web_sys::{DragEvent, Event, FileList, HtmlInputElement, MouseEvent};
use yew::html::TargetCast;
use yew::{html, Callback, Component, Context, Html};

use crate::components::sql_column_info::SQLTableColumnInfo;
use crate::generate_sql::{generate_fake_entries, generate_table_guessess, SQLValueGuess};
use crate::magicdraw_parser::{parse_project, SQLTable, SQLTableCollection};

const COLLECTION_STORE_KEY: &str = "current_collection";
const DEFAULT_ROWS_PER_TABLE: u32 = 20;

pub enum Msg {
	Noop,
	Loaded(String, Vec<u8>),
	UploadProject(File),
	UpdateCurrentProject(Option<SQLTableCollection>),
	UpdateGenarator(String, SQLValueGuess),
	ShowNextTable,
	ShowPrevTable,
	AllGoodConfirmation,
	GenerateSQL,
	UpdateRowsPerTable(u32),
}

pub struct App {
	active_readers: HashMap<String, FileReader>,
	current_collection: Option<Vec<Rc<SQLTable>>>,
	current_guessess: Vec<Rc<RefCell<HashMap<String, SQLValueGuess>>>>,
	currently_shown_table: usize,
	all_good_confirmed: bool,
	generated_sql: Option<String>,
	rows_per_table: u32,
}

impl Component for App {
	type Message = Msg;
	type Properties = ();

	fn create(_ctx: &Context<Self>) -> Self {
		let mut current_guessess = vec![];
		let mut current_collection = None;
		if let Ok(collection) = LocalStorage::get::<SQLTableCollection>("current_collection") {
			for table in &collection.tables {
				let guess = generate_table_guessess(table);
				current_guessess.push(Rc::new(RefCell::new(guess)));
			}

			current_collection = Some(collection.tables.into_iter().map(Rc::new).collect());
		}

		Self {
			active_readers: HashMap::default(),
			current_collection,
			currently_shown_table: 0,
			all_good_confirmed: true, // TODO: make this false, by default
			generated_sql: None,
			current_guessess,
			rows_per_table: DEFAULT_ROWS_PER_TABLE,
		}
	}

	fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
		match msg {
			Msg::Loaded(file_name, data) => {
				if file_name.ends_with(".mdzip") {
					let cursor = Cursor::new(&data);

					let mut collections = parse_project(cursor).expect("oops");
					if collections.len() >= 1 {
						let msg = Self::update_current_collection(Some(collections.remove(0)));
						ctx.link().send_message(msg);
					}
					// TODO: show error message
				}

				self.active_readers.remove(&file_name);
				true
			}
			Msg::UploadProject(file) => {
				let file_name = file.name();

				let task = {
					let link = ctx.link().clone();
					let file_name = file_name.clone();

					gloo::file::callbacks::read_as_bytes(&file, move |res| {
						// TODO: show error message
						link.send_message(Msg::Loaded(file_name, res.expect("failed to read file")))
					})
				};

				self.active_readers.insert(file_name, task);
				true
			}
			Msg::Noop => false,
			Msg::UpdateCurrentProject(collection) => {
				if let Some(collection) = collection {
					LocalStorage::set(COLLECTION_STORE_KEY, &collection).unwrap();
					self.currently_shown_table = 0;
					self.all_good_confirmed = false;
					self.generated_sql = None;
					self.current_guessess = vec![];
					for table in &collection.tables {
						let guess = generate_table_guessess(table);
						self.current_guessess.push(Rc::new(RefCell::new(guess)));
					}
					self.current_collection =
						Some(collection.tables.into_iter().map(Rc::new).collect());
				} else {
					LocalStorage::delete(COLLECTION_STORE_KEY);
					self.current_collection = None
				}

				true
			}
			Msg::ShowNextTable => {
				if let Some(collection) = &self.current_collection {
					self.currently_shown_table =
						(self.currently_shown_table + 1).min(collection.len() - 1);
					return true;
				}
				false
			}
			Msg::ShowPrevTable => {
				if self.currently_shown_table > 0 {
					self.currently_shown_table = self.currently_shown_table - 1;
					return true;
				}
				false
			}
			Msg::AllGoodConfirmation => {
				self.all_good_confirmed = true;
				true
			}
			Msg::UpdateGenarator(column, generator) => {
				let mut guessess = self.current_guessess[self.currently_shown_table].borrow_mut();
				let entry = guessess.get_mut(&column).unwrap();
				*entry = generator;
				true
			}
			Msg::GenerateSQL => {
				let tables = self.current_collection.as_ref().unwrap();
				let guessess = self.current_guessess.iter().map(|v| v.borrow()).collect();
				// TODO: show error message
				if let Ok(result) = generate_fake_entries(tables, &guessess, self.rows_per_table) {
					self.generated_sql = Some(result)
				} else {
					self.generated_sql = None
				}
				true
			}
			Msg::UpdateRowsPerTable(rows_per_table) => {
				self.rows_per_table = rows_per_table;
				false
			}
		}
	}

	fn view(&self, ctx: &Context<Self>) -> Html {
		html! {
			<main class="flex-col 4rem center">
				<p class="text-3xl text-center">{ "🪄 MagicDraw SQL Data Generator" }</p>
				{ self.show_step1(ctx) }
				if self.current_collection.is_some() {
					{ self.show_step2(ctx) }
					if self.all_good_confirmed {
						{ self.show_step3(ctx) }
						if self.generated_sql.is_some() {
							{ self.show_step4(ctx) }
						}
					}
				}
			</main>
		}
	}
}

impl App {
	fn show_step1(&self, ctx: &Context<Self>) -> Html {
		let prevent_default_cb = Callback::from(|event: DragEvent| {
			event.prevent_default();
		});

		html! {
			<div>
				<p class="text-2xl mt-2rem pb-1rem">
					<span>{ "1. Upload " }</span>
					<code class="bg-dark900 p-0.2rem rounded">{".mdzip"}</code>
					<span>{ " project" }</span>
				</p>
				<label for="file-upload">
					<div
						class="flex flex-col rounded items-center p-3rem bg-dark800"
						border="dotted dark100 0.2rem"
						cursor="pointer"
						ondrop={ctx.link().callback(|event: DragEvent| {
							event.prevent_default();
							let files = event.data_transfer().unwrap().files();
							Self::upload_project(files)
						})}
						ondragover={&prevent_default_cb}
						ondragenter={&prevent_default_cb}
					>
						<div class="i-mdi-file-upload-outline text-4rem"></div>
					</div>
				</label>
				<input
					id="file-upload"
					type="file"
					class = "hidden"
					accept=".mdzip"
					onchange={ctx.link().callback(move |e: Event| {
						let input: HtmlInputElement = e.target_unchecked_into();
						Self::upload_project(input.files())
					})}
				/>
				<p class="text-amber300">{ "NOTE: This relies on the fact, that you have a .dll script configured" }</p>
			</div>
		}
	}

	fn show_step2(&self, ctx: &Context<Self>) -> Html {
		let collection = self.current_collection.as_ref().unwrap();

		html! {
			<div>
				<p class="text-2xl mt-2rem">{ "2. Make sure everything looks 👌" }</p>
				<div class="mb-0.5rem gap-3 flex flex-row items-center">
					<button
						class="p-0.5rem btn-white"
						onclick={ctx.link().callback(move |_: MouseEvent| { Msg::ShowPrevTable })}
					>
						{ "< Previous" }
					</button>
					<div> { self.currently_shown_table + 1 } { " / " } { collection.len() } </div>
					<button
						class="p-0.5rem btn-white"
						onclick={ctx.link().callback(move |_: MouseEvent| { Msg::ShowNextTable })}
					>
						{ "Next >" }
					</button>
				</div>
				<SQLTableColumnInfo
					table={collection[self.currently_shown_table].clone()}
					guessess={self.current_guessess[self.currently_shown_table].clone()}
					onchange={ctx.link().callback(|(column_name, generator)| {
						Msg::UpdateGenarator(column_name, generator)
					})}
				/>
				<button
					class="display-block p-1rem  mt-1rem btn-emerald"
					onclick={ctx.link().callback(move |_: MouseEvent| { Msg::AllGoodConfirmation })}
				>{ "All good?" }</button>
			</div>
		}
	}

	fn show_step3(&self, ctx: &Context<Self>) -> Html {
		let on_rows_changed = ctx.link().callback(|e: Event| {
			let value_str = e.target_unchecked_into::<HtmlInputElement>().value();
			let value = value_str.parse().unwrap_or(DEFAULT_ROWS_PER_TABLE);
			Msg::UpdateRowsPerTable(value)
		});

		html! {
			<div>
				<p class="text-2xl mt-2rem">{ "3. Final settings" }</p>
				<label for="gen-amount-input">
					{ "Entries per table: " }
				</label>
				<input
					id="gen-amount-input"
					class="rounded items-center p-0.3rem bg-dark800 text-light100 w-5rem b-0"
					value={self.rows_per_table.to_string()}
					type="number"
					onchange={on_rows_changed}
				/>

				<button
					class="block mt-1rem p-1rem btn-emerald"
					onclick={ctx.link().callback(|_: MouseEvent| { Msg::GenerateSQL })}
				>
					{ "Generate" }
				</button>
			</div>
		}
	}

	fn show_step4(&self, ctx: &Context<Self>) -> Html {
		let sql = self.generated_sql.as_ref().unwrap();
		html! {
			<div>
				<p class="text-2xl mt-2rem">{ "4. Copy & Paste" }</p>
				<pre class="bg-dark900 p-0.5rem rounded">
					{ sql }
				</pre>
			</div>
		}
	}

	fn upload_project(files: Option<FileList>) -> Msg {
		if let Some(files) = files {
			let file = js_sys::try_iter(&files)
				.unwrap()
				.unwrap()
				.next()
				.map(|v| web_sys::File::from(v.unwrap()))
				.map(File::from)
				.unwrap();
			Msg::UploadProject(file)
		} else {
			Msg::Noop
		}
	}

	pub fn update_current_collection(current_collection: Option<SQLTableCollection>) -> Msg {
		Msg::UpdateCurrentProject(current_collection)
	}
}
