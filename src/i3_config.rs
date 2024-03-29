use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use regex::Regex;
use thiserror::Error;
#[cfg(target_family = "unix")]
use tokio_i3ipc::I3;

type Result<T> = std::result::Result<T, I3ConfigError>;

#[derive(Debug, Error, Clone)]
pub enum I3ConfigError {
    #[error("failed to parse config")]
    ConfigParsingError,
    #[allow(dead_code)]
    #[error("failed to query i3 for config")]
    FailedI3Query,
    #[allow(dead_code)]
    #[error("i3 not supported on this platform")]
    UnsupportedPlatform,
    #[error("Failed to download file")]
    FailedGetRequest,
}

#[cfg(target_family = "unix")]
async fn get_i3_config_ipc() -> Result<String> {
    let mut i3 = I3::connect()
        .await
        .map_err(|_| I3ConfigError::FailedI3Query)?;
    let config = i3
        .get_config()
        .await
        .map_err(|_| I3ConfigError::FailedI3Query)?;
    Ok(config.config)
}

#[cfg(target_family = "windows")]
async fn get_i3_config_ipc() -> Result<String> {
    Err(I3ConfigError::UnsupportedPlatform)
}

async fn download_i3_config(url: &str) -> Result<String> {
    // TODO (David): This method doesn't really
    // provide much detail about why it failed.
    // Maybe add some error propagation. Thiserror
    // makes that easy
    let response = reqwest::get(url)
        .await
        .map_err(|_| I3ConfigError::FailedGetRequest)?;
    if !response.status().is_success() {
        eprintln!("Web request failed with status {:?}", response.status());
        return Err(I3ConfigError::FailedGetRequest);
    }
    let config = response
        .text()
        .await
        .map_err(|_| I3ConfigError::FailedGetRequest)?;
    Ok(config)
}

const SHIFT_PATTERN: &str = "<shift>";
const CONTROL_PATTERN: &str = "<ctrl>";
const ALT_PATTERN: &str = "<alt>";
const META_PATTERN: &str = "<>";

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Modifiers {
    shift: bool,
    control: bool,
    alt: bool,
    meta: bool,
}

impl Modifiers {
    pub fn new(shift: bool, control: bool, alt: bool, meta: bool) -> Self {
        Modifiers {
            shift,
            control,
            alt,
            meta,
        }
    }

