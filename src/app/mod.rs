pub(crate) mod hotkeys;
pub(crate) mod init;
pub(crate) mod key_mapping;
pub(crate) mod message;
pub(crate) mod state;
pub(crate) mod subscription;
pub(crate) mod update;
pub(crate) mod view;

use crate::app::message::Message;
use crate::app::state::{EditorUiState, ExecutionState, MacroLibraryState};
use cosmic::app::{Core, Task};
use cosmic::cosmic_config::Config;
use cosmic::{executor, ApplicationExt, Element};

pub(crate) struct App {
    pub(crate) core: Core,
    pub(crate) config: Config,
    pub(crate) macro_lib: MacroLibraryState,
    pub(crate) execution: ExecutionState,
    pub(crate) editor_ui: EditorUiState,
}

impl App {
    pub(crate) fn update_title(&mut self) -> Task<Message> {
        let header_title = "Macros".to_string();
        let window_title = header_title.clone();
        self.set_header_title(header_title);
        if let Some(id) = self.core.main_window_id() {
            self.set_window_title(window_title, id)
        } else {
            Task::none()
        }
    }
}

impl cosmic::Application for App {
    type Executor = executor::Default;
    type Flags = ();
    type Message = Message;

    const APP_ID: &'static str = crate::config::APP_ID;

    fn core(&self) -> &Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    fn init(core: Core, _input: Self::Flags) -> (Self, Task<Self::Message>) {
        let mut app = init::build_app(core);
        let command = init::setup_app(&mut app);
        (app, command)
    }

    fn subscription(&self) -> cosmic::iced::Subscription<Self::Message> {
        subscription::build_subscription(self)
    }

    fn update(&mut self, message: Self::Message) -> Task<Self::Message> {
        if !matches!(message, Message::RemoveMacro | Message::RemoveMacroTick(_)) {
            self.editor_ui.confirm_remove_macro = false;
        }
        if !matches!(
            message,
            Message::ClearInstructions | Message::ClearInstructionsTick(_) | Message::ClearInstructionsTimeout(_)
        ) {
            self.editor_ui.confirm_clear_instructions = false;
        }
        update::handle_update(self, message)
    }

    fn view(&self) -> Element<'_, Self::Message> {
        view::build_view(self)
    }
}
