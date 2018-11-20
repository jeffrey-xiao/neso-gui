use super::{Error, Result};
use sdl2::pixels::PixelFormatEnum;
use sdl2::render::{Texture, TextureCreator};
use sdl2::video::WindowContext;

pub fn get_pattern_table_texture<'a>(
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

pub fn get_nametable_texture<'a>(
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

pub fn get_colors_texture<'a>(
    texture_creator: &'a TextureCreator<WindowContext>,
    colors: &[u32],
) -> Result<Texture<'a>> {
    let mut texture = texture_creator
        .create_texture_streaming(PixelFormatEnum::RGB24, 16, 4)
        .map_err(|err| Error::new("creating colors texture", &err))?;
    texture
        .with_lock(None, |buffer: &mut [u8], _pitch: usize| {
            for i in 0..64 {
                buffer[i * 3] = ((colors[i] >> 16) & 0xFF) as u8;
                buffer[i * 3 + 1] = ((colors[i] >> 8) & 0xFF) as u8;
                buffer[i * 3 + 2] = (colors[i] & 0xFF) as u8;
            }
        })
        .map_err(|err| Error::from_description("locking colors texture", err))?;
    Ok(texture)
}

pub fn get_palettes_texture<'a>(
    texture_creator: &'a TextureCreator<WindowContext>,
    colors: &[u32],
    palettes: &[u8],
) -> Result<Texture<'a>> {
    let mut texture = texture_creator
        .create_texture_streaming(PixelFormatEnum::RGB24, 16, 2)
        .map_err(|err| Error::new("creating palettes texture", &err))?;
    texture
        .with_lock(None, |buffer: &mut [u8], _pitch: usize| {
            for i in 0..32 {
                // Handle background color mirroring
                let color_index = palettes[if i % 4 == 0 { 0 } else { i % 16 }] as usize;
                buffer[i * 3] = ((colors[color_index] >> 16) & 0xFF) as u8;
                buffer[i * 3 + 1] = ((colors[color_index] >> 8) & 0xFF) as u8;
                buffer[i * 3 + 2] = (colors[color_index] & 0xFF) as u8;
            }
        })
        .map_err(|err| Error::from_description("locking palettes texture", err))?;
    Ok(texture)
}
