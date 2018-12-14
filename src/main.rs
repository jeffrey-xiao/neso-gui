mod config;
mod graphics;

use clap::{App, Arg};
use log::{error, info, warn};
use neso::Nes;
use sdl2::audio::AudioSpecDesired;
use sdl2::event::Event;
use sdl2::pixels::{Color, PixelFormatEnum};
use sdl2::rect::Rect;
use simplelog::{CombinedLogger, Level, LevelFilter, TermLogger};
use std::time::{Duration, Instant};
use std::{error, fmt, fs, process, ptr, result, slice, thread};

const SPEEDS: [f32; 9] = [
    1.0 / 2.0,
    1.0 / 1.75,
    1.0 / 1.5,
    1.0 / 1.25,
    1.0,
    1.25,
    1.5,
    1.75,
    2.00,
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

    fn cause(&self) -> Option<&dyn error::Error> {
        None
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Error in {} - {}", self.context, self.details)
    }
}

pub type Result<T> = result::Result<T, Error>;

struct EmulatorState {
    nes: Nes,
    config: config::Config,
    rom_path: String,
    is_muted: bool,
    is_paused: bool,
    is_running: bool,
    debug_enabled: bool,
    speed_index: usize,
}

impl EmulatorState {
    fn toggle_muted(&mut self) {
        self.is_muted = !self.is_muted;
        info!("[GUI] Is muted: {}.", self.is_muted);
    }

    fn toggle_paused(&mut self) {
        self.is_paused = !self.is_paused;
        info!("[GUI] Is paused: {}.", self.is_paused);
    }

    fn stop(&mut self) -> Result<()> {
        self.save()?;
        self.is_running = false;
        Ok(())
    }

    fn increase_speed(&mut self) {
        if self.speed_index < SPEEDS.len() - 1 {
            self.speed_index += 1;
        }
        info!("[GUI] Speed set to: {:.2}.", SPEEDS[self.speed_index]);
        self.reset_sample_freq();
    }

    fn decrease_speed(&mut self) {
        if self.speed_index > 0 {
            self.speed_index -= 1;
        }
        info!("[GUI] Speed set to: {:.2}.", SPEEDS[self.speed_index]);
        self.reset_sample_freq();
    }

    fn handle_button_press(&mut self, keybinding_value: config::KeybindingValue) -> Result<()> {
        for (port, controller_config) in self.config.controller_configs.iter().enumerate() {
            if let Some(index) = controller_config.keybinding_map.get(&keybinding_value) {
                self.nes.press_button(port, *index as u8);
            }
        }

        if self
            .config
            .keybindings_config
            .mute
            .contains(&keybinding_value)
        {
            self.toggle_muted();
        }

        if self
            .config
            .keybindings_config
            .pause
            .contains(&keybinding_value)
        {
            self.toggle_paused();
        }

        if self
            .config
            .keybindings_config
            .reset
            .contains(&keybinding_value)
        {
            self.nes.reset();
        }

        if self
            .config
            .keybindings_config
            .exit
            .contains(&keybinding_value)
        {
            self.stop()?;
        }

        if self
            .config
            .keybindings_config
            .save_state
            .contains(&keybinding_value)
        {
            self.save_state()?;
        }

        if self
            .config
            .keybindings_config
            .load_state
            .contains(&keybinding_value)
        {
            self.load_state()?;
        }

        if self
            .config
            .keybindings_config
            .increase_speed
            .contains(&keybinding_value)
        {
            self.increase_speed();
        }

        if self
            .config
            .keybindings_config
            .decrease_speed
            .contains(&keybinding_value)
        {
            self.decrease_speed();
        }

        Ok(())
    }

    fn handle_button_release(&mut self, keybinding_value: config::KeybindingValue) {
        for (port, controller_config) in self.config.controller_configs.iter().enumerate() {
            if let Some(index) = controller_config.keybinding_map.get(&keybinding_value) {
                self.nes.release_button(port, *index as u8);
            }
        }
    }

    fn save(&self) -> Result<()> {
        let data = self
            .nes
            .save()
            .map_err(|err| Error::new("getting save data", &err))?;
        let save_file_path = self.config.get_save_file(&self.rom_path);
        if let Some(data) = data {
            info!("[GUI] Writing save file at {:?}.", save_file_path);
            fs::create_dir_all(&self.config.data_path)
                .map_err(|err| Error::new("creating data directory: {}", &err))?;
            fs::write(save_file_path, &data)
                .map_err(|err| Error::new("writing save data", &err))?;
        }
        Ok(())
    }

    fn load(&mut self) -> Result<()> {
        let save_file_path = self.config.get_save_file(&self.rom_path);
        if save_file_path.exists() {
            info!("[GUI] Reading save file at {:?}.", save_file_path);
            let data =
                fs::read(save_file_path).map_err(|err| Error::new("reading save data", &err))?;
            self.nes
                .load(&data)
                .map_err(|err| Error::new("loading save data", &err))?;
        }
        Ok(())
    }

