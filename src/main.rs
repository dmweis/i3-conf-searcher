mod i3_config;

use iced::{
    text_input, Align, Application, Column, Command, Container, Element, Font, Length, Row,
    Settings, Space, Text, TextInput,
};

pub fn main() {
    Searcher::run(Settings::default())
}

#[derive(Debug)]
enum Searcher {
    Loading,
    Searching {
        search_string: String,
        text_input_state: text_input::State,
        shortcuts: i3_config::ConfigMetadata,
    },
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

impl Application for Searcher {
    type Executor = iced::executor::Default;
    type Message = Message;
    type Flags = ();

    fn new(_flags: ()) -> (Searcher, Command<Message>) {
        (
            Searcher::Loading,
            Command::perform(load_i3_config(), Message::ConfigLoaded),
        )
    }

    fn title(&self) -> String {
        String::from("i3 Config Searcher")
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::ConfigLoaded(Ok(config)) => {
                *self = Searcher::Searching {
                    search_string: String::from(""),
                    text_input_state: text_input::State::focused(),
                    shortcuts: config,
                };

                Command::none()
            }
            Message::ConfigLoaded(Err(LoadError)) => {
                // TODO: Make a screen for this
                panic!("Failed to load i3 config!");
            }
            Message::InputChanged(input) => match self {
                Searcher::Searching {
                    search_string,
                    text_input_state: _,
                    shortcuts: _,
                } => {
                    *search_string = input;
                    Command::none()
                }
                _ => Command::none(),
            },
        }
    }

    fn view(&mut self) -> Element<Message> {
        let content: Element<_> = match self {
            Searcher::Loading => Column::new()
                .width(Length::Shrink)
                .push(Text::new("Loading config...").size(40))
                .into(),
            Searcher::Searching {
                search_string,
                text_input_state,
                shortcuts,
            } => {
                let input = TextInput::new(
                    text_input_state,
                    "Enter search here...",
                    search_string,
                    Message::InputChanged,
                )
                .width(Length::Fill)
                .size(40);

                let entries = shortcuts
                    .filter(&search_string)
                    .iter()
                    .fold(Column::new(), |column: Column<Message>, config_entry| {
                        column.push(config_entry.view())
                    });

                let content = Column::new()
                    .push(input)
                    .push(entries)
                    .spacing(20)
                    .padding(20);
                content.into()
            }
        };

        Container::new(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x()
            .align_y(iced::Align::Start)
            .into()
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
            .push(Text::new(self.description().to_owned()).font(FONT).size(20))
            .push(Space::new(Length::Fill, Length::Shrink))
            .push(Text::new(self.keys().to_owned()).font(FONT).size(20))
            .into()
    }
}

const FONT: Font = Font::External {
    name: "Icons",
    bytes: include_bytes!("../fonts/MesloLGS NF Regular.ttf"),
};
