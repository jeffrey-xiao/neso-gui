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

struct KeycodeValues(Vec<Keycode>);
struct KeycodeValuesVisitor(PhantomData<KeycodeValues>);

impl<'de> Visitor<'de> for KeycodeValuesVisitor {
    type Value = KeycodeValues;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("keycode string or list of keycode strings")
    }

    fn visit_str<E>(self, keycode_name: &str) -> Result<Self::Value, E>
    where
        E: Error,
    {
        let keycode = Keycode::from_name(keycode_name).ok_or_else(|| {
            E::invalid_value(
                Unexpected::Str(keycode_name),
                &"a string as a keycode name.",
            )
        })?;
        Ok(KeycodeValues(vec![keycode]))
    }

    fn visit_seq<S>(self, mut visitor: S) -> Result<Self::Value, S::Error>
    where
        S: SeqAccess<'de>,
    {
        let mut value = visitor.next_element::<String>()?;
        let mut keycodes = Vec::new();
        while let Some(keycode_name) = value {
            let keycode = Keycode::from_name(&keycode_name).ok_or_else(|| {
                S::Error::invalid_value(
                    Unexpected::Str(&keycode_name),
                    &"a string as a keycode name.",
                )
            })?;
            keycodes.push(keycode);
            value = visitor.next_element::<String>()?;
        }
        Ok(KeycodeValues(keycodes))
    }
}

impl<'de> Deserialize<'de> for KeycodeValues {
    fn deserialize<D>(deserializer: D) -> Result<KeycodeValues, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(KeycodeValuesVisitor(PhantomData))
    }
}

struct RawKeycodeConfig(HashMap<String, KeycodeValues>);

impl<'de> Deserialize<'de> for RawKeycodeConfig {
    fn deserialize<D>(deserializer: D) -> Result<RawKeycodeConfig, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(RawKeycodeConfig(Deserialize::deserialize(deserializer)?))
    }
}

pub struct ControllerConfig {
    pub keycode_map: HashMap<Keycode, usize>,
}

impl ControllerConfig {
    fn new(keycode_map: HashMap<Keycode, usize>) -> Self {
        ControllerConfig { keycode_map }
    }
}

impl<'de> Deserialize<'de> for ControllerConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let mut controller_config = ControllerConfig::new(HashMap::new());

        let mut raw_keycode_config = RawKeycodeConfig(
            vec![
                ("a".to_string(), KeycodeValues(vec![Keycode::P])),
                ("b".to_string(), KeycodeValues(vec![Keycode::O])),
                (
                    "select".to_string(),
                    KeycodeValues(vec![Keycode::LShift, Keycode::RShift]),
                ),
                ("start".to_string(), KeycodeValues(vec![Keycode::Return])),
                ("up".to_string(), KeycodeValues(vec![Keycode::W])),
                ("down".to_string(), KeycodeValues(vec![Keycode::S])),
                ("left".to_string(), KeycodeValues(vec![Keycode::A])),
                ("right".to_string(), KeycodeValues(vec![Keycode::D])),
            ]
            .into_iter()
            .collect(),
        );
        raw_keycode_config
            .0
            .extend(RawKeycodeConfig::deserialize(deserializer)?.0.into_iter());

        for entry in raw_keycode_config.0 {
            match CONTROLLER_FIELDS
                .iter()
                .position(|field| **field == entry.0)
            {
                Some(index) => {
                    for keycode in (entry.1).0 {
                        controller_config.keycode_map.insert(keycode, index);
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
        let keycode_map = vec![
            (Keycode::P, 0),
            (Keycode::O, 1),
            (Keycode::RShift, 2),
            (Keycode::LShift, 2),
            (Keycode::Return, 3),
            (Keycode::W, 4),
            (Keycode::S, 5),
            (Keycode::A, 6),
            (Keycode::D, 7),
        ]
        .into_iter()
        .collect();

        ControllerConfig { keycode_map }
    }
}

pub struct KeybindingsConfig {
    pub exit: Vec<Keycode>,
    pub mute: Vec<Keycode>,
    pub reset: Vec<Keycode>,
}

impl<'de> Deserialize<'de> for KeybindingsConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let mut keybindings_config = KeybindingsConfig::default();
        let raw_keycode_config = RawKeycodeConfig::deserialize(deserializer)?;

        for entry in raw_keycode_config.0 {
            match entry.0.as_ref() {
                "exit" => keybindings_config.exit = (entry.1).0,
                "mute" => keybindings_config.mute = (entry.1).0,
                "reset" => keybindings_config.reset = (entry.1).0,
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
            reset: vec![Keycode::R],
            mute: vec![Keycode::M],
            exit: vec![Keycode::Escape],
        }
    }
}

pub struct Config {
    pub data_path: PathBuf,
    pub keybindings_config: KeybindingsConfig,
    pub controller_configs: [ControllerConfig; 2],
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

fn get_default_data_path() -> PathBuf {
    let xdg_config_home = option_env!("XDG_DATA_HOME");
    let config_home_dir = format!("{}/{}", env!("HOME"), ".local/share");
    Path::new(xdg_config_home.unwrap_or(&config_home_dir))
        .join(env!("CARGO_PKG_NAME"))
        .join(format!("{}.toml", env!("CARGO_PKG_NAME")))
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