    fn save_state(&self) -> Result<()> {
        let data = self
            .nes
            .save_state()
            .map_err(|err| Error::new("getting save state data", &err))?;
        let save_state_file_path = self.config.get_save_state_file(&self.rom_path);
        info!(
            "[GUI] Writing save state file at {:?}.",
            save_state_file_path
        );
        fs::create_dir_all(&self.config.data_path)
            .map_err(|err| Error::new("creating data directory: {}", &err))?;
        fs::write(save_state_file_path, &data)
            .map_err(|err| Error::new("writing save state data", &err))?;
        Ok(())
    }

    fn load_state(&mut self) -> Result<()> {
        let save_state_file_path = self.config.get_save_state_file(&self.rom_path);
        if save_state_file_path.exists() {
            info!(
                "[GUI] Reading save state file at {:?}.",
                save_state_file_path
            );
            let data = fs::read(save_state_file_path)
                .map_err(|err| Error::new("reading save state data", &err))?;
            self.nes
                .load_state(&data)
                .map_err(|err| Error::new("loading save state data", &err))?;
        } else {
            warn!("No save state exists for this ROM.");
        }
        self.reset_sample_freq();
        Ok(())
    }

    fn mus_per_frame(&self) -> Duration {
        Duration::from_micros((1.0 / SPEEDS[self.speed_index] / 60.0 * 1e6).round() as u64)
    }

    fn reset_sample_freq(&mut self) {
        self.nes
            .set_sample_freq(44_100.0 / SPEEDS[self.speed_index]);
    }

    fn window_dimensions(&self) -> (u32, u32) {
        if self.debug_enabled {
            (1024, 736)
        } else {
            (512, 480)
        }
    }
}

