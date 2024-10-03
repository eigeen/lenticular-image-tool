use std::{fs::File, io::BufReader};

use fast_image_resize::ResizeAlg;
use img::{Cmyk8Color, MatrixImage};
use ndarray::Axis;
use tiff::{decoder::DecodingResult, encoder::colortype};

use log::{debug, warn};

mod img;
mod resize;

fn main() -> anyhow::Result<()> {
    env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .init();

    let inputs = vec![
        "testing/01.tif",
        "testing/02.tif",
        "testing/03.tif",
        "testing/04.tif",
        "testing/05.tif",
        "testing/06.tif",
        "testing/07.tif",
        "testing/08.tif",
        "testing/09.tif",
        "testing/10.tif",
        "testing/11.tif",
    ];

    let out_width = 2772;
    let out_height = 3967;
    let n_lines = out_width / 11;
    debug!("n_lines: {}", n_lines);
    let mut out_img: MatrixImage<Cmyk8Color> = MatrixImage::new(out_width, out_height);

    let mut input_imgs = vec![];
    for input in inputs {
        debug!("Reading input file: {}", input);

        let mut reader = BufReader::new(File::open(input)?);
        let mut decoder = tiff::decoder::Decoder::new(&mut reader)?;

        let DecodingResult::U8(img_res) = decoder.read_image()? else {
            return Err(anyhow::anyhow!("Unexpected decoding result"));
        };
        let (width, height) = decoder.dimensions()?;

        let resized_img = resize::resize_cmyk8(
            img_res,
            width,
            height,
            out_width,
            out_height,
            ResizeAlg::Nearest,
        )?;

        input_imgs.push(MatrixImage::<Cmyk8Color>::from_slice(
            &resized_img,
            out_width,
            out_height,
        )?);
    }

    let out_mat = out_img.matrix_mut();

    for col_index in 0..out_width {
        let img_index = col_index as usize % input_imgs.len();
        let img = input_imgs.get(img_index).unwrap();
        let img_mat = img.matrix();

        if col_index as usize >= img_mat.ncols() {
            warn!("Skipping column {} because it is out of bounds", col_index);
            continue;
        }
        let input_column = img_mat.column(col_index as usize);
        out_mat
            .index_axis_mut(Axis(1), col_index as usize)
            .assign(&input_column);
    }
    // let n_lines = out_height / 11;
    // for line in 0..n_lines {
    //     let img_index = line as usize % input_imgs.len();
    //     let img = input_imgs.get(img_index).unwrap();
    //     let img_mat = img.matrix();

    //     let col_start = line * 11;
    //     let col_end = col_start + 11;

    //     for i in col_start..col_end {
    //         if i as usize >= img_mat.ncols() {
    //             warn!("Skipping column {} because it is out of bounds", i);
    //             continue;
    //         }
    //         let input_column = img_mat.column(i as usize);
    //         out_mat
    //             .index_axis_mut(Axis(1), i as usize)
    //             .assign(&input_column);
    //     }
    // }

    debug!("Writing output file: testing/out.tif");
    let mut encoder = tiff::encoder::TiffEncoder::new(File::create("testing/out.tif")?)?;
    let out_tiff_img = encoder.new_image::<colortype::CMYK8>(out_width, out_height)?;
    out_tiff_img.write_data(&out_img.to_bytes())?;

    Ok(())
}

// fn v2() -> anyhow::Result<()> {
//     env_logger::builder()
//         .filter_level(log::LevelFilter::Debug)
//         .init();

//     let inputs = vec![
//         "testing/01.tif",
//         "testing/02.tif",
//         "testing/03.tif",
//         "testing/04.tif",
//         "testing/05.tif",
//         "testing/06.tif",
//         "testing/07.tif",
//         "testing/08.tif",
//         "testing/09.tif",
//         "testing/10.tif",
//         "testing/11.tif",
//     ];

//     let out_width = 2772;
//     let out_height = 3967;
//     let n_cols = out_width / 11;
//     debug!("n_cols: {}", n_cols);
//     let mut out_img: MatrixImage<Cmyk8Color> = MatrixImage::new(out_width, out_height);

//     let mut input_imgs = vec![];
//     for input in inputs {
//         debug!("Reading input file: {}", input);

//         let mut reader = BufReader::new(File::open(input)?);
//         let mut decoder = tiff::decoder::Decoder::new(&mut reader)?;

