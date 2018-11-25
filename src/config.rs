use sdl2::controller::Button;
use sdl2::keyboard::Keycode;
use serde::de::{Deserialize, Deserializer, Error, SeqAccess, Unexpected, Visitor};
use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use std::str;
use toml::{value, Value};

const CONTROLLER_FIELDS: [&str; 8] = ["a", "b", "select", "start", "up", "down", "left", "right"];

#[derive(Clone, Copy, Eq, Hash, PartialEq)]
pub enum KeybindingValue {
    ButtonValue(Button),
    KeycodeValue(Keycode),
}
impl KeybindingValue {
    pub fn from_string(controller_type: &ControllerType, value: &str) -> Option<KeybindingValue> {
        if *controller_type == ControllerType::Keyboard {
            Keycode::from_name(value).map(KeybindingValue::KeycodeValue)
        } else {
            Button::from_string(value).map(KeybindingValue::ButtonValue)
        }
    }
}

struct RawKeybindingValues(Vec<String>);
struct RawKeybindingValuesVisitor(PhantomData<RawKeybindingValues>);

impl<'de> Visitor<'de> for RawKeybindingValuesVisitor {
    type Value = RawKeybindingValues;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("keycode string or list of keycode strings")
    }

    fn visit_str<E>(self, keycode_name: &str) -> Result<Self::Value, E>
    where
        E: Error,
    {
        Ok(RawKeybindingValues(vec![keycode_name.to_owned()]))
    }

    fn visit_seq<S>(self, mut visitor: S) -> Result<Self::Value, S::Error>
    where
        S: SeqAccess<'de>,
    {
        let mut value = visitor.next_element::<String>()?;
        let mut keycodes = Vec::new();
        while let Some(keycode_name) = value {
            keycodes.push(keycode_name);
            value = visitor.next_element::<String>()?;
        }
        Ok(RawKeybindingValues(keycodes))
    }
}

impl<'de> Deserialize<'de> for RawKeybindingValues {
    fn deserialize<D>(deserializer: D) -> Result<RawKeybindingValues, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(RawKeybindingValuesVisitor(PhantomData))
    }
}

#[derive(Deserialize, PartialEq)]
pub enum ControllerType {
    Controller,
    Keyboard,
}

#[derive(Deserialize)]
struct RawKeybindingConfig {
    #[serde(rename = "type")]
    controller_type: ControllerType,
    #[serde(flatten)]
    raw_keybindings: HashMap<String, RawKeybindingValues>,
}

impl RawKeybindingConfig {
    fn default_keyboard() -> Self {
        RawKeybindingConfig {
            controller_type: ControllerType::Keyboard,
            raw_keybindings: vec![
                ("a".to_string(), RawKeybindingValues(vec!["P".to_owned()])),
                ("b".to_string(), RawKeybindingValues(vec!["O".to_owned()])),
                (
                    "select".to_string(),
                    RawKeybindingValues(vec!["Left Shift".to_owned(), "Right Shift".to_owned()]),
                ),
                (
                    "start".to_string(),
                    RawKeybindingValues(vec!["Return".to_owned()]),
                ),
                ("up".to_string(), RawKeybindingValues(vec!["W".to_owned()])),
                (
                    "down".to_string(),
                    RawKeybindingValues(vec!["S".to_owned()]),
                ),
                (
                    "left".to_string(),
                    RawKeybindingValues(vec!["A".to_owned()]),
                ),
                (
                    "right".to_string(),
                    RawKeybindingValues(vec!["D".to_owned()]),
                ),
            ]
            .into_iter()
            .collect(),
        }
    }

    fn default_controller() -> Self {
        RawKeybindingConfig {
            controller_type: ControllerType::Controller,
            raw_keybindings: vec![
                ("a".to_string(), RawKeybindingValues(vec!["a".to_owned()])),
                ("b".to_string(), RawKeybindingValues(vec!["b".to_owned()])),
                (
                    "select".to_string(),
                    RawKeybindingValues(vec!["back".to_owned()]),
                ),
                (
                    "start".to_string(),
                    RawKeybindingValues(vec!["start".to_owned()]),
                ),
                (
                    "up".to_string(),
                    RawKeybindingValues(vec!["dpup".to_owned()]),
                ),
                (
                    "down".to_string(),
                    RawKeybindingValues(vec!["dpdown".to_owned()]),
                ),
                (
                    "left".to_string(),
                    RawKeybindingValues(vec!["dpleft".to_owned()]),
                ),
                (
                    "right".to_string(),
                    RawKeybindingValues(vec!["dpright".to_owned()]),
                ),
            ]
            .into_iter()
            .collect(),
        }
    }
}

pub struct ControllerConfig {
    pub keybinding_map: HashMap<KeybindingValue, usize>,
}

impl ControllerConfig {
    fn new(keybinding_map: HashMap<KeybindingValue, usize>) -> Self {
        ControllerConfig { keybinding_map }
    }
}

