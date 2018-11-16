#[macro_use]
extern crate clap;
extern crate neso;
extern crate sdl2;
extern crate simplelog;
#[macro_use]
extern crate serde_derive;
extern crate toml;

use clap::{App, Arg};
use neso::Nes;
use sdl2::audio::AudioSpecDesired;
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::PixelFormatEnum;
use sdl2::rect::Rect;
use simplelog::{CombinedLogger, Config, Level, LevelFilter, TermLogger};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use std::{error, fmt, fs, process, ptr, result, slice, thread};

const KEYS: [Keycode; 8] = [
    Keycode::Q,
    Keycode::W,
    Keycode::E,
    Keycode::R,
    Keycode::Up,
    Keycode::Down,
    Keycode::Left,
    Keycode::Right,
];

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

fn get_config_path(config_path_opt: Option<&str>) -> PathBuf {
    match config_path_opt {
        Some(config_path) => PathBuf::from(config_path),
        None => {
            let xdg_config_home = option_env!("XDG_CONFIG_HOME");
            let config_home_dir = format!("{}/{}", env!("HOME"), ".config");
            Path::new(xdg_config_home.unwrap_or(&config_home_dir))
                .join(env!("CARGO_PKG_NAME"))
                .join(format!("{}.toml", env!("CARGO_PKG_NAME")))
        }
    }
}

fn run() -> Result<()> {
    let logger_config = Config {
        time: Some(Level::Debug),
        level: Some(Level::Debug),
        target: None,
        location: None,
        time_format: None,
    };
    let term_logger = TermLogger::new(LevelFilter::Debug, logger_config).ok_or(
        Error::from_description("setting up logger", "Could not create `TermLogger`."),
    )?;
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
    let config_path = get_config_path(matches.value_of("config"));

    let mus_per_frame = Duration::from_micros((1.0f64 / 60.0 * 1e6).round() as u64);
    let buffer = fs::read(rom_path).map_err(|err| Error::new("reading ROM", &err))?;
    let mut nes = Nes::new();
    nes.load_rom(&buffer);

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
    let device = audio_subsystem
        .open_queue::<f32, _>(None, &desired_spec)
        .map_err(|err| Error::from_description("opening audio queue", err))?;

    canvas.present();
    device.resume();

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
                Event::Quit { .. }
                | Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                } => break 'running,
                Event::KeyDown {
                    keycode: Some(keycode),
                    ..
                } => {
                    if let Some(index) = KEYS.iter().position(|key| *key == keycode) {
                        nes.press_button(0, index as u8);
                        nes.press_button(1, index as u8);
                    } else if keycode == Keycode::T {
                        nes.reset();
                    }
                },
                Event::KeyUp {
                    keycode: Some(keycode),
                    ..
                } => {
                    if let Some(index) = KEYS.iter().position(|key| *key == keycode) {
                        nes.release_button(0, index as u8);
                        nes.release_button(1, index as u8);
                    }
                },
                _ => {},
            }
        }

        if matches.value_of("frames").is_none() {
            nes.step_frame();
        }

        let buffer_len = nes.audio_buffer_len();
        let slice = unsafe { slice::from_raw_parts(nes.audio_buffer(), buffer_len) };
        device.queue(&slice[..]);
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
            let mut pattern_table = Vec::with_capacity(512);
            for i in 0..8 {
                let chr_bank = unsafe { slice::from_raw_parts(nes.chr_bank(i), 0x400) };
                for j in 0..64 {
                    let mut tile = [0; 64];
                    for index in 0..8 {
                        let byte = chr_bank[j * 16 + index];
                        for y in 0..8 {
                            tile[index as usize * 8 + 7 - y] |=
                                if byte & 1 << y != 0 { 1 } else { 0 };
                        }
                    }
                    for index in 0..8 {
                        let byte = chr_bank[j * 16 + index + 8];
                        for y in 0..8 {
                            tile[index as usize * 8 + 7 - y] |=
                                if byte & 1 << y != 0 { 2 } else { 0 };
                        }
                    }
                    pattern_table.push(tile);
                }
            }
            for nametable in 0..4 {
                let nametable_bank =
                    unsafe { slice::from_raw_parts(nes.nametable_bank(nametable), 0x800) };
                let mut texture = texture_creator
                    .create_texture_streaming(PixelFormatEnum::RGB24, 256, 240)
                    .unwrap();
                texture
                    .with_lock(None, |buffer: &mut [u8], _pitch: usize| {
                        for i in 0..30usize {
                            for j in 0..32usize {
                                let index = nametable_bank[i * 32 + j] as usize
                                    + nes.background_chr_bank() * 64;
                                for x in 0..8usize {
                                    for y in 0..8usize {
                                        let offset = ((i * 8 + x) * 256 + j * 8 + y) * 3;
                                        let val = match pattern_table[index][x * 8 + y] {
                                            0 => 255,
                                            1 => 200,
                                            2 => 150,
                                            3 => 100,
                                            _ => panic!("ERROR"),
                                        };
                                        buffer[offset] = val;
                                        buffer[offset + 1] = val;
                                        buffer[offset + 2] = val;
                                    }
                                }
                            }
                        }
                    })
                    .unwrap();
                canvas
                    .copy(
                        &texture,
                        None,
                        Some(Rect::new(
                            240 * 2 + 240 * (nametable as i32 % 2),
                            256 * (nametable as i32 / 2),
                            240,
                            256,
                        )),
                    )
                    .unwrap();
            }
            let mut texture = texture_creator
                .create_texture_streaming(PixelFormatEnum::RGB24, 256, 128)
                .unwrap();
            texture
                .with_lock(None, |buffer: &mut [u8], _pitch: usize| {
                    for i in 0..16usize {
                        for j in 0..32usize {
                            for x in 0..8usize {
                                for y in 0..8usize {
                                    let row = (i * 2) % 16 + j / 16;
                                    let col = if i < 8 { j % 16 } else { j % 16 + 16 };
                                    let offset = ((row * 8 + x) * 256 + col * 8 + y) * 3;
                                    let val = match pattern_table[i * 32 + j][x * 8 + y] {
                                        0 => 255,
                                        1 => 200,
                                        2 => 150,
                                        3 => 100,
                                        _ => panic!("ERROR"),
                                    };
                                    buffer[offset] = val;
                                    buffer[offset + 1] = val;
                                    buffer[offset + 2] = val;
                                }
                            }
                        }
                    }
                })
                .unwrap();
            canvas
                .copy(
                    &texture,
                    None,
                    Some(Rect::new(0, 256 * 2, 256 * 2, 128 * 2)),
                )
                .unwrap();
            let palette = unsafe { slice::from_raw_parts(nes.palettes(), 32) };
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
        println!("{}", err);
        process::exit(1);
    }
}
