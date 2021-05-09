mod i3_config;
mod style;

use style::Theme;

use clap::Clap;
use iced::{
    scrollable, text_input, Align, Application, Clipboard, Color, Column, Command, Container,
    Element, Font, Length, Row, Scrollable, Settings, Space, Subscription, Text, TextInput,
};

use iced_native::{
    keyboard::{Event, KeyCode},
    window,
    Event::{Keyboard, Window},
};

#[derive(Clap)]
#[clap(
    about = "Application for searching i3 config",
    author = "David W. <dweis7@gmail.com>"
)]
struct Args {
    #[clap(short, long, about = "Use light theme")]
    light: bool,
    #[clap(short, long, about = "Stay alive after focus loss")]
    keep_alive: bool,
}

pub fn main() {
    let args: Args = Args::parse();
    let theme = if args.light {
        Theme::Light
    } else {
        Theme::Dark
    };
    let init_flags = InitFlags::new(theme, !args.keep_alive);
    ApplicationState::run(Settings::with_flags(init_flags)).unwrap()
}

#[derive(Debug)]
struct InitFlags {
    theme: Theme,
    exit_on_focus_loss: bool,
}

impl InitFlags {
    fn new(theme: Theme, exit_on_focus_loss: bool) -> Self {
        InitFlags {
            theme,
            exit_on_focus_loss,
        }
    }
}

#[derive(Debug)]
struct ApplicationState {
    theme: Theme,
    exit_on_focus_loss: bool,
    state: Searcher,
    modifier_state: i3_config::Modifiers,
}

impl ApplicationState {
    fn new(theme: Theme, exit_on_focus_loss: bool) -> ApplicationState {
        ApplicationState {
            theme,
            exit_on_focus_loss,
            state: Searcher::Loading,
            modifier_state: i3_config::Modifiers::default(),
        }
    }
}

#[derive(Debug)]
struct State {
    scroll: scrollable::State,
    search_string: String,
    text_input_state: text_input::State,
    shortcuts: i3_config::ConfigMetadata,
}

impl State {
    pub fn new(config: i3_config::ConfigMetadata) -> State {
        State {
            scroll: scrollable::State::new(),
            search_string: String::from(""),
            text_input_state: text_input::State::focused(),
            shortcuts: config,
        }
    }
}

#[derive(Debug)]
enum Searcher {
    Loading,
    Searching(State),
    Error,
}

#[derive(Debug, Clone)]
enum Message {
    ConfigLoaded(Result<i3_config::ConfigMetadata, I3ConfigError>),
    InputChanged(String),
    Exit,
    EventOccurred(iced_native::Event),
}

#[derive(Debug, Clone)]
#[non_exhaustive]
enum I3ConfigError {
    LoadError,
}

async fn load_i3_config() -> Result<i3_config::ConfigMetadata, I3ConfigError> {
    i3_config::ConfigMetadata::load_ipc()
        .await
        .map_err(|_| I3ConfigError::LoadError)
}

impl Application for ApplicationState {
    type Executor = iced::executor::Default;
    type Message = Message;
    type Flags = InitFlags;

    fn new(flags: Self::Flags) -> (ApplicationState, Command<Message>) {
        (
            ApplicationState::new(flags.theme, flags.exit_on_focus_loss),
            Command::perform(load_i3_config(), Message::ConfigLoaded),
        )
    }

    fn title(&self) -> String {
        String::from("i3 Config Searcher")
    }

