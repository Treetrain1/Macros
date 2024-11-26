// Copyright 2023 System76 <info@system76.com>
// SPDX-License-Identifier: MPL-2.0

//! Application API example

mod macros;

use crate::macros::{Instruction, Macro};
use cosmic::app::{Core, Settings, Task};
use cosmic::cosmic_config::{Config, ConfigGet, ConfigSet};
use cosmic::iced::widget::column;
use cosmic::iced_core::Size;
use cosmic::widget::nav_bar;
use cosmic::{executor, iced, ApplicationExt, Element};
use enigo::{Axis, Coordinate, Direction, Enigo, Key, Keyboard, Mouse};
use std::sync::{Arc, Mutex};
use std::thread;
use std::thread::{sleep, JoinHandle};
use cosmic::iced_widget::row;
use enigo::agent::Token;
use tracing::warn;

#[derive(Clone, Copy)]
pub enum Page {
    Page1,
    //Page2,
    //Page3,
    //Page4,
}

impl Page {
    const fn as_str(self) -> &'static str {
        match self {
            Page::Page1 => "Macros",
            //Page::Page2 => "Page 2",
            //Page::Page3 => "Page 3",
            //Page::Page4 => "Page 4",
        }
    }
}

/// Runs application with these settings
#[rustfmt::skip]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    let _ = tracing_log::LogTracer::init();

    let input = vec![
        (Page::Page1, "🌟 Create and manage macros.".into()),
        //(Page::Page1, "🖖 Hello from libcosmic.".into()),
        //(Page::Page2, "🌟 This is an example application.".into()),
        //(Page::Page3, "🚧 The libcosmic API is not stable yet.".into()),
        //(Page::Page4, "🚀 Copy the source code and experiment today!".into()),
    ];

    let settings = Settings::default()
        .size(Size::new(1024., 768.));

    cosmic::app::run::<App>(settings, input)?;

    Ok(())
}

/// Messages that are used specifically by our [`App`].
#[derive(Clone, Debug)]
pub enum Message {
    Input1(String),
    Input2(String),
    Ignore,
    ToggleHide,
    SelectMacro(usize),
    RunMacro(Option<usize>),
}

/// The [`App`] stores application-specific state.
struct App {
    core: Core,
    nav_model: nav_bar::Model,
    input_1: String,
    input_2: String,
    hidden: bool,
    macro_selected: Option<usize>,
    config: Config,
    enigo: Arc<Mutex<Enigo>>,
    thread_pool: ThreadPool,
    macros: Option<Vec<String>>,
}

fn make_enigo() -> Enigo {
    Enigo::new(&enigo::Settings::default()).unwrap()
}

struct ThreadPool {
    workers: Vec<JoinHandle<()>>,
}

impl ThreadPool {
    fn new() -> Self {
        ThreadPool { workers: Vec::new() }
    }

    fn add_worker(&mut self, worker: JoinHandle<()>) {
        self.workers.push(worker);
    }
}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        for worker in self.workers.drain(..) {
            worker.join().expect("TODO: panic message");
        }
    }
}

/// Implement [`cosmic::Application`] to integrate with COSMIC.
impl cosmic::Application for App {
    /// Default async executor to use with the app.
    type Executor = executor::Default;

    /// Argument received [`cosmic::Application::new`].
    type Flags = Vec<(Page, String)>;

    /// Message type specific to our [`App`].
    type Message = Message;

    /// The unique application ID to supply to the window manager.
    const APP_ID: &'static str = "com.treetrain1.Macros";

    fn core(&self) -> &Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    /// Creates the application, and optionally emits command on initialize.
    fn init(core: Core, input: Self::Flags) -> (Self, Task<Self::Message>) {
        let mut nav_model = nav_bar::Model::default();

        for (title, content) in input {
            nav_model.insert().text(title.as_str()).data(content);
        }

        nav_model.activate_position(0);

        let mut app = App {
            core,
            nav_model,
            input_1: String::new(),
            input_2: String::new(),
            hidden: true,
            macro_selected: Some(0),
            config: Config::new(Self::APP_ID, 1).unwrap(),
            enigo: Arc::new(Mutex::from(make_enigo())),
            thread_pool: ThreadPool::new(),
            macros: None,
        };

        let config = &app.config;
        let tx = config.transaction();
        let mut macros = config.get::<Vec<Macro>>("macros");
        if macros.is_err() {
            tx.set("macros", vec![
                Macro::new("macro".into(), "description".into(), vec![
                    Instruction::Wait(1000),
                    Instruction::Token(Token::MoveMouse(100, 100, Coordinate::Rel)),
                    Instruction::Token(Token::Key(Key::Unicode('a'.into()), Direction::Press)),
                    Instruction::Token(Token::Key(Key::Unicode('a'.into()), Direction::Release)),
                    Instruction::Token(Token::Key(Key::Unicode('a'.into()), Direction::Press)),
                    Instruction::Token(Token::Key(Key::Unicode('a'.into()), Direction::Release)),
                    Instruction::Wait(1000),
                    Instruction::Token(Token::Key(Key::Unicode('b'.into()), Direction::Press)),
                    Instruction::Token(Token::Key(Key::Unicode('b'.into()), Direction::Release)),
                    Instruction::Token(Token::Text("Skibidi toilet ohio rizz".into())),
                    Instruction::Wait(500),
                    Instruction::Token(Token::Scroll(4, Axis::Vertical)),
                ]),
                Macro::new("macro2".into(), "description".into(), vec![
                    Instruction::Wait(1000),
                    Instruction::Token(Token::Text("NJOPFPDSFSODPFJODSIFJOPSDPFJ THIS IS FROM A MACRO".into())),
                    Instruction::Wait(500),
                    Instruction::Token(Token::Scroll(4, Axis::Vertical)),
                ]),
                Macro::new("skibidi".into(), "awesome macro".into(), vec![
                    Instruction::Wait(1000),
                    Instruction::Token(Token::Text("Skibidi Skibidi Skibidi Skibidi Skibidi Skibidi Skibidi".into())),
                ]),
            ]).expect("TODO: panic message");
            macros = config.get::<Vec<Macro>>("macros");
        }
        println!("Commit transaction: {:?}", tx.commit());

        let macros = macros.unwrap();
        app.macros = Some(macros.iter().map(|x| x.name.clone()).collect::<Vec<String>>());

        let command = app.update_title();

        (app, command)
    }

