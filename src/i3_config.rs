use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use regex::Regex;
use std::{error, fmt};
use tokio_i3ipc::I3;

type Result<T> = std::result::Result<T, Box<dyn error::Error + Send + Sync>>;

#[derive(Debug, Clone)]
struct I3ConfigError;

impl fmt::Display for I3ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "failed to parse i3 config")
    }
}

impl error::Error for I3ConfigError {}

async fn get_i3_config_ipc() -> Result<String> {
    let mut i3 = I3::connect().await?;
    let config = i3.get_config().await?;
    Ok(config.config)
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConfigEntry {
    group: String,
    description: String,
    keys: String,
}

impl ConfigEntry {
    pub fn description(&self) -> &str {
        &self.description
    }

    pub fn keys(&self) -> &str {
        &self.keys
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConfigMetadata {
    entries: Vec<ConfigEntry>,
}

impl ConfigMetadata {
    fn parse(text: &str) -> Result<ConfigMetadata> {
        let re = Regex::new(r"##(?P<group>.*)//(?P<description>.*)//(?P<keys>.*)##")?;
        let mut entries = vec![];
        for cap in re.captures_iter(text) {
            let entry = ConfigEntry {
                group: cap
                    .name("group")
                    .ok_or(I3ConfigError)?
                    .as_str()
                    .trim()
                    .to_owned(),
                description: cap
                    .name("description")
                    .ok_or(I3ConfigError)?
                    .as_str()
                    .trim()
                    .to_owned(),
                keys: cap
                    .name("keys")
                    .ok_or(I3ConfigError)?
                    .as_str()
                    .trim()
                    .to_owned(),
            };
            entries.push(entry);
        }
        Ok(ConfigMetadata { entries })
    }

    pub async fn load_ipc() -> Result<ConfigMetadata> {
        let config_text = get_i3_config_ipc().await?;
        ConfigMetadata::parse(&config_text)
    }

    pub fn filter(&self, filter: &str) -> Vec<&ConfigEntry> {
        let matcher = SkimMatcherV2::default();
        let mut matches = vec![];
        for entry in &self.entries {
            if let Some(score) = matcher.fuzzy_match(entry.description(), filter) {
                matches.push((entry, score))
            }
        }
        matches.sort_by(|a, b| b.1.cmp(&a.1));
        matches.into_iter().map(|(val, _)| val).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn simple_i3_config() -> &'static str {
        let sample = "## group1 // description1 // keys1 ##
        bindsym $mod+Ctrl+$alt+Left move workspace to output left
        ## group2 // description2 // keys2 ##
        bindsym $mod+grave exec /usr/bin/x-terminal-emulator";
        sample
    }

    #[test]
    fn parse_simple_i3_config() {
        let sample = simple_i3_config();
        let config = ConfigMetadata::parse(sample).unwrap();
        assert_eq!(config.entries.len(), 2);
        assert_eq!(
            config.entries[0],
            ConfigEntry {
                group: String::from("group1"),
                description: String::from("description1"),
                keys: String::from("keys1"),
            }
        );
        assert_eq!(
            config.entries[1],
            ConfigEntry {
                group: String::from("group2"),
                description: String::from("description2"),
                keys: String::from("keys2"),
            }
        );
    }

    #[test]
    fn parse_simple_i3_no_vals() {
        let sample = "bindsym $mod+Ctrl+$alt+Left move workspace to output left
        bindsym $mod+grave exec /usr/bin/x-terminal-emulator";
        let config = ConfigMetadata::parse(sample).unwrap();
        assert_eq!(config.entries.len(), 0);
    }

    #[test]
    fn parse_simple_i3_empty() {
        let sample = "";
        let config = ConfigMetadata::parse(sample).unwrap();
        assert_eq!(config.entries.len(), 0);
    }

    #[test]
    fn parse_simple_i3_config_comments() {
        let sample = "## group1 // description1 // keys1 ## some comments";
        let config = ConfigMetadata::parse(sample).unwrap();
        assert_eq!(config.entries.len(), 1);
        assert_eq!(
            config.entries[0],
            ConfigEntry {
                group: String::from("group1"),
                description: String::from("description1"),
                keys: String::from("keys1"),
            }
        );
    }

    #[test]
    fn parse_simple_i3_config_multiple_words() {
        let sample = "## this is group1 // this is description1 // this is keys1 ##";
        let config = ConfigMetadata::parse(sample).unwrap();
        assert_eq!(config.entries.len(), 1);
        assert_eq!(
            config.entries[0],
            ConfigEntry {
                group: String::from("this is group1"),
                description: String::from("this is description1"),
                keys: String::from("this is keys1"),
            }
        );
    }

    #[test]
    fn parse_simple_i3_config_line_comment() {
        let sample = "# other comment
        ## group1 // description1 // keys1 ##";
        let config = ConfigMetadata::parse(sample).unwrap();
        assert_eq!(config.entries.len(), 1);
        assert_eq!(
            config.entries[0],
            ConfigEntry {
                group: String::from("group1"),
                description: String::from("description1"),
                keys: String::from("keys1"),
            }
        );
    }

    #[test]
    fn filter_i3_entries() {
        let sample = simple_i3_config();
        let config = ConfigMetadata::parse(sample).unwrap();
        let filtered_entries = config.filter("dsc1");
        assert_eq!(filtered_entries.len(), 1);
        assert_eq!(
            filtered_entries[0].description(),
            String::from("description1")
        );
    }

    #[test]
    fn filter_i3_entries_empty_returns_all() {
        let sample = simple_i3_config();
        let config = ConfigMetadata::parse(sample).unwrap();
        let filtered_entries = config.filter("");
        assert_eq!(filtered_entries.len(), 2);
    }

    #[test]
    fn filter_i3_entries_no_match() {
        let sample = simple_i3_config();
        let config = ConfigMetadata::parse(sample).unwrap();
        let filtered_entries = config.filter("qw");
        assert!(filtered_entries.is_empty());
    }

    #[test]
    fn filter_i3_entries_sorted() {
        let sample = "## group1 // abdc // keys1 ##
        ## group2 // abc // keys2 ##";
        let config = ConfigMetadata::parse(sample).unwrap();
        let filtered_entries = config.filter("abc");
        assert_eq!(filtered_entries.len(), 2);
        assert_eq!(filtered_entries[0].description(), String::from("abc"));
        assert_eq!(filtered_entries[1].description(), String::from("abdc"));
    }
}
