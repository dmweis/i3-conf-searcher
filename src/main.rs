mod i3_config;
mod style;

use style::Theme;

use iced::{
    scrollable, text_input, Align, Application, Color, Column, Command, Container, Element, Font,
    Length, Row, Scrollable, Settings, Space, Text, TextInput,
};

pub fn main() {
    ApplicationState::run(Settings::with_flags(Theme::Dark))
}

#[derive(Debug)]
struct ApplicationState {
    theme: Theme,
    state: Searcher,
}

impl ApplicationState {
    fn new(theme: Theme) -> ApplicationState {
        ApplicationState {
            theme,
            state: Searcher::Loading,
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
    ConfigLoaded(Result<i3_config::ConfigMetadata, LoadError>),
    InputChanged(String),
}

#[derive(Debug, Clone)]
struct LoadError;

async fn load_i3_config() -> Result<i3_config::ConfigMetadata, LoadError> {
    i3_config::ConfigMetadata::load_ipc()
        .await
        .map_err(|_| LoadError)
}

impl Application for ApplicationState {
    type Executor = iced::executor::Default;
    type Message = Message;
    type Flags = Theme;

    fn new(flags: Theme) -> (ApplicationState, Command<Message>) {
        (
            ApplicationState::new(flags),
            Command::perform(load_i3_config(), Message::ConfigLoaded),
        )
    }

    fn title(&self) -> String {
        String::from("i3 Config Searcher")
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::ConfigLoaded(Ok(config)) => {
                self.state = Searcher::Searching(State::new(config));
                Command::none()
            }
            Message::ConfigLoaded(Err(LoadError)) => {
                self.state = Searcher::Error;
                Command::none()
            }
            Message::InputChanged(input) => match &mut self.state {
                Searcher::Searching(state) => {
                    state.search_string = input;
                    Command::none()
                }
                _ => Command::none(),
            },
        }
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
                .size(40);

                let entries = state
                    .shortcuts
                    .filter(&state.search_string)
                    .iter()
                    .fold(Column::new(), |column: Column<Message>, config_entry| {
                        column.push(config_entry.view())
                    });

                let scrollable_entries = Scrollable::new(&mut state.scroll)
                    .push(entries)
                    .style(self.theme);

                let content = Column::new()
                    .push(input)
                    .push(scrollable_entries)
                    .spacing(20)
                    .padding(20);

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
            .padding(30)
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