    /// Allows COSMIC to integrate with your application's [`nav_bar::Model`].
    fn nav_model(&self) -> Option<&nav_bar::Model> {
        Some(&self.nav_model)
    }

    /// Called when a navigation item is selected.
    fn on_nav_select(&mut self, id: nav_bar::Id) -> Task<Self::Message> {
        self.nav_model.activate(id);
        self.update_title()
    }

    /// Handle application events here.
    fn update(&mut self, message: Self::Message) -> Task<Self::Message> {
        match message {
            Message::Input1(v) => {
                self.input_1 = v;
            }
            Message::Input2(v) => {
                self.input_2 = v;
            }
            Message::Ignore => {}
            Message::ToggleHide => {
                self.hidden = !self.hidden;
            }
            Message::SelectMacro(mac) => {
                self.macro_selected = Some(mac);
            }
            Message::RunMacro(selected) => {
                if selected.is_none() {
                    return Task::none();
                }
                let selected = selected.unwrap();
                let pool = &mut self.thread_pool;
                let thread_num = pool.workers.len();
                let enigo = (&self.enigo).clone();
                let config = self.config.clone();
                let thread = thread::Builder::new().name(format!("macro_thread: {thread_num}")).spawn(move || {
                    println!("Running macro...");
                    let macs = config.get::<Vec<Macro>>("macros").expect("TODO: panic message");
                    let mac = &macs[selected];
                    let mut enigo = enigo.lock().unwrap();
                    for ins in &mac.code {
                        #[allow(unreachable_patterns)] match ins {
                            Instruction::Wait(duration) => {
                                sleep(std::time::Duration::from_millis(*duration));
                            }
                            Instruction::Token(token) => {
                                match token {
                                    Token::Text(text) => {
                                        enigo.text(&text).expect("TODO: panic message");
                                    }
                                    Token::Key(key, direction) => {
                                        enigo.key(*key, *direction).expect("TODO: panic message");
                                    }
                                    Token::Raw(keycode, direction) => {
                                        enigo.raw(*keycode, *direction).expect("TODO: panic message");
                                    }
                                    Token::Button(button, direction) => {
                                        enigo.button(*button, *direction).expect("TODO: panic message");
                                    }
                                    Token::MoveMouse(x, y, coord) => {
                                        enigo.move_mouse(*x, *y, *coord).expect("TODO: panic message");
                                    }
                                    Token::Scroll(amount, axis) => {
                                        enigo.scroll(*amount, *axis).expect("TODO: panic message");
                                    }
                                    _ => {
                                        warn!("Token not implemented.");
                                    }
                                }
                            }
                            _ => {
                                println!("Instruction not implemented.");
                            }
                        }
                    }
                    println!("Macro complete.");
                }).expect("TODO: panic message");
                pool.add_worker(thread);
            }
        }
        Task::none()
    }

    /// Creates a view after each update.
    fn view(&self) -> Element<Self::Message> {
        let page_content = self
            .nav_model
            .active_data::<String>()
            .map_or("No page selected", String::as_str);

        let text = cosmic::widget::text(page_content);

        let mut content = column![
                text,
                cosmic::widget::text_input::text_input("", &self.input_1)
                    .on_input(Message::Input1)
                    .on_clear(Message::Ignore),
                cosmic::widget::text_input::secure_input(
                    "",
                    &self.input_1,
                    Some(Message::ToggleHide),
                    self.hidden
                )
                .on_input(Message::Input1),
                cosmic::widget::text_input::text_input("", &self.input_1).on_input(Message::Input1),
                cosmic::widget::text_input::search_input("", &self.input_2)
                    .on_input(Message::Input2)
                    .on_clear(Message::Ignore),
            ]
                .width(iced::Length::Fill)
                .height(iced::Length::Shrink)
                .align_x(iced::Alignment::Center);

        //content = content.push(cosmic::widget::calendar::calendar(now, |date| Message::Input2(format!("Selected date: {}", date))));
        if let Some(macs) = &self.macros {
            content = content.push(row![
                cosmic::widget::dropdown(macs, Some(0), Message::SelectMacro),

            cosmic::widget::button::text("Run macro")
                .on_press(Message::RunMacro(self.macro_selected.clone()))
            ]);
        }

        let centered = cosmic::widget::container(content)
            .width(iced::Length::Fill)
            .height(iced::Length::Shrink)
            .align_x(iced::Alignment::Center)
            .align_y(iced::Alignment::Center);

        Element::from(centered)
    }
}

impl App
where
    Self: cosmic::Application,
{
    fn active_page_title(&mut self) -> &str {
        self.nav_model
            .text(self.nav_model.active())
            .unwrap_or("Unknown Page")
    }

    fn update_title(&mut self) -> Task<Message> {
        let header_title: String = format!("{} — Macros", self.active_page_title().to_owned());
        let window_title = header_title.clone();
        self.set_header_title(header_title);
        if let Some(id) = self.core.main_window_id() {
            self.set_window_title(window_title, id)
        } else {
            Task::none()
        }
    }
}