    fn update(&mut self, message: Message, _: &mut Clipboard) -> Command<Message> {
        match message {
            Message::ConfigLoaded(Ok(config)) => {
                self.state = Searcher::Searching(State::new(config));
                Command::none()
            }
            Message::ConfigLoaded(Err(_)) => {
                self.state = Searcher::Error;
                Command::none()
            }
            Message::InputChanged(input) => match &mut self.state {
                Searcher::Searching(state) => {
                    state.scroll = scrollable::State::new();
                    state.search_string = input;
                    Command::none()
                }
                _ => Command::none(),
            },
            Message::Exit => std::process::exit(0),
            Message::EventOccurred(Keyboard(Event::ModifiersChanged(modifiers))) => {
                let modifier_state = i3_config::Modifiers::new(
                    modifiers.shift,
                    modifiers.control,
                    modifiers.alt,
                    modifiers.logo,
                );
                self.modifier_state = modifier_state;
                Command::none()
            }
            Message::EventOccurred(Keyboard(Event::KeyReleased {
                key_code,
                modifiers,
            })) => {
                let modifier_state = i3_config::Modifiers::new(
                    modifiers.shift,
                    modifiers.control,
                    modifiers.alt,
                    modifiers.logo,
                );
                // This will work because KeyDown will release focus from the text input
                // and then we get the event here
                // This may be flaky and in the future this may need a better solution
                self.modifier_state = modifier_state;
                if key_code == KeyCode::Escape {
                    std::process::exit(0);
                }
                Command::none()
            }
            Message::EventOccurred(Window(window::Event::Unfocused)) => {
                if self.exit_on_focus_loss {
                    std::process::exit(0);
                }
                Command::none()
            }
            Message::EventOccurred(_) => Command::none(),
        }
    }

    fn subscription(&self) -> Subscription<Self::Message> {
        iced_native::subscription::events().map(Message::EventOccurred)
    }

    fn view(&mut self) -> Element<Message> {
        match &mut self.state {
            Searcher::Loading => Container::new(Text::new("Loading config...").size(40))
                .width(Length::Fill)
                .height(Length::Fill)
                .center_x()
                .center_y()
                .style(self.theme)
                .into(),
            Searcher::Error => Container::new(
                Text::new("Error loading i3 config")
                    .size(40)
                    .color(Color::from_rgb(1., 0., 0.)),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x()
            .center_y()
            .style(self.theme)
            .into(),
            Searcher::Searching(state) => {
                let input = TextInput::new(
                    &mut state.text_input_state,
                    "Enter search here...",
                    &state.search_string,
                    Message::InputChanged,
                )
                .width(Length::Fill)
                .style(self.theme)
                .size(30)
                .padding(10)
                .on_submit(Message::Exit);

                let modifiers_label = Row::new()
                    .width(Length::Fill)
                    .align_items(Align::Start)
                    .push(Space::new(Length::Units(10), Length::Units(20)))
                    .push(
                        Text::new(self.modifier_state.description())
                            .color(Color::from_rgb(0.5, 0.5, 0.5))
                            .font(FONT)
                            .size(20),
                    );

                let entries = state
                    .shortcuts
                    .filter(&state.search_string, &self.modifier_state)
                    .iter()
                    .fold(
                        Column::new().padding(20),
                        |column: Column<Message>, config_entry| column.push(config_entry.view()),
                    );

                let scrollable_entries = Scrollable::new(&mut state.scroll)
                    .push(entries)
                    .style(self.theme);

                let content = Column::new()
                    .push(input)
                    .push(modifiers_label)
                    .push(scrollable_entries)
                    .spacing(10)
                    .padding(5);

                Container::new(content)
                    .style(self.theme)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .center_x()
                    .align_y(iced::Align::Start)
                    .into()
            }
        }
    }
}

trait ViewModel {
    fn view(&self) -> Element<Message>;
}

impl ViewModel for i3_config::ConfigEntry {
    fn view(&self) -> Element<Message> {
        Row::new()
            .width(Length::Fill)
            .align_items(Align::Center)
            .padding(10)
            .push(
                Text::new(self.group().to_owned())
                    .font(FONT)
                    .size(20)
                    .color(Color::from_rgb(0.9, 0.6, 0.1)),
            )
            .push(Space::new(Length::Units(10), Length::Shrink))
            .push(Text::new(self.description().to_owned()).font(FONT).size(20))
            .push(Space::new(Length::Fill, Length::Shrink))
            .push(Text::new(self.keys().to_owned()).font(FONT).size(20))
            .into()
    }
}

const FONT: Font = Font::External {
    name: "MesloLGS",
    bytes: include_bytes!("../fonts/MesloLGS NF Regular.ttf"),
};
