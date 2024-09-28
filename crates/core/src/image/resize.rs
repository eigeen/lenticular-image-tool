use std::num::NonZero;

use fast_image_resize::{FilterType, Image, PixelType, ResizeAlg, Resizer};

use crate::error::{Error, Result};

pub fn resize_cmyk8(
    src: Vec<u8>,
    width: u32,
    height: u32,
    out_width: u32,
    out_height: u32,
) -> Result<Vec<u8>> {
    let input_height =
        NonZero::new(height).ok_or(Error::InvalidInput("height cannot be zero".to_string()))?;
    let input_width =
        NonZero::new(width).ok_or(Error::InvalidInput("height cannot be zero".to_string()))?;
    let output_height =
        NonZero::new(out_height).ok_or(Error::InvalidInput("height cannot be zero".to_string()))?;
    let output_width =
        NonZero::new(out_width).ok_or(Error::InvalidInput("height cannot be zero".to_string()))?;

    let src_image = Image::from_vec_u8(input_width, input_height, src, PixelType::U8x4)?;

    let mut dst_image = Image::new(output_width, output_height, PixelType::U8x4);
    let mut dst_view = dst_image.view_mut();

    let mut resizer = Resizer::new(ResizeAlg::Convolution(FilterType::Lanczos3));
    resizer.resize(&src_image.view(), &mut dst_view)?;

    Ok(dst_image.buffer().to_vec())
}