fn run() -> Result<()> {
    let logger_config = simplelog::Config {
        time: Some(Level::Error),
        level: Some(Level::Error),
        target: None,
        location: None,
        time_format: None,
    };
    let term_logger = TermLogger::new(LevelFilter::Info, logger_config).ok_or_else(|| {
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

    let mut state = EmulatorState {
        nes: Nes::default(),
        config: config::Config::parse_config(config::get_config_path(matches.value_of("config")))?,
        rom_path: matches
            .value_of("rom-path")
            .expect("Expected `rom-path` to exist.")
            .to_owned(),
        is_muted: false,
        is_paused: matches.value_of("frames").is_some(),
        is_running: true,
        debug_enabled: matches.is_present("debug"),
        speed_index: 4,
    };
    state
        .nes
        .load_rom(&fs::read(&state.rom_path).map_err(|err| Error::new("reading ROM", &err))?);
    state.load()?;

    let sdl_context =
        sdl2::init().map_err(|err| Error::from_description("initializing `sdl2`", err))?;
    let video_subsystem = sdl_context
        .video()
        .map_err(|err| Error::from_description("initializing `sdl2` video subsystem", err))?;
    let audio_subsystem = sdl_context
        .audio()
        .map_err(|err| Error::from_description("initializing `sdl2` audio subsystem", err))?;
    let game_controller_subsystem = sdl_context.game_controller().map_err(|err| {
        Error::from_description("initializing `sdl2` game controller subsystem", err)
    })?;

    let available = game_controller_subsystem
        .num_joysticks()
        .map_err(|err| Error::from_description("enumerating joysticks", err))?;
    let mut _controller = None;
    for id in 0..available {
        if game_controller_subsystem.is_game_controller(id) {
            match game_controller_subsystem.open(id) {
                Ok(c) => {
                    info!("[GUI] Opened controller: {}", c.name());
                    _controller = Some(c);
                    break;
                },
                Err(err) => error!("[GUI] Failed to open controller: {}", err),
            }
        }
    }

    let (width, height) = state.window_dimensions();
    let window = video_subsystem
        .window("neso-gui", width, height)
        .position_centered()
        .opengl()
        .build()
        .map_err(|err| Error::new("building window", &err))?;

    let mut canvas = window
        .into_canvas()
        .build()
        .map_err(|err| Error::new("building canvas", &err))?;
    let texture_creator = canvas.texture_creator();
    canvas.present();
    canvas.set_draw_color(Color::RGB(255, 255, 255));

    let audio_queue = audio_subsystem
        .open_queue::<f32, _>(
            None,
            &AudioSpecDesired {
                freq: Some(44_100),
                channels: Some(1),
                samples: Some(1024),
            },
        )
        .map_err(|err| Error::from_description("opening audio queue", err))?;
    audio_queue.resume();

    let mut event_pump = sdl_context
        .event_pump()
        .map_err(|err| Error::from_description("obtaining `sdl` event pump", err))?;

    if let Some(frames) = matches.value_of("frames") {
        for _ in 0..frames
            .parse()
            .map_err(|err| Error::new("parsing frames", &err))?
        {
            state.nes.step_frame();
        }
    }

    while state.is_running {
        let start = Instant::now();

        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. } => {
                    state.stop()?;
                },
                Event::KeyDown {
                    keycode: Some(keycode),
                    ..
                } => {
                    let keybinding_value = config::KeybindingValue::KeycodeValue(keycode);
                    state.handle_button_press(keybinding_value)?;
                },
                Event::KeyUp {
                    keycode: Some(keycode),
                    ..
                } => {
                    let keybinding_value = config::KeybindingValue::KeycodeValue(keycode);
                    state.handle_button_release(keybinding_value);
                },
                Event::ControllerButtonDown { button, .. } => {
                    let keybinding_value = config::KeybindingValue::ButtonValue(button);
                    state.handle_button_press(keybinding_value)?;
                },
                Event::ControllerButtonUp { button, .. } => {
                    let keybinding_value = config::KeybindingValue::ButtonValue(button);
                    state.handle_button_release(keybinding_value);
                },
                _ => {},
            }
        }

        if !state.is_paused {
            state.nes.step_frame();
        }

        if !state.is_paused && !state.is_muted {
            let buffer_len = state.nes.audio_buffer_len();
            let slice = unsafe { slice::from_raw_parts(state.nes.audio_buffer(), buffer_len) };
            audio_queue.queue(&slice[0..buffer_len]);
        }

        let mut texture = texture_creator
            .create_texture_streaming(PixelFormatEnum::ABGR8888, 256, 240)
            .map_err(|err| Error::new("creating output texture", &err))?;
        texture
            .with_lock(None, |buffer: &mut [u8], _pitch: usize| unsafe {
                ptr::copy_nonoverlapping(
                    state.nes.image_buffer(),
                    buffer.as_mut_ptr(),
                    240 * 256 * 4,
                );
            })
            .map_err(|err| Error::from_description("locking output texture", err))?;
        canvas
            .copy(&texture, None, Some(Rect::new(0, 0, 256 * 2, 240 * 2)))
            .map_err(|err| Error::from_description("copying output texture to canvas", err))?;

        if state.debug_enabled {
            let debug_data = graphics::DebugData::new(&state.nes);

            let colors_rect = Rect::new(512, 480 + 16 * 4, 32 * 16, 32 * 4);
            canvas
                .copy(
                    &graphics::get_colors_texture(&texture_creator, &debug_data)?,
                    None,
                    Some(colors_rect),
                )
                .map_err(|err| Error::from_description("copying colors texture to canvas", err))?;
            canvas
                .draw_rect(colors_rect)
                .map_err(|err| Error::from_description("drawing colors border", err))?;

            let palettes_rect = Rect::new(512, 480 + 32 * 4 + 16 * 4, 32 * 16, 32 * 2);
            canvas
                .copy(
                    &graphics::get_palettes_texture(&texture_creator, &debug_data)?,
                    None,
                    Some(palettes_rect),
                )
                .map_err(|err| {
                    Error::from_description("copying palettes texture to canvas", err)
                })?;
            canvas
                .draw_rect(palettes_rect)
                .map_err(|err| Error::from_description("drawing palettes border", err))?;

            let oam_rect = Rect::new(512, 480, 16 * 32, 16 * 4);
            canvas
                .copy(
                    &graphics::get_oam_texture(&texture_creator, &debug_data)?,
                    None,
                    Some(oam_rect),
                )
                .map_err(|err| Error::from_description("copying oam texture to canvas", err))?;
            canvas
                .draw_rect(oam_rect)
                .map_err(|err| Error::from_description("drawing palettes border", err))?;

            for bank_index in 0..4 {
                canvas
                    .copy(
                        &graphics::get_nametable_texture(
                            &texture_creator,
                            &debug_data,
                            bank_index,
                        )?,
                        None,
                        Some(Rect::new(
                            512 + 256 * (bank_index as i32 % 2),
                            240 * (bank_index as i32 / 2),
                            256,
                            240,
                        )),
                    )
                    .map_err(|err| {
                        Error::from_description("copying nametable texture to canvas", err)
                    })?;
            }
            canvas
                .draw_rect(Rect::new(512, 0, 512, 480))
                .map_err(|err| Error::from_description("drawing nametables border", err))?;

            for table_index in 0..2 {
                canvas
                    .copy(
                        &graphics::get_pattern_table_texture(
                            &texture_creator,
                            &debug_data,
                            table_index,
                        )?,
                        None,
                        Some(Rect::new(table_index as i32 * 256, 480, 256, 256)),
                    )
                    .map_err(|err| {
                        Error::from_description("copying pattern table texture to canvas", err)
                    })?;
            }
        }

        canvas.present();

        let elapsed = start.elapsed();
        let mus_per_frame = state.mus_per_frame();
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