impl<'de> Deserialize<'de> for ControllerConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let mut controller_config = ControllerConfig::new(HashMap::new());

        let parsed_raw_config = RawKeybindingConfig::deserialize(deserializer)?;
        let mut raw_config = if parsed_raw_config.controller_type == ControllerType::Keyboard {
            RawKeybindingConfig::default_keyboard()
        } else {
            RawKeybindingConfig::default_controller()
        };
        raw_config
            .raw_keybindings
            .extend(parsed_raw_config.raw_keybindings.into_iter());
        let controller_type = raw_config.controller_type;

        for entry in raw_config.raw_keybindings {
            match CONTROLLER_FIELDS
                .iter()
                .position(|field| **field == entry.0)
            {
                Some(index) => {
                    for raw_keybinding_str in (entry.1).0 {
                        let keybinding =
                            KeybindingValue::from_string(&controller_type, &raw_keybinding_str)
                                .ok_or_else(|| {
                                    let err_msg = if controller_type == ControllerType::Keyboard {
                                        &"a string as a keycode string."
                                    } else {
                                        &"a string as a button name."
                                    };
                                    Error::invalid_value(
                                        Unexpected::Str(&raw_keybinding_str),
                                        err_msg,
                                    )
                                })?;
                        controller_config.keybinding_map.insert(keybinding, index);
                    }
                },
                None => {
                    return Err(Error::invalid_value(
                        Unexpected::Str(&entry.0),
                        &"a valid controller field",
                    ))
                },
            }
        }

        Ok(controller_config)
    }
}

impl Default for ControllerConfig {
    fn default() -> Self {
        ControllerConfig {
            keybinding_map: vec![
                (KeybindingValue::KeycodeValue(Keycode::P), 0),
                (KeybindingValue::KeycodeValue(Keycode::O), 1),
                (KeybindingValue::KeycodeValue(Keycode::RShift), 2),
                (KeybindingValue::KeycodeValue(Keycode::LShift), 2),
                (KeybindingValue::KeycodeValue(Keycode::Return), 3),
                (KeybindingValue::KeycodeValue(Keycode::W), 4),
                (KeybindingValue::KeycodeValue(Keycode::S), 5),
                (KeybindingValue::KeycodeValue(Keycode::A), 6),
                (KeybindingValue::KeycodeValue(Keycode::D), 7),
            ]
            .into_iter()
            .collect(),
        }
    }
}

pub struct KeybindingsConfig {
    pub mute: Vec<KeybindingValue>,
    pub pause: Vec<KeybindingValue>,
    pub reset: Vec<KeybindingValue>,
    pub exit: Vec<KeybindingValue>,
    pub save_state: Vec<KeybindingValue>,
    pub load_state: Vec<KeybindingValue>,
    pub increase_speed: Vec<KeybindingValue>,
    pub decrease_speed: Vec<KeybindingValue>,
}

impl<'de> Deserialize<'de> for KeybindingsConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let mut keybindings_config = KeybindingsConfig::default();
        let raw_config = RawKeybindingConfig::deserialize(deserializer)?;
        let controller_type = raw_config.controller_type;

        for entry in raw_config.raw_keybindings {
            let mut keybindings = Vec::new();
            for raw_keybinding_str in (entry.1).0.iter() {
                let keybinding =
                    KeybindingValue::from_string(&controller_type, &raw_keybinding_str)
                        .ok_or_else(|| {
                            let err_msg = if controller_type == ControllerType::Keyboard {
                                &"a string as a keycode string."
                            } else {
                                &"a string as a button name."
                            };
                            Error::invalid_value(Unexpected::Str(&raw_keybinding_str), err_msg)
                        })?;
                keybindings.push(keybinding);
            }
            match entry.0.as_ref() {
                "mute" => keybindings_config.mute = keybindings,
                "pause" => keybindings_config.pause = keybindings,
                "reset" => keybindings_config.reset = keybindings,
                "exit" => keybindings_config.exit = keybindings,
                "save_state" => keybindings_config.save_state = keybindings,
                "load_state" => keybindings_config.load_state = keybindings,
                "increase_speed" => keybindings_config.increase_speed = keybindings,
                "decrease_speed" => keybindings_config.decrease_speed = keybindings,
                _ => {
                    return Err(Error::invalid_value(
                        Unexpected::Str(&entry.0),
                        &"a valid controller field",
                    ))
                },
            }
        }

        Ok(keybindings_config)
    }
}

