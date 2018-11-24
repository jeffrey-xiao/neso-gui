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
                            let mut val = (chr_banks[bank_index][bank_offset] >> (7 - j)) & 0x01
                                | ((chr_banks[bank_index][bank_offset + 8] >> (7 - j)) & 0x01) << 1;
                            val = 255 - 85 * val;
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
    colors: &[u32],
    palettes: &[u8],
    chr_banks: &[&[u8]],
    nametable_bank: &[u8],
) -> Result<Texture<'a>> {
    let (nametable, attribute_table) = nametable_bank.split_at(960);
    let mut texture = texture_creator
        .create_texture_streaming(PixelFormatEnum::RGB24, 256, 240)
        .map_err(|err| Error::new("creating pattern table texture", &err))?;
    texture
        .with_lock(None, |buffer: &mut [u8], _pitch: usize| {
            for row in 0..30 {
                for i in 0..8 {
                    for col in 0..32 {
                        let byte_index = (nametable[row * 32 + col] as usize) * 16 + i;
                        let attribute_table_index = (row / 4) * 8 + col / 4;
                        let attribute_table_shift = if (row / 2) % 2 == 0 { 0 } else { 4 }
                            | if (col / 2) % 2 == 0 { 0 } else { 2 };
                        let palette_index = (attribute_table[attribute_table_index]
                            >> attribute_table_shift)
                            & 0x03;
                        for j in 0..8 {
                            let bank_index = byte_index / 0x400;
                            let bank_offset = byte_index % 0x400;
                            let val = (chr_banks[bank_index][bank_offset] >> (7 - j)) & 0x01
                                | (chr_banks[bank_index][bank_offset + 8] >> (7 - j) & 0x01) << 1;
                            let color_index = if val == 0 {
                                palettes[0] as usize
                            } else {
                                palettes[(palette_index * 4 + val) as usize] as usize
                            };
                            let buffer_index = (row * 8 * 32 * 8 + i * 32 * 8 + col * 8 + j) * 3;
                            buffer[buffer_index] = ((colors[color_index] >> 16) & 0xFF) as u8;
                            buffer[buffer_index + 1] = ((colors[color_index] >> 8) & 0xFF) as u8;
                            buffer[buffer_index + 2] = (colors[color_index] & 0xFF) as u8;
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
                let color_index = palettes[if i % 4 == 0 { 0 } else { i % 32 }] as usize;
                buffer[i * 3] = ((colors[color_index] >> 16) & 0xFF) as u8;
                buffer[i * 3 + 1] = ((colors[color_index] >> 8) & 0xFF) as u8;
                buffer[i * 3 + 2] = (colors[color_index] & 0xFF) as u8;
            }
        })
        .map_err(|err| Error::from_description("locking palettes texture", err))?;
    Ok(texture)
}
