use super::{Error, Result};
use neso::Nes;
use sdl2::pixels::PixelFormatEnum;
use sdl2::render::{Texture, TextureCreator};
use sdl2::video::WindowContext;
use std::slice;

const CHR_BANK_SIZE: usize = 0x400;
const NAMETABLE_BANK_SIZE: usize = 0x800;
const PATTERN_TABLE_SIZE: usize = 0x1000;

pub struct DebugData<'a> {
    pub colors: &'a [u32],
    pub palettes: &'a [u8],
    pub chr_banks: Vec<&'a [u8]>,

    pub nametable_banks: Vec<&'a [u8]>,
    pub oam: &'a [u8],
    pub tall_sprites_enabled: bool,
    pub background_chr_bank: usize,
}

impl<'a> DebugData<'a> {
    pub fn new(nes: &Nes) -> DebugData<'a> {
        let mut chr_banks = Vec::with_capacity(8);
        for bank_index in 0..8 {
            chr_banks
                .push(unsafe { slice::from_raw_parts(nes.chr_bank(bank_index), CHR_BANK_SIZE) });
        }

        let mut nametable_banks = Vec::with_capacity(4);
        for bank_index in 0..4 {
            nametable_banks.push(unsafe {
                slice::from_raw_parts(nes.nametable_bank(bank_index), NAMETABLE_BANK_SIZE)
            });
        }

        DebugData {
            colors: unsafe { slice::from_raw_parts(nes.colors(), 64) },
            palettes: unsafe { slice::from_raw_parts(nes.palettes(), 32) },
            chr_banks,
            nametable_banks,
            oam: unsafe { slice::from_raw_parts(nes.object_attribute_memory(), 0x100) },
            tall_sprites_enabled: nes.tall_sprites_enabled(),
            background_chr_bank: nes.background_chr_bank(),
        }
    }
}

pub fn get_colors_texture<'a>(
    texture_creator: &'a TextureCreator<WindowContext>,
    d: &DebugData,
) -> Result<Texture<'a>> {
    let cols = 16;
    let rows = 4;
    let mut texture = texture_creator
        .create_texture_streaming(PixelFormatEnum::RGB24, cols as u32, rows as u32)
        .map_err(|err| Error::new("creating colors texture", &err))?;
    texture
        .with_lock(None, |buffer: &mut [u8], _pitch: usize| {
            for i in 0..rows * cols {
                buffer[i * 3] = ((d.colors[i] >> 16) & 0xFF) as u8;
                buffer[i * 3 + 1] = ((d.colors[i] >> 8) & 0xFF) as u8;
                buffer[i * 3 + 2] = (d.colors[i] & 0xFF) as u8;
            }
        })
        .map_err(|err| Error::from_description("locking colors texture", err))?;
    Ok(texture)
}

pub fn get_palettes_texture<'a>(
    texture_creator: &'a TextureCreator<WindowContext>,
    d: &DebugData,
) -> Result<Texture<'a>> {
    let cols = 16;
    let rows = 2;
    let mut texture = texture_creator
        .create_texture_streaming(PixelFormatEnum::RGB24, cols as u32, rows as u32)
        .map_err(|err| Error::new("creating palettes texture", &err))?;
    texture
        .with_lock(None, |buffer: &mut [u8], _pitch: usize| {
            for i in 0..rows * cols {
                // Handle background color mirroring
                let color_index = d.palettes[if i % 4 == 0 { 0 } else { i % 32 }] as usize;
                buffer[i * 3] = ((d.colors[color_index] >> 16) & 0xFF) as u8;
                buffer[i * 3 + 1] = ((d.colors[color_index] >> 8) & 0xFF) as u8;
                buffer[i * 3 + 2] = (d.colors[color_index] & 0xFF) as u8;
            }
        })
        .map_err(|err| Error::from_description("locking palettes texture", err))?;
    Ok(texture)
}