impl Default for KeybindingsConfig {
    fn default() -> Self {
        KeybindingsConfig {
            mute: vec![KeybindingValue::KeycodeValue(Keycode::M)],
            pause: vec![KeybindingValue::KeycodeValue(Keycode::Space)],
            reset: vec![KeybindingValue::KeycodeValue(Keycode::R)],
            exit: vec![KeybindingValue::KeycodeValue(Keycode::Escape)],
            save_state: vec![KeybindingValue::KeycodeValue(Keycode::F1)],
            load_state: vec![KeybindingValue::KeycodeValue(Keycode::F2)],
            increase_speed: vec![KeybindingValue::KeycodeValue(Keycode::RightBracket)],
            decrease_speed: vec![KeybindingValue::KeycodeValue(Keycode::LeftBracket)],
        }
    }
}

fn get_default_data_path() -> PathBuf {
    let xdg_config_home = option_env!("XDG_DATA_HOME");
    let config_home_dir = format!("{}/{}", env!("HOME"), ".local/share");
    Path::new(xdg_config_home.unwrap_or(&config_home_dir)).join(env!("CARGO_PKG_NAME"))
}

fn parse_table(toml_value: Value, details: &str) -> super::Result<value::Table> {
    match toml_value {
        toml::Value::Table(table) => Ok(table),
        _ => Err(super::Error::from_description("parsing config", details)),
    }
}

fn parse_general_config(config: &mut Config, toml_value: Value) -> super::Result<()> {
    let toml_table = parse_table(toml_value, "Expected `general` to be a table.")?;
    for toml_entry in toml_table {
        match toml_entry.0.as_ref() {
            "data_path" => {
                config.data_path = Path::new(toml_entry.1.as_str().ok_or_else(|| {
                    super::Error::from_description(
                        "parsing config",
                        "Expected `data_path` to be a string.",
                    )
                })?)
                .to_owned();
            },
            _ => {
                return Err(super::Error::from_description(
                    "parsing config",
                    format!("Unexpected value in `general` table: {}.", toml_entry.0),
                ));
            },
        }
    }

    Ok(())
}

pub fn get_config_path<P>(config_path_opt: Option<P>) -> PathBuf
where
    P: AsRef<Path>,
{
    match config_path_opt {
        Some(config_path) => PathBuf::from(config_path.as_ref()),
        None => {
            let xdg_config_home = option_env!("XDG_CONFIG_HOME");
            let config_home_dir = format!("{}/{}", env!("HOME"), ".config");
            Path::new(xdg_config_home.unwrap_or(&config_home_dir))
                .join(env!("CARGO_PKG_NAME"))
                .join(format!("{}.toml", env!("CARGO_PKG_NAME")))
        },
    }
}

pub struct Config {
    pub data_path: PathBuf,
    pub keybindings_config: KeybindingsConfig,
    pub controller_configs: [ControllerConfig; 2],
}

impl Config {
    pub fn get_save_file<P>(&self, rom_path: P) -> PathBuf
    where
        P: AsRef<Path>,
    {
        let save_file_name = rom_path.as_ref().with_extension("sav");
        self.data_path.join(
            save_file_name
                .file_name()
                .expect("Expected valid file name."),
        )
    }

    pub fn get_save_state_file<P>(&self, rom_path: P) -> PathBuf
    where
        P: AsRef<Path>,
    {
        let save_state_file_name = rom_path.as_ref().with_extension("state");
        self.data_path.join(
            save_state_file_name
                .file_name()
                .expect("Expected valid file name."),
        )
    }

    pub fn parse_config<P>(config_path: P) -> super::Result<Config>
    where
        P: AsRef<Path>,
    {
        let mut config = Config {
            data_path: get_default_data_path(),
            keybindings_config: KeybindingsConfig::default(),
            controller_configs: [ControllerConfig::default(), ControllerConfig::default()],
        };

        if !config_path.as_ref().exists() {
            return Ok(config);
        }

        let config_file_buffer =
            fs::read(&config_path).map_err(|err| super::Error::new("reading config", &err))?;
        let toml_value = str::from_utf8(&config_file_buffer)
            .map_err(|err| super::Error::new("reading config", &err))?
            .parse::<toml::Value>()
            .map_err(|err| super::Error::new("parsing config", &err))?;
        let toml_table = parse_table(toml_value, "Expected table at root of config.")?;

        for toml_entry in toml_table {
            let (toml_key, toml_value) = toml_entry;
            match toml_key.as_ref() {
                "general" => parse_general_config(&mut config, toml_value)?,
                "keybindings" => {
                    config.keybindings_config = toml_value
                        .try_into::<KeybindingsConfig>()
                        .map_err(|err| super::Error::new("parsing keybindings config", &err))?;
                },
                "port-1" => {
                    config.controller_configs[0] = toml_value
                        .try_into::<ControllerConfig>()
                        .map_err(|err| super::Error::new("parsing port-1 config", &err))?
                },
                "port-2" => {
                    config.controller_configs[0] = toml_value
                        .try_into::<ControllerConfig>()
                        .map_err(|err| super::Error::new("parsing port-2 config", &err))?
                },
                _ => warn!("Unexpected value in root of config: {}.", toml_key),
            }
        }

        Ok(config)
    }
}
