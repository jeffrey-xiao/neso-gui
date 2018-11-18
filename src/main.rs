extern crate clap;
extern crate neso;
extern crate sdl2;
#[macro_use]
extern crate log;
extern crate serde_derive;
extern crate simplelog;
extern crate toml;

mod config;

use clap::{App, Arg};
use neso::Nes;
use sdl2::audio::AudioSpecDesired;
use sdl2::event::Event;
use sdl2::pixels::PixelFormatEnum;
use sdl2::rect::Rect;
use sdl2::render::{Texture, TextureCreator};
use std::path::Path;
use sdl2::video::WindowContext;
use simplelog::{CombinedLogger, Level, LevelFilter, TermLogger};
use std::time::{Duration, Instant};
use std::{error, fmt, fs, process, ptr, result, slice, thread};

#[derive(Debug)]
pub struct Error {
    context: String,
    description: String,
    details: String,
}

impl Error {
    pub fn new<T, U>(context: T, error: &U) -> Self
    where
        T: Into<String>,
        U: error::Error,
    {
        Error {
            context: context.into(),
            description: error.description().into(),
            details: error.to_string(),
        }
    }

    pub fn from_description<T, U>(context: T, details: U) -> Self
    where
        T: Into<String>,
        U: Into<String>,
    {
        Error {
            context: context.into(),
            description: "a custom error".into(),
            details: details.into(),
        }
    }
}

impl error::Error for Error {
    fn description(&self) -> &str {
        &self.description
    }

    fn cause(&self) -> Option<&error::Error> {
        None
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Error in {} - {}", self.context, self.details)
    }
}

pub type Result<T> = result::Result<T, Error>;

fn get_pattern_table_texture<'a>(
    texture_creator: &'a TextureCreator<WindowContext>,
    chr_banks: &[&[u8]],
    offset: usize,
) -> Result<Texture<'a>> {
    let mut texture = texture_creator
        .create_texture_streaming(PixelFormatEnum::RGB24, 128, 128)
        .map_err(|err| Error::new("creating pattern table texture", &err))?;
    texture
        .with_lock(None, |buffer: &mut [u8], _pitch: usize| {
            for row in 0..16 {
                for i in 0..8 {
                    for col in 0..16 {
                        let byte_index = (row * 16 * 8 + col * 8) * 2 + i + offset;
                        for j in 0..8 {
                            let bank_index = byte_index / 0x400;
                            let bank_offset = byte_index % 0x400;
                            let val = (chr_banks[bank_index][bank_offset] >> (7 - j)) & 0x01
                                | ((chr_banks[bank_index][bank_offset + 8] >> (7 - j)) & 0x01) << 1;
                            let val = match val {
                                0 => 255,
                                1 => 200,
                                2 => 150,
                                3 => 100,
                                _ => panic!("Should never happen."),
                            };
                            let buffer_index = (row * 16 * 8 * 8 + i * 16 * 8 + col * 8 + j) * 3;
                            buffer[buffer_index] = val;
                            buffer[buffer_index + 1] = val;
                            buffer[buffer_index + 2] = val;
                        }
                    }
                }
            }
        })
        .map_err(|err| Error::from_description("locking pattern table texture", err))?;
    Ok(texture)
}