    pub fn description(&self) -> String {
        let mut description = String::new();
        if self.meta {
            description.push_str(META_PATTERN);
        }
        if self.control {
            description.push_str(CONTROL_PATTERN);
        }
        if self.shift {
            description.push_str(SHIFT_PATTERN);
        }
        if self.alt {
            description.push_str(ALT_PATTERN);
        }
        if description.is_empty() {
            String::from("No modifiers pressed...")
        } else {
            description
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConfigEntry {
    group: String,
    description: String,
    keys: String,
    description_indices: Option<Vec<usize>>,
    group_indices: Option<Vec<usize>>,
}

impl ConfigEntry {
    pub fn new(group: String, description: String, keys: String) -> Self {
        ConfigEntry {
            group,
            description,
            keys,
            description_indices: None,
            group_indices: None,
        }
    }

    pub fn group(&self) -> &str {
        &self.group
    }

    pub fn description(&self) -> &str {
        &self.description
    }

    pub fn full_text(&self) -> String {
        format!("{} {}", self.group, self.description)
    }

    pub fn keys(&self) -> &str {
        &self.keys
    }

    pub fn matches_modifiers(&self, modifiers: &Modifiers) -> bool {
        let lower_case_keys = self.keys.to_lowercase();
        if modifiers.shift && !lower_case_keys.contains(SHIFT_PATTERN) {
            return false;
        }
        if modifiers.control && !lower_case_keys.contains(CONTROL_PATTERN) {
            return false;
        }
        if modifiers.alt && !lower_case_keys.contains(ALT_PATTERN) {
            return false;
        }
        if modifiers.meta && !lower_case_keys.contains(META_PATTERN) {
            return false;
        }
        true
    }

    pub fn clear_matches(&mut self) {
        self.group_indices = None;
        self.description_indices = None;
    }

    pub fn set_group_indices(&mut self, indices: Vec<usize>) {
        self.group_indices = Some(indices);
    }

    pub fn set_description_indices(&mut self, indices: Vec<usize>) {
        self.description_indices = Some(indices);
    }

    pub fn matched_description(&self) -> Vec<MatchElement> {
        split_to_groups_by_indices(&self.description(), self.description_indices.as_ref())
    }
    pub fn matched_group(&self) -> Vec<MatchElement> {
        split_to_groups_by_indices(&self.group(), self.group_indices.as_ref())
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum MatchElement {
    Matched(String),
    Unmatched(String),
}

fn split_to_groups_by_indices(text: &str, indices: Option<&Vec<usize>>) -> Vec<MatchElement> {
    if let Some(indices) = indices {
        let mut parts = vec![];
        let mut buffer = String::new();
        let mut last_matched = false;
        for (index, character) in text.chars().enumerate() {
            let matched = indices.contains(&index);
            if matched {
                if last_matched {
                    buffer.push(character);
                } else {
                    if !buffer.is_empty() {
                        parts.push(MatchElement::Unmatched(buffer.clone()));
                    }
                    buffer.clear();
                    buffer.push(character);
                }
            } else if last_matched {
                if !buffer.is_empty() {
                    parts.push(MatchElement::Matched(buffer.clone()));
                }
                buffer.clear();
                buffer.push(character);
            } else {
                buffer.push(character);
            }
            last_matched = matched;
        }
        if last_matched {
            parts.push(MatchElement::Matched(buffer));
        } else {
            parts.push(MatchElement::Unmatched(buffer));
        }
        parts
    } else {
        vec![MatchElement::Unmatched(text.to_owned())]
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConfigMetadata {
    entries: Vec<ConfigEntry>,
}

impl ConfigMetadata {
    fn parse(text: &str) -> Result<ConfigMetadata> {
        let re = Regex::new(r"(?m)^\s*##(?P<group>.*)//(?P<description>.*)//(?P<keys>.*)##")
            .map_err(|_| I3ConfigError::ConfigParsingError)?;
        let mut entries = vec![];
        for cap in re.captures_iter(text) {
            let entry = ConfigEntry::new(
                cap.name("group")
                    .ok_or(I3ConfigError::ConfigParsingError)?
                    .as_str()
                    .trim()
                    .to_owned(),
                cap.name("description")
                    .ok_or(I3ConfigError::ConfigParsingError)?
                    .as_str()
                    .trim()
                    .to_owned(),
                cap.name("keys")
                    .ok_or(I3ConfigError::ConfigParsingError)?
                    .as_str()
                    .trim()
                    .to_owned(),
            );
            entries.push(entry);
        }
        Ok(ConfigMetadata { entries })
    }

    pub async fn load_from_ipc() -> Result<ConfigMetadata> {
        let config_text = get_i3_config_ipc().await?;
        ConfigMetadata::parse(&config_text)
    }

    pub async fn load_from_web(url: &str) -> Result<ConfigMetadata> {
        let config_text = download_i3_config(url).await?;
        ConfigMetadata::parse(&config_text)
    }

    pub fn filter(&mut self, filter: &str, modifiers: &Modifiers) -> Vec<&ConfigEntry> {
        let matcher = SkimMatcherV2::default();
        let mut matches = vec![];
        for entry in &mut self.entries {
            entry.clear_matches();
            if let Some((score, indices)) = matcher.fuzzy_indices(&entry.full_text(), filter) {
                if entry.matches_modifiers(&modifiers) {
                    let group_len = entry.group().len();
                    entry.set_group_indices(
                        indices
                            .iter()
                            .cloned()
                            .filter(|val| *val < group_len)
                            .collect(),
                    );
                    entry.set_description_indices(
                        indices
                            .iter()
                            .cloned()
                            .filter(|val| *val > group_len)
                            .map(|val| val - group_len - 1)
                            .collect(),
                    );
                    matches.push((entry, score))
                }
            }
        }
        matches.sort_by(|a, b| b.1.cmp(&a.1));
        matches.into_iter().map(|(val, _)| &*val).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn simple_i3_config() -> &'static str {
        "## group1 // description1 // keys1 ##
        bindsym $mod+Ctrl+$alt+Left move workspace to output left
        ## group2 // description2 // keys2 ##
        bindsym $mod+grave exec /usr/bin/x-terminal-emulator"
    }

    #[test]
    fn parse_simple_i3_config() {
        let sample = simple_i3_config();
        let config = ConfigMetadata::parse(sample).unwrap();
        assert_eq!(config.entries.len(), 2);
        assert_eq!(
            config.entries[0],
            ConfigEntry::new(
                String::from("group1"),
                String::from("description1"),
                String::from("keys1"),
            )
        );
        assert_eq!(
            config.entries[1],
            ConfigEntry::new(
                String::from("group2"),
                String::from("description2"),
                String::from("keys2"),
            )
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
            ConfigEntry::new(
                String::from("group1"),
                String::from("description1"),
                String::from("keys1"),
            )
        );
    }

    #[test]
    fn parse_simple_i3_ignore_commented() {
        let sample = "# ## group1 // description1 // keys1 ## some comments";
        let config = ConfigMetadata::parse(sample).unwrap();
        assert!(config.entries.is_empty());
    }

    #[test]
    fn parse_simple_i3_config_multiple_words() {
        let sample = "## this is group1 // this is description1 // this is keys1 ##";
        let config = ConfigMetadata::parse(sample).unwrap();
        assert_eq!(config.entries.len(), 1);
        assert_eq!(
            config.entries[0],
            ConfigEntry::new(
                String::from("this is group1"),
                String::from("this is description1"),
                String::from("this is keys1"),
            )
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
            ConfigEntry::new(
                String::from("group1"),
                String::from("description1"),
                String::from("keys1"),
            )
        );
    }

    #[test]
    fn filter_i3_entries() {
        let sample = simple_i3_config();
        let mut config = ConfigMetadata::parse(sample).unwrap();
        let filtered_entries = config.filter("dsc1", &Modifiers::default());
        assert_eq!(filtered_entries.len(), 1);
        assert_eq!(
            filtered_entries[0].description(),
            String::from("description1")
        );
    }

    #[test]
    fn filter_i3_entries_empty_returns_all() {
        let sample = simple_i3_config();
        let mut config = ConfigMetadata::parse(sample).unwrap();
        let filtered_entries = config.filter("", &Modifiers::default());
        assert_eq!(filtered_entries.len(), 2);
    }

    #[test]
    fn filter_i3_entries_no_match() {
        let sample = simple_i3_config();
        let mut config = ConfigMetadata::parse(sample).unwrap();
        let filtered_entries = config.filter("qw", &Modifiers::default());
        assert!(filtered_entries.is_empty());
    }

    #[test]
    fn filter_i3_entries_sorted() {
        let sample = "## group1 // abdc // keys1 ##
        ## group2 // abc // keys2 ##";
        let mut config = ConfigMetadata::parse(sample).unwrap();
        let filtered_entries = config.filter("abc", &Modifiers::default());
        assert_eq!(filtered_entries.len(), 2);
        assert_eq!(filtered_entries[0].description(), String::from("abc"));
        assert_eq!(filtered_entries[1].description(), String::from("abdc"));
    }

    #[test]
    fn filter_i3_by_group() {
        let sample = "## group1 // abdc // keys1 ##
        ## group2 // abc // keys2 ##";
        let mut config = ConfigMetadata::parse(sample).unwrap();
        let filtered_entries = config.filter("grp2", &Modifiers::default());
        assert_eq!(filtered_entries.len(), 1);
        assert_eq!(filtered_entries[0].description(), String::from("abc"));
    }

    #[test]
    fn test_modifiers_shift() {
        let modifiers = Modifiers::new(true, false, false, false);
        let short_cut = ConfigEntry::new(
            String::from("group"),
            String::from("group"),
            String::from("<shift>"),
        );
        assert!(short_cut.matches_modifiers(&modifiers))
    }

    #[test]
    fn test_modifiers_not_shift() {
        let modifiers = Modifiers::new(true, false, false, false);
        let short_cut = ConfigEntry::new(
            String::from("group"),
            String::from("group"),
            String::from("<ctrl>"),
        );
        assert!(!short_cut.matches_modifiers(&modifiers))
    }

    #[test]
    fn test_modifiers_shift_upper_case() {
        let modifiers = Modifiers::new(true, false, false, false);
        let short_cut = ConfigEntry::new(
            String::from("group"),
            String::from("group"),
            String::from("<Shift><ctrl>"),
        );
        assert!(short_cut.matches_modifiers(&modifiers))
    }

    #[test]
    fn test_modifiers_control() {
        let modifiers = Modifiers::new(false, true, false, false);
        let short_cut = ConfigEntry::new(
            String::from("group"),
            String::from("group"),
            String::from("<ctrl><alt>"),
        );
        assert!(short_cut.matches_modifiers(&modifiers))
    }

    #[test]
    fn test_modifiers_alt() {
        let modifiers = Modifiers::new(false, false, true, false);
        let short_cut = ConfigEntry::new(
            String::from("group"),
            String::from("group"),
            String::from("<alt>"),
        );
        assert!(short_cut.matches_modifiers(&modifiers))
    }

    #[test]
    fn test_modifiers_meta() {
        let modifiers = Modifiers::new(false, false, false, true);
        let short_cut = ConfigEntry::new(
            String::from("group"),
            String::from("group"),
            String::from("<>"),
        );
        assert!(short_cut.matches_modifiers(&modifiers))
    }

    #[test]
    fn test_modifiers_ctrl_shift() {
        let modifiers = Modifiers::new(true, true, false, false);
        let short_cut = ConfigEntry::new(
            String::from("group"),
            String::from("group"),
            String::from("<Shift><ctrl>"),
        );
        assert!(short_cut.matches_modifiers(&modifiers))
    }

    #[test]
    fn highlight_simple_group() {
        let sample = "## group1 // abdc // keys1 ##";
        let mut config = ConfigMetadata::parse(sample).unwrap();
        let filtered_entries = config.filter("gro", &Modifiers::default());
        let expected_group = vec![
            MatchElement::Matched("gro".to_owned()),
            MatchElement::Unmatched("up1".to_owned()),
        ];
        let expected_description = vec![MatchElement::Unmatched("abdc".to_owned())];
        assert_eq!(filtered_entries[0].matched_group(), expected_group);
        assert_eq!(
            filtered_entries[0].matched_description(),
            expected_description
        );
    }

    #[test]
    fn highlight_simple_description() {
        let sample = "## group1 // abdc // keys1 ##";
        let mut config = ConfigMetadata::parse(sample).unwrap();
        let filtered_entries = config.filter("ab", &Modifiers::default());
        let expected_group = vec![MatchElement::Unmatched("group1".to_owned())];
        let expected_description = vec![
            MatchElement::Matched("ab".to_owned()),
            MatchElement::Unmatched("dc".to_owned()),
        ];
        assert_eq!(filtered_entries[0].matched_group(), expected_group);
        assert_eq!(
            filtered_entries[0].matched_description(),
            expected_description
        );
    }

    #[test]
    fn highlight_simple_with_space() {
        let sample = "## group1 // abdc // keys1 ##";
        let mut config = ConfigMetadata::parse(sample).unwrap();
        let filtered_entries = config.filter("group1 abdc", &Modifiers::default());
        let expected_group = vec![MatchElement::Matched("group1".to_owned())];
        let expected_description = vec![MatchElement::Matched("abdc".to_owned())];
        assert_eq!(filtered_entries[0].matched_group(), expected_group);
        assert_eq!(
            filtered_entries[0].matched_description(),
            expected_description
        );
    }
}
