// Copyright 2023 System76 <info@system76.com>
// SPDX-License-Identifier: MPL-2.0

//! Application API example

use chrono::{Local, NaiveDate};
use cosmic::app::{Core, Settings, Task};
use cosmic::iced::widget::column;
use cosmic::iced_core::Size;
use cosmic::widget::nav_bar;
use cosmic::{executor, iced, ApplicationExt, Element};
use cosmic::cosmic_config::{Config, ConfigSet};
use enigo::{Coordinate, Enigo, Keyboard, Mouse};

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

    println!("bruh");
    Ok(())
}

/// Messages that are used specifically by our [`App`].
#[derive(Clone, Debug)]
pub enum Message {
    Input1(String),
    Input2(String),
    Ignore,
    ToggleHide,
}

/// The [`App`] stores application-specific state.
pub struct App {
    core: Core,
    nav_model: nav_bar::Model,
    input_1: String,
    input_2: String,
    hidden: bool,
    date_selected: NaiveDate,
    config: Config,
    enigo: Enigo,
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
            date_selected: NaiveDate::from(Local::now().naive_local()),
            config: Config::new(Self::APP_ID, 1).unwrap(),
            enigo: Enigo::new(&enigo::Settings::default()).unwrap()
        };

        let config = &app.config;
        let tx = config.transaction();
        println!("Set example-bool to false: {:?}", tx.set("example-bool", false));
        println!("Set example-int to 0: {:?}", tx.set("example-int", 0));
        println!(
            "Set example-string to \"\": {:?}",
            tx.set("example-string", "")
        );
        println!("Set random thing to some big object {:?}", tx.set("random-thing", vec![1, 2, 3, 4, 5]));
        println!("Commit transaction: {:?}", tx.commit());

        let command = app.update_title();

        let enigo = &mut app.enigo;
        enigo.move_mouse(40, 40, Coordinate::Rel).expect("TODO: panic message");
        enigo.fast_text("baka").expect("TODO: panic message");

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
        let now: &NaiveDate = &self.date_selected;

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

        content = content.push(cosmic::widget::calendar::calendar(now, |date| Message::Input2(format!("Selected date: {}", date))));

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