fn get_nametable_texture<'a>(
    texture_creator: &'a TextureCreator<WindowContext>,
    chr_banks: &[&[u8]],
    nametable_bank: &[u8],
) -> Result<Texture<'a>> {
    let mut texture = texture_creator
        .create_texture_streaming(PixelFormatEnum::RGB24, 256, 240)
        .map_err(|err| Error::new("creating pattern table texture", &err))?;
    texture
        .with_lock(None, |buffer: &mut [u8], _pitch: usize| {
            for row in 0..30 {
                for i in 0..8 {
                    for col in 0..32 {
                        let byte_index = (nametable_bank[row * 32 + col] as usize) * 16 + i;
                        for j in 0..8 {
                            let bank_index = byte_index / 0x400;
                            let bank_offset = byte_index % 0x400;
                            let val = (chr_banks[bank_index][bank_offset] >> (7 - j)) & 0x01
                                | (chr_banks[bank_index][bank_offset + 8] >> (7 - j) & 0x01) << 1;
                            let val = match val {
                                0 => 255,
                                1 => 200,
                                2 => 150,
                                3 => 100,
                                _ => panic!("Should never happen."),
                            };
                            let buffer_index = (row * 8 * 32 * 8 + i * 32 * 8 + col * 8 + j) * 3;
                            buffer[buffer_index] = val;
                            buffer[buffer_index + 1] = val;
                            buffer[buffer_index + 2] = val;
                        }
                    }
                }
            }
        })
        .map_err(|err| Error::from_description("locking nametable texture", err))?;
    Ok(texture)
}

fn save<P>(config: &config::Config, nes: &Nes, rom_path: P) -> Result<()>
where
    P: AsRef<Path>,
{
    let data = nes.save().map_err(|err| Error::new("getting save data", &err))?;
    let save_file_path = config.get_save_file(rom_path);
    if let Some(data) = data {
        info!("[GUI] Writing save file at {:?}.", save_file_path);
        fs::create_dir_all(&config.data_path).map_err(|err| Error::new("creating data directory: {}", &err))?;
        fs::write(save_file_path, &data).map_err(|err| Error::new("writing save data", &err))?;
    }
    Ok(())
}

fn load<P>(config: &config::Config, nes: &mut Nes, rom_path: P) -> Result<()>
where
    P: AsRef<Path>,
{
    let save_file_path = config.get_save_file(rom_path);
    if save_file_path.exists() {
        info!("[GUI] Reading save file at {:?}.", save_file_path);
        let data = fs::read(save_file_path).map_err(|err| Error::new("reading save data", &err))?;
        nes.load(&data).map_err(|err| Error::new("loading save data", &err))?;
    }
    Ok(())
}

fn save_state<P>(config: &config::Config, nes: &Nes, rom_path: P) -> Result<()>
where
    P: AsRef<Path>,
{
    let data = nes.save_state().map_err(|err| Error::new("getting save state data", &err))?;
    let save_state_file_path = config.get_save_state_file(rom_path);
    info!("[GUI] Writing save state file at {:?}.", save_state_file_path);
    fs::create_dir_all(&config.data_path).map_err(|err| Error::new("creating data directory: {}", &err))?;
    fs::write(save_state_file_path, &data).map_err(|err| Error::new("writing save state data", &err))?;
    Ok(())
}

fn load_state<P>(config: &config::Config, nes: &mut Nes, rom_path: P) -> Result<()>
where
    P: AsRef<Path>,
{
    let save_state_file_path = config.get_save_state_file(rom_path);
    if save_state_file_path.exists() {
        info!("[GUI] Reading save state file at {:?}.", save_state_file_path);
        let data = fs::read(save_state_file_path).map_err(|err| Error::new("reading save state data", &err))?;
        nes.load_state(&data).map_err(|err| Error::new("loading save state data", &err))?;
    } else {
        warn!("No save state exists for this ROM.");
    }
    Ok(())
}