//         let DecodingResult::U8(img_res) = decoder.read_image()? else {
//             return Err(anyhow::anyhow!("Unexpected decoding result"));
//         };
//         let (width, height) = decoder.dimensions()?;

//         let resized_img = resize::resize_cmyk8(
//             img_res,
//             width,
//             height,
//             n_cols,
//             out_height,
//             ResizeAlg::Nearest,
//         )?;

//         input_imgs.push(MatrixImage::<Cmyk8Color>::from_slice(
//             &resized_img,
//             n_cols,
//             out_height,
//         )?);
//     }

//     let out_mat = out_img.matrix_mut();

//     for index_col in 0..n_cols {
//         let col_start = index_col * 11;
//         let col_end = col_start + 11;

//         let len = input_imgs.len();
//         let index_img = index_col as usize % len;

//         debug!("Processing column {} image: {}", index_col, index_img);
//         let input = input_imgs.get_mut(index_img).unwrap();
//         let input_mat = input.matrix_mut();

//         for i in col_start..col_end {
//             let group_size = 11 * len;
//             let group_index = i as usize / group_size;
//             let img_col_start: usize = group_index * 11 + i as usize % 11;

//             if img_col_start >= input_mat.ncols() {
//                 warn!("Skipping column {} because it is out of bounds", i);
//                 continue;
//             }
//             let input_column = input_mat.column(img_col_start);
//             out_mat
//                 .index_axis_mut(Axis(1), i as usize)
//                 .assign(&input_column);
//         }
//     }

//     debug!("Writing output file: testing/out.tif");
//     let mut encoder = tiff::encoder::TiffEncoder::new(File::create("testing/out.tif")?)?;
//     let out_tiff_img = encoder.new_image::<colortype::CMYK8>(out_width, out_height)?;
//     out_tiff_img.write_data(&out_img.to_bytes())?;

//     Ok(())
// }

// fn v1() -> anyhow::Result<()> {
//     env_logger::builder()
//         .filter_level(log::LevelFilter::Debug)
//         .init();

//     let inputs = vec![
//         "testing/01.tif",
//         "testing/02.tif",
//         "testing/03.tif",
//         "testing/04.tif",
//         "testing/05.tif",
//         "testing/06.tif",
//         "testing/07.tif",
//         "testing/08.tif",
//         "testing/09.tif",
//         "testing/10.tif",
//         "testing/11.tif",
//     ];

//     let out_width = 2772;
//     let out_height = 3967;
//     let mut out_img: MatrixImage<Cmyk8Color> = MatrixImage::new(out_width, out_height);

//     let mut input_imgs = vec![];
//     for input in inputs {
//         debug!("Reading input file: {}", input);

//         let mut reader = BufReader::new(File::open(input)?);
//         let mut decoder = tiff::decoder::Decoder::new(&mut reader)?;

//         let DecodingResult::U8(img_res) = decoder.read_image()? else {
//             return Err(anyhow::anyhow!("Unexpected decoding result"));
//         };
//         let (width, height) = decoder.dimensions()?;

//         let resized_img = resize::resize_cmyk8(
//             img_res,
//             width,
//             height,
//             out_width,
//             out_height,
//             ResizeAlg::Nearest,
//         )?;

//         input_imgs.push(MatrixImage::<Cmyk8Color>::from_slice(
//             &resized_img,
//             out_width,
//             out_height,
//         )?);
//     }

//     let out_mat = out_img.matrix_mut();

//     let n_cols = out_width / 11;
//     debug!("n_cols: {}", n_cols);

//     for index_col in 0..n_cols {
//         let col_start = index_col * 11;
//         let col_end = col_start + 11;

//         let len = input_imgs.len();
//         debug!(
//             "Processing column {} image: {}",
//             index_col,
//             index_col as usize % len
//         );
//         let input = input_imgs.get_mut(index_col as usize % len).unwrap();
//         let input_mat = input.matrix_mut();

//         for i in col_start..col_end {
//             let input_column = input_mat.column(i as usize);
//             out_mat
//                 .index_axis_mut(Axis(1), i as usize)
//                 .assign(&input_column);
//         }
//     }

//     debug!("Writing output file: testing/out.tif");
//     let mut encoder = tiff::encoder::TiffEncoder::new(File::create("testing/out.tif")?)?;
//     let out_tiff_img = encoder.new_image::<colortype::CMYK8>(out_width, out_height)?;
//     out_tiff_img.write_data(&out_img.to_bytes())?;

//     Ok(())
// }
