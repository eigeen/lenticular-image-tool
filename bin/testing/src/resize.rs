use std::num::NonZero;

use fast_image_resize::{Image, PixelType, ResizeAlg, Resizer};

pub fn resize_cmyk8(
    src: Vec<u8>,
    width: u32,
    height: u32,
    out_width: u32,
    out_height: u32,
    alg: ResizeAlg,
) -> anyhow::Result<Vec<u8>> {
    let input_height = NonZero::new(height).unwrap();
    let input_width = NonZero::new(width).unwrap();
    let output_height = NonZero::new(out_height).unwrap();
    let output_width = NonZero::new(out_width).unwrap();

    let src_image = Image::from_vec_u8(input_width, input_height, src, PixelType::U8x4)?;

    let mut dst_image = Image::new(output_width, output_height, PixelType::U8x4);
    let mut dst_view = dst_image.view_mut();

    let mut resizer = Resizer::new(alg);
    resizer.resize(&src_image.view(), &mut dst_view)?;

    Ok(dst_image.buffer().to_vec())
}
