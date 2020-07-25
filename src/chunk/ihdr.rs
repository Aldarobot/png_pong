// PNG Pong
//
// Copyright © 2019-2020 Jeron Aldaron Lau
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// https://apache.org/licenses/LICENSE-2.0>, or the Zlib License, <LICENSE-ZLIB
// or http://opensource.org/licenses/Zlib>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

use std::io::{Read, Write};

use crate::{
    checksum::CrcDecoder, consts, decode::Error as DecoderError,
    encode::Error as EncoderError,
};

/// Standard PNG color types.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ColorType {
    /// greyscale: 1, 2, 4, 8, 16 bit
    Grey = 0u8,
    /// RGB: 8, 16 bit
    Rgb = 2,
    /// palette: 1, 2, 4, 8 bit
    Palette = 3,
    /// greyscale with alpha: 8, 16 bit
    GreyAlpha = 4,
    /// RGB with alpha: 8, 16 bit
    Rgba = 6,
}

impl ColorType {
    /// channels * bytes per channel = bytes per pixel
    pub(crate) fn channels(self) -> u8 {
        match self {
            ColorType::Grey | ColorType::Palette => 1,
            ColorType::GreyAlpha => 2,
            ColorType::Rgb => 3,
            ColorType::Rgba => 4,
        }
    }

    /// get the total amount of bits per pixel, based on colortype and bitdepth
    /// in the struct
    pub(crate) fn bpp(self, bit_depth: u8) -> u8 {
        assert!(bit_depth >= 1 && bit_depth <= 16);
        /*bits per pixel is amount of channels * bits per channel*/
        let ch = self.channels();
        ch * if ch > 1 {
            if bit_depth == 8 {
                8
            } else {
                16
            }
        } else {
            bit_depth
        }
    }

    /// Error if invalid color type / bit depth combination for PNG.
    pub(crate) fn check_png_color_validity(
        self,
        bd: u8,
    ) -> Result<(), DecoderError> {
        match self {
            ColorType::Grey => {
                if !(bd == 1 || bd == 2 || bd == 4 || bd == 8 || bd == 16) {
                    return Err(DecoderError::ColorMode(self, bd));
                }
            }
            ColorType::Palette => {
                if !(bd == 1 || bd == 2 || bd == 4 || bd == 8) {
                    return Err(DecoderError::ColorMode(self, bd));
                }
            }
            ColorType::Rgb | ColorType::GreyAlpha | ColorType::Rgba => {
                if !(bd == 8 || bd == 16) {
                    return Err(DecoderError::ColorMode(self, bd));
                }
            }
        }
        Ok(())
    }
}

/// Image Header Chunk Data (IHDR)
#[derive(Copy, Clone, Debug)]
pub struct ImageHeader {
    /// Width of the image
    pub width: u32,
    /// Height of the image
    pub height: u32,
    /// The colortype of the image
    pub color_type: ColorType,
    /// How many bits per channel
    pub bit_depth: u8,
    /// True for adam7 interlacing, false for no interlacing.
    pub interlace: bool,
}

impl ImageHeader {
    pub(crate) fn write<W: Write>(
        &self,
        writer: &mut W,
    ) -> Result<(), EncoderError> {
        let mut header = Vec::new();
        super::encode_u32(&mut header, self.width)?;
        super::encode_u32(&mut header, self.height)?;
        super::encode_u8(&mut header, self.bit_depth)?;
        super::encode_u8(&mut header, self.color_type as u8)?;
        super::encode_u8(&mut header, 0u8)?;
        super::encode_u8(&mut header, 0u8)?;
        super::encode_u8(&mut header, self.interlace as u8)?;

        super::encode_chunk(writer, consts::IMAGE_HEADER, &header)
    }

    pub(crate) fn read<R: Read>(
        reader: &mut R,
    ) -> Result<(Self, u32), DecoderError> {
        let mut chunk = CrcDecoder::new(reader, consts::IMAGE_HEADER);

        // Read file
        let width = chunk.u32()?;
        let height = chunk.u32()?;
        if width == 0 || height == 0 {
            return Err(DecoderError::ImageDimensions);
        }
        let bit_depth = chunk.u8()?;
        if bit_depth == 0 || bit_depth > 16 {
            return Err(DecoderError::BitDepth(bit_depth));
        }
        let color_type = match chunk.u8()? {
            0 => ColorType::Grey,
            2 => ColorType::Rgb,
            3 => ColorType::Palette,
            4 => ColorType::GreyAlpha,
            6 => ColorType::Rgba,
            c => return Err(DecoderError::ColorType(c)),
        };
        color_type.check_png_color_validity(bit_depth)?;
        if chunk.u8()? != 0 {
            /*error: only compression method 0 is allowed in the specification*/
            return Err(DecoderError::CompressionMethod);
        }
        if chunk.u8()? != 0 {
            /*error: only filter method 0 is allowed in the specification*/
            return Err(DecoderError::FilterMethod);
        }
        let interlace = match chunk.u8()? {
            0 => false,
            1 => true,
            _ => return Err(DecoderError::InterlaceMethod),
        };
        // Success
        Ok((
            ImageHeader {
                width,
                height,
                color_type,
                bit_depth,
                interlace,
            },
            chunk.end()?,
        ))
    }

    /// get the total amount of bits per pixel, based on colortype and bitdepth
    /// in the struct
    pub(crate) fn bpp(&self) -> u8 {
        self.color_type.bpp(self.bit_depth) /*4 or 6*/
    }

    /// Returns the byte size of a raw image buffer with given width, height and
    /// color mode
    pub(crate) fn raw_size(&self) -> usize {
        /*will not overflow for any color type if roughly w * h < 268435455*/
        let bpp = self.bpp() as usize;
        let n = self.width as usize * self.height as usize;
        ((n / 8) * bpp) + ((n & 7) * bpp + 7) / 8
    }
}
