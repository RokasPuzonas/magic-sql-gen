use std::io::Cursor;
use std::collections::{HashMap, self};
use std::rc::Rc;
use gloo::console::console_dbg;
use gloo::file::callbacks::FileReader;
use gloo::file::File;
use gloo::storage::{LocalStorage, Storage};
use web_sys::{DragEvent, Event, FileList, HtmlInputElement, MouseEvent};
use yew::html::TargetCast;
use yew::{html, Callback, Component, Context, Html};

use crate::magicdraw_parser::{parse_project, SQLTableCollection, SQLTable};
use crate::components::sql_column_info::SQLTableColumnInfo;

const COLLECTION_STORE_KEY: &str = "current_collection";

pub enum Msg {
	Noop,
	Loaded(String, Vec<u8>),
	UploadProject(File),
	UpdateCurrentProject(Option<SQLTableCollection>),
	ShowNextTable,
	ShowPrevTable
}

pub struct App {
	active_readers: HashMap<String, FileReader>,
	current_collection: Option<Vec<Rc<SQLTable>>>,
	currently_shown_table: usize
}

impl Component for App {
	type Message = Msg;
	type Properties = ();

	fn create(_ctx: &Context<Self>) -> Self {
		let mut current_collection = None;
		if let Ok(collection) = LocalStorage::get::<SQLTableCollection>("current_collection") {
			current_collection = Some(collection.tables.into_iter().map(Rc::new).collect());
		}

		Self {
			active_readers: HashMap::default(),
			current_collection,
			currently_shown_table: 0
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
						link.send_message(Msg::Loaded(
							file_name,
							res.expect("failed to read file"),
						))
					})
				};

				self.active_readers.insert(file_name, task);
				true
			},
			Msg::Noop => false,
			Msg::UpdateCurrentProject(collection) => {
				if let Some(collection) = collection {
					LocalStorage::set(COLLECTION_STORE_KEY, &collection).unwrap();
					self.current_collection = Some(collection.tables.into_iter().map(Rc::new).collect());
				} else {
					LocalStorage::delete(COLLECTION_STORE_KEY);
					self.current_collection = None
				}

				true
			},
			Msg::ShowNextTable => {
				if let Some(collection) = &self.current_collection {
					self.currently_shown_table = (self.currently_shown_table + 1).min(collection.len()-1);
					return true;
				}
				false
			},
			Msg::ShowPrevTable => {
				if self.currently_shown_table > 0 {
					self.currently_shown_table = self.currently_shown_table - 1;
					return true;
				}
				false
			},
		}
	}

	fn view(&self, ctx: &Context<Self>) -> Html {
		let prevent_default_cb = Callback::from(|event: DragEvent| {
			event.prevent_default();
		});

		html! {
			<main class="flex-col 4rem center">
				<p class="text-3xl text-center">{ "🪄 MagicDraw SQL Data Generator" }</p>
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
				if let Some(collection) = &self.current_collection {
					<div>
						<p class="text-2xl mt-2rem">{ "2. Make sure everything looks 👌" }</p>
						<div class="mb-0.5rem gap-3 flex flex-row items-center">
							<button
								class="p-0.5rem btn-white"
								onclick={ctx.link().callback(move |_: MouseEvent| {
									Msg::ShowPrevTable
								})}
							>
								{ "< Previous" }
							</button>
							<div> { self.currently_shown_table + 1 } { " / " } { collection.len() } </div>
							<button
								class="p-0.5rem btn-white"
								onclick={ctx.link().callback(move |_: MouseEvent| {
									Msg::ShowNextTable
								})}
							>
								{ "Next >" }
							</button>
						</div>
						{ Self::show_table(collection[self.currently_shown_table].clone()) }
						<button class="display-block p-1rem  mt-1rem btn-emerald">{ "All good?" }</button>
					</div>
					<div>
						<p class="text-2xl mt-2rem">{ "3. Copy & Paste" }</p>
					</div>
				}
			</main>
		}
	}
}

impl App {

	fn show_table(table: Rc<SQLTable>) -> Html {
		html!{
			<SQLTableColumnInfo table={table} />
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