pub fn get_pattern_table_texture<'a>(
    texture_creator: &'a TextureCreator<WindowContext>,
    d: &DebugData,
    table_index: usize,
) -> Result<Texture<'a>> {
    let cols = 16;
    let rows = 16;
    let offset = table_index * PATTERN_TABLE_SIZE;
    let mut texture = texture_creator
        .create_texture_streaming(PixelFormatEnum::RGB24, cols as u32 * 8, rows as u32 * 8)
        .map_err(|err| Error::new("creating pattern table texture", &err))?;
    texture
        .with_lock(None, |buffer: &mut [u8], _pitch: usize| {
            for row in 0..rows {
                for i in 0..8 {
                    for col in 0..cols {
                        let byte_index = (row * cols + col) * 16 + i + offset;
                        for j in 0..8 {
                            let bank_index = byte_index / CHR_BANK_SIZE;
                            let bank_offset = byte_index % CHR_BANK_SIZE;
                            let mut val = (d.chr_banks[bank_index][bank_offset] >> (7 - j)) & 0x01
                                | ((d.chr_banks[bank_index][bank_offset + 8] >> (7 - j)) & 0x01)
                                    << 1;
                            val = 255 - 85 * val;
                            let buffer_index = (8 * cols * (row * 8 + i) + col * 8 + j) * 3;
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
    d: &DebugData,
    bank_index: usize,
) -> Result<Texture<'a>> {
    let cols = 32;
    let rows = 30;
    let (nametable, attribute_table) = d.nametable_banks[bank_index].split_at(cols * rows);
    let mut texture = texture_creator
        .create_texture_streaming(PixelFormatEnum::RGB24, cols as u32 * 8, rows as u32 * 8)
        .map_err(|err| Error::new("creating nametable texture", &err))?;
    texture
        .with_lock(None, |buffer: &mut [u8], _pitch: usize| {
            for row in 0..rows {
                for i in 0..8 {
                    for col in 0..cols {
                        let byte_index = (nametable[row * cols + col] as usize) * 16 + i;
                        let attribute_table_index = (row / 4) * 8 + col / 4;
                        let attribute_table_shift = if (row / 2) % 2 == 0 { 0 } else { 4 }
                            | if (col / 2) % 2 == 0 { 0 } else { 2 };
                        let palette_index = (attribute_table[attribute_table_index]
                            >> attribute_table_shift)
                            & 0x03;
                        for j in 0..8 {
                            let bank_index = byte_index / CHR_BANK_SIZE + d.background_chr_bank;
                            let bank_offset = byte_index % CHR_BANK_SIZE;
                            let val = (d.chr_banks[bank_index][bank_offset] >> (7 - j)) & 0x01
                                | (d.chr_banks[bank_index][bank_offset + 8] >> (7 - j) & 0x01) << 1;
                            let color_index = if val == 0 {
                                d.palettes[0] as usize
                            } else {
                                d.palettes[(palette_index * 4 + val) as usize] as usize
                            };
                            let buffer_index = (8 * cols * (row * 8 + i) + col * 8 + j) * 3;
                            buffer[buffer_index] = ((d.colors[color_index] >> 16) & 0xFF) as u8;
                            buffer[buffer_index + 1] = ((d.colors[color_index] >> 8) & 0xFF) as u8;
                            buffer[buffer_index + 2] = (d.colors[color_index] & 0xFF) as u8;
                        }
                    }
                }
            }
        })
        .map_err(|err| Error::from_description("locking nametable texture", err))?;
    Ok(texture)
}

pub fn get_oam_texture<'a>(
    texture_creator: &'a TextureCreator<WindowContext>,
    d: &DebugData,
) -> Result<Texture<'a>> {
    let cols = 32;
    let rows = 4;
    let mut texture = texture_creator
        .create_texture_streaming(PixelFormatEnum::RGB24, cols as u32 * 8, rows as u32 * 8)
        .map_err(|err| Error::new("creating oam texture", &err))?;
    texture
        .with_lock(None, |buffer: &mut [u8], _pitch: usize| {
            for s in 0..cols * rows / 2 {
                let row = s / cols;
                let col = s % cols;
                let (tiles, tile_index, pattern_table_addr) = {
                    let tile_index = d.oam[s * 4 + 1] as usize;
                    if d.tall_sprites_enabled {
                        let pattern_table_addr = (tile_index & 0x01) * PATTERN_TABLE_SIZE;
                        (2, tile_index & !0x01, pattern_table_addr)
                    } else {
                        let sprite_bank_index = (d.background_chr_bank + 4) % 8;
                        (1, tile_index, sprite_bank_index * CHR_BANK_SIZE)
                    }
                };
                let attributes = d.oam[s * 4 + 2];
                let palette = (attributes & 0x03) + 4;
                let flip_vert = attributes & 0x80 != 0;
                let flip_hori = attributes & 0x40 != 0;
                for t in 0..tiles {
                    for i in 0..8 {
                        for j in 0..8 {
                            let ci = if flip_vert { 7 - i } else { i };
                            let cj = if flip_hori { j } else { 7 - j };
                            let addr = pattern_table_addr + (tile_index | t) * 16 + ci;
                            let bank_index = addr / CHR_BANK_SIZE;
                            let bank_offset = addr % CHR_BANK_SIZE;
                            let val = (d.chr_banks[bank_index][bank_offset] >> cj) & 0x01
                                | (d.chr_banks[bank_index][bank_offset + 8] >> cj & 0x01) << 1;
                            let color_index = if val == 0 {
                                d.palettes[0] as usize
                            } else {
                                d.palettes[(palette * 4 + val) as usize] as usize
                            };
                            let buffer_index =
                                (8 * cols * ((2 * row + t) * 8 + i) + col * 8 + j) * 3;
                            buffer[buffer_index] = ((d.colors[color_index] >> 16) & 0xFF) as u8;
                            buffer[buffer_index + 1] = ((d.colors[color_index] >> 8) & 0xFF) as u8;
                            buffer[buffer_index + 2] = (d.colors[color_index] & 0xFF) as u8;
                        }
                    }
                }
            }
        })
        .map_err(|err| Error::from_description("locking oam texture", err))?;
    Ok(texture)
}
