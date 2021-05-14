mod i3_config;
mod keyboard_controller;
mod style;

use style::Theme;

use clap::Clap;
use iced::{
    container, scrollable, text_input, Align, Application, Background, Clipboard, Color, Column,
    Command, Container, Element, Font, Length, Row, Scrollable, Settings, Space, Subscription,
    Text, TextInput,
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
    selected_keys: Option<String>,
}

impl State {
    pub fn new(config: i3_config::ConfigMetadata) -> State {
        State {
            scroll: scrollable::State::new(),
            search_string: String::from(""),
            text_input_state: text_input::State::focused(),
            shortcuts: config,
            selected_keys: None,
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
    EventOccurred(iced_native::Event),
    Selected,
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
            Message::Selected => {
                if let Searcher::Searching(state) = &self.state {
                    if let Some(keys) = &state.selected_keys {
                        println!("Selected keys are {}", keys);
                        keyboard_controller::execute(&keys).unwrap();
                        std::process::exit(0);
                    } else {
                        println!("No keys selected");
                    }
                }
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
                .on_submit(Message::Selected);

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
                    .filter(&state.search_string, &self.modifier_state);

                let content = if entries.is_empty() {
                    state.selected_keys = None;
                    let warning = Text::new("No matching entries")
                        .size(40)
                        .horizontal_alignment(iced::HorizontalAlignment::Center)
                        .vertical_alignment(iced::VerticalAlignment::Top)
                        .width(Length::Fill)
                        .height(Length::Fill)
                        .color(Color::from_rgb(0.9, 0.6, 0.1));

                    Column::new()
                        .push(input)
                        .push(modifiers_label)
                        .push(warning)
                        .spacing(10)
                        .padding(5)
                } else {
                    state.selected_keys =
                        Some(entries.first().expect("Can't happen").keys().to_owned());
                    let entries_column = entries.iter().fold(
                        Column::new().padding(20),
                        |column: Column<Message>, config_entry| column.push(config_entry.view()),
                    );

                    let scrollable_entries = Scrollable::new(&mut state.scroll)
                        .push(entries_column)
                        .style(self.theme);
                    Column::new()
                        .push(input)
                        .push(modifiers_label)
                        .push(scrollable_entries)
                        .spacing(10)
                        .padding(5)
                };

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
        let mut row = Row::new()
            .width(Length::Fill)
            .align_items(Align::Center)
            .padding(10);

        for element in self.matched_group() {
            match element {
                i3_config::MatchElement::Matched(element) => {
                    row = row.push(
                        Text::new(element)
                            .font(FONT)
                            .size(20)
                            .color(Color::from_rgb(1.0, 0.0, 0.5)),
                    );
                }

                i3_config::MatchElement::Unmatched(element) => {
                    row = row.push(
                        Text::new(element.to_owned())
                            .font(FONT)
                            .size(20)
                            .color(Color::from_rgb(0.9, 0.6, 0.1)),
                    );
                }
            }
        }

        row = row.push(Space::new(Length::Units(10), Length::Shrink));
        for element in self.matched_description() {
            match element {
                i3_config::MatchElement::Matched(element) => {
                    row = row.push(
                        Text::new(element)
                            .font(FONT)
                            .size(20)
                            .color(Color::from_rgb(1.0, 0.0, 0.5)),
                    );
                }

                i3_config::MatchElement::Unmatched(element) => {
                    row = row.push(Text::new(element.to_owned()).font(FONT).size(20));
                }
            }
        }
        row = row
            .push(Space::new(Length::Fill, Length::Shrink))
            .push(Text::new(self.keys().to_owned()).font(FONT).size(20));

        if self.is_selected() {
            Container::new(row)
                .width(Length::Fill)
                .style(SelectedContainer {})
                .into()
        } else {
            row.into()
        }
    }
}

const FONT: Font = Font::External {
    name: "MesloLGS",
    bytes: include_bytes!("../fonts/MesloLGS NF Regular.ttf"),
};

pub struct SelectedContainer;

impl container::StyleSheet for SelectedContainer {
    fn style(&self) -> container::Style {
        container::Style {
            background: Some(Background::Color(Color::from_rgb8(45, 43, 79))),
            text_color: Some(Color::WHITE),
            ..container::Style::default()
        }
    }
}