fn run() -> Result<()> {
    let logger_config = simplelog::Config {
        time: Some(Level::Error),
        level: Some(Level::Error),
        target: None,
        location: None,
        time_format: None,
    };
    let term_logger = TermLogger::new(LevelFilter::Debug, logger_config).ok_or_else(|| {
        Error::from_description("setting up logger", "Could not create `TermLogger`.")
    })?;
    CombinedLogger::init(vec![term_logger]).map_err(|err| Error::new("setting up logger", &err))?;

    let matches = App::new("neso-gui")
        .version(env!("CARGO_PKG_VERSION"))
        .author("Jeffrey Xiao <jeffrey.xiao1998@gmail.com>")
        .about("A NES emulator built with Rust and sdl2.")
        .arg(
            Arg::with_name("rom-path")
                .help("Path to rom.")
                .index(1)
                .required(true),
        )
        .arg(
            Arg::with_name("config")
                .help("Path to configuration file.")
                .takes_value(true)
                .short("c")
                .long("config"),
        )
        .arg(
            Arg::with_name("debug")
                .help("Enable debug views.")
                .short("d")
                .long("debug"),
        )
        .arg(
            Arg::with_name("frames")
                .help("Number of frames to run.")
                .short("f")
                .long("frames")
                .takes_value(true),
        )
        .get_matches();

    let debug_enabled = matches.is_present("debug");
    let rom_path = matches
        .value_of("rom-path")
        .expect("Expected `rom-path` to exist.");
    let config_path = config::get_config_path(matches.value_of("config"));
    let config = config::Config::parse_config(config_path)?;
    let mut is_muted = false;

    let mus_per_frame = Duration::from_micros((1.0f64 / 60.0 * 1e6).round() as u64);
    let mut nes = Nes::new();
    nes.load_rom(&fs::read(rom_path).map_err(|err| Error::new("reading ROM", &err))?);
    load(&config, &mut nes, &rom_path)?;

    let sdl_context =
        sdl2::init().map_err(|err| Error::from_description("initializing `sdl2`", err))?;
    let video_subsystem = sdl_context
        .video()
        .map_err(|err| Error::from_description("initializing `sdl2` video subsystem", err))?;
    let audio_subsystem = sdl_context
        .audio()
        .map_err(|err| Error::from_description("initializing `sdl2` audio subsystem", err))?;

    let window_dimensions = if debug_enabled {
        (960, 736)
    } else {
        (480, 512)
    };

    let window = video_subsystem
        .window("neso-gui", window_dimensions.0, window_dimensions.1)
        .position_centered()
        .opengl()
        .build()
        .map_err(|err| Error::new("building window", &err))?;
    let mut canvas = window
        .into_canvas()
        .build()
        .map_err(|err| Error::new("building canvas", &err))?;
    let texture_creator = canvas.texture_creator();

    let desired_spec = AudioSpecDesired {
        freq: Some(44_100),
        channels: Some(1),
        samples: Some(1024),
    };
    let audio_queue = audio_subsystem
        .open_queue::<f32, _>(None, &desired_spec)
        .map_err(|err| Error::from_description("opening audio queue", err))?;

    canvas.present();
    audio_queue.resume();

    let mut event_pump = sdl_context
        .event_pump()
        .map_err(|err| Error::from_description("obtaining `sdl` event pump", err))?;

    if let Some(frames) = matches.value_of("frames") {
        for _ in 0..frames.parse().unwrap() {
            nes.step_frame();
        }
    }

    let colors = unsafe { slice::from_raw_parts(nes.colors(), 64) };

    'running: loop {
        let start = Instant::now();
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. } => {
                    save(&config, &nes, rom_path)?;
                    break 'running;
                }
                Event::KeyDown {
                    keycode: Some(keycode),
                    ..
                } => {
                    for (port, controller_config) in config.controller_configs.iter().enumerate() {
                        if let Some(index) = controller_config.keycode_map.get(&keycode) {
                            nes.press_button(port, *index as u8);
                        }
                    }

                    if config.keybindings_config.reset.contains(&keycode) {
                        nes.reset();
                    }

                    if config.keybindings_config.exit.contains(&keycode) {
                        save(&config, &nes, rom_path)?;
                        break 'running;
                    }

                    if config.keybindings_config.mute.contains(&keycode) {
                        is_muted = !is_muted;
                    }

                    if config.keybindings_config.save_state.contains(&keycode) {
                        save_state(&config, &nes, rom_path)?;
                    }

                    if config.keybindings_config.load_state.contains(&keycode) {
                        load_state(&config, &mut nes, rom_path)?;
                    }
                },
                Event::KeyUp {
                    keycode: Some(keycode),
                    ..
                } => {
                    for (port, controller_config) in config.controller_configs.iter().enumerate() {
                        if let Some(index) = controller_config.keycode_map.get(&keycode) {
                            nes.release_button(port, *index as u8);
                        }
                    }
                },
                _ => {},
            }
        }

        if matches.value_of("frames").is_none() {
            nes.step_frame();
        }

        if !is_muted {
            let buffer_len = nes.audio_buffer_len();
            let slice = unsafe { slice::from_raw_parts(nes.audio_buffer(), buffer_len) };
            audio_queue.queue(&slice[0..buffer_len]);
        }

        canvas.clear();
        let mut texture = texture_creator
            .create_texture_streaming(PixelFormatEnum::ABGR8888, 256, 240)
            .unwrap();
        texture
            .with_lock(None, |buffer: &mut [u8], _pitch: usize| {
                unsafe {
                    ptr::copy_nonoverlapping(
                        nes.image_buffer(),
                        buffer.as_mut_ptr(),
                        256 * 240 * 4,
                    );
                }
            })
            .unwrap();
        canvas
            .copy(&texture, None, Some(Rect::new(0, 0, 240 * 2, 256 * 2)))
            .unwrap();
        if debug_enabled {
            let palette = unsafe { slice::from_raw_parts(nes.palettes(), 32) };
            let mut chr_banks = Vec::with_capacity(8);
            for bank_index in 0..8 {
                chr_banks.push(unsafe { slice::from_raw_parts(nes.chr_bank(bank_index), 0x400) });
            }

            for bank_index in 0..4 {
                let nametable_bank =
                    unsafe { slice::from_raw_parts(nes.nametable_bank(bank_index), 0x800) };
                canvas
                    .copy(
                        &get_nametable_texture(&texture_creator, &chr_banks[4..], nametable_bank)?,
                        None,
                        Some(Rect::new(
                            240 * 2 + 240 * (bank_index as i32 % 2),
                            256 * (bank_index as i32 / 2),
                            240,
                            256,
                        )),
                    )
                    .map_err(|err| Error::from_description("copying texture to canvas", err))?;
            }

            for table_index in 0..2 {
                canvas
                    .copy(
                        &get_pattern_table_texture(
                            &texture_creator,
                            &chr_banks,
                            table_index * 0x1000,
                        )?,
                        None,
                        Some(Rect::new(
                            table_index as i32 * 256,
                            256 * 2,
                            128 * 2,
                            128 * 2,
                        )),
                    )
                    .map_err(|err| Error::from_description("copying texture to canvas", err))?;
            }

            let mut texture = texture_creator
                .create_texture_streaming(PixelFormatEnum::RGB24, 16, 2)
                .unwrap();
            texture
                .with_lock(None, |buffer: &mut [u8], _pitch: usize| {
                    for i in 0..32 {
                        let color_index = palette[if i % 4 == 0 { 0 } else { i }] as usize;
                        buffer[i * 3 + 2] = (colors[color_index] & 0xFF) as u8;
                        buffer[i * 3 + 1] = ((colors[color_index] >> 8) & 0xFF) as u8;
                        buffer[i * 3] = ((colors[color_index] >> 16) & 0xFF) as u8;
                    }
                })
                .unwrap();
            canvas
                .copy(
                    &texture,
                    None,
                    Some(Rect::new(256 * 2, 256 * 2, 160 * 2, 20 * 2)),
                )
                .unwrap();
        }

        canvas.present();

        let elapsed = start.elapsed();
        if mus_per_frame > elapsed {
            thread::sleep(mus_per_frame - elapsed);
        }
    }
    Ok(())
}

pub fn main() {
    if let Err(err) = run() {
        error!("{}", err);
        process::exit(1);
    }
}
