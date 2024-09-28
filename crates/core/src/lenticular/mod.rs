pub mod tiff;

/// 全局选项
pub struct Options {
    pub lpi: f64,
    pub physical_width_cm: f64,
}

/// 针对每张图的选项
pub struct ImageOptions {
    pub lenticular_width_px: u32,
}

// /// 创建每行像素的索引映射
// ///
// /// num_cols: 列数（像素）
// ///
// /// lenticular_width_px: 每条光栅线的宽度（像素）
// ///
// /// num_images: 输入图像的数量
// ///
// /// img_index: 当前图像的索引值
// fn create_line_index_mapping(
//     num_cols: usize,
//     lenticular_width_px: usize,
//     num_images: usize,
//     img_index: usize,
// ) -> Vec<usize> {
//     let mut output = vec![0_usize; num_cols];

//     // 图像整体像素偏移（基于当前是第几张图）
//     let image_offset_px = img_index * lenticular_width_px;

//     (0..num_cols).for_each(|row_index| {
//         // 当前组索引
//         let group_index = row_index / lenticular_width_px;
//         // 组内像素偏移
//         let group_internal_offset_px = row_index % lenticular_width_px;
//         // 当前组像素偏移
//         let group_offset_px = group_index * num_images * lenticular_width_px;

//         let abs_index = group_internal_offset_px + group_offset_px + image_offset_px;

//         output[row_index] = abs_index;
//     });

//     output
// }

fn create_line_index_mapping_advanced(
    num_cols: usize,
    lenticular_width_map: &[usize],
    img_index: usize,
) -> Vec<usize> {
    // todo: 验证索引不得超过map尺寸
    let mut output = vec![0_usize; num_cols];

    // 当前图像的光栅线宽度
    let lenticular_width_px = lenticular_width_map[img_index];
    // 一组图像光栅线宽度之和
    let total_lenticular_width_px: usize = lenticular_width_map.iter().sum();
    // 图像整体像素偏移（基于当前是第几张图）
    // 当前图之前还有多少光栅线宽度
    let image_offset_px: usize = lenticular_width_map.iter().take(img_index).sum();

    (0..num_cols).for_each(|row_index| {
        // 当前组索引
        let group_index = row_index / lenticular_width_px;
        // 组内像素偏移
        let group_internal_offset_px = row_index % lenticular_width_px;
        // 当前组像素偏移
        let group_offset_px = group_index * total_lenticular_width_px;

        let abs_index = group_internal_offset_px + group_offset_px + image_offset_px;

        output[row_index] = abs_index;
    });

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_line_index_mapping_advanced() {
        // 均匀，2*3
        let result = create_line_index_mapping_advanced(12, &[3, 3], 0);
        eprintln!("result: {:?}", result);
        assert_eq!(result, [0, 1, 2, 6, 7, 8, 12, 13, 14, 18, 19, 20]);

        // 均匀，3*4，第二张图
        let result = create_line_index_mapping_advanced(12, &[4, 4, 4], 1);
        eprintln!("result: {:?}", result);
        assert_eq!(result, [4, 5, 6, 7, 16, 17, 18, 19, 28, 29, 30, 31]);

        // 不均匀，3+3+2，第一张图
        let result = create_line_index_mapping_advanced(12, &[3, 3, 2], 0);
        eprintln!("result: {:?}", result);
        assert_eq!(result, [0, 1, 2, 8, 9, 10, 16, 17, 18, 24, 25, 26]);
    }

    // #[test]
    // fn test_tiff() {
    //     let file = std::fs::File::open("../../input/光栅色纸测试图1.tif").unwrap();
    //     let reader = std::io::BufReader::new(file);

    //     // image1
    //     let mut decoder1 = tiff::decoder::Decoder::new(reader).unwrap();
    //     eprintln!("color type1: {:?}", decoder1.colortype().unwrap());
    //     let resolution_unit = decoder1
    //         .get_tag(Tag::ResolutionUnit)
    //         .unwrap()
    //         .into_u32()
    //         .unwrap();
    //     let x_resolution = decoder1.get_tag(Tag::XResolution).unwrap();
    //     let y_resolution = decoder1.get_tag(Tag::YResolution).unwrap();
    //     eprintln!(
    //         "resolution_unit: {}, x_resolution: {:?}, y_x_resolution: {:?}",
    //         resolution_unit, x_resolution, y_resolution
    //     );

    //     // image2
    //     let file = std::fs::File::open("../../input/光栅色纸测试图2.tif").unwrap();
    //     let reader = std::io::BufReader::new(file);
    //     let mut decoder2 = tiff::decoder::Decoder::new(reader).unwrap();
    //     eprintln!("color type2: {:?}", decoder2.colortype().unwrap());

    //     let mut params = Params::new(100.41, 10.6);
    //     params.set_lenticular_width_px(4);

    //     // 计算缩放目标
    //     let physical_width_in_per_px = 1.0 / (params.lpi * 4.0);
    //     let dpi = 1.0 / physical_width_in_per_px;
    //     eprintln!("dpi: {}", dpi);
    //     let target_height_px: u32 = (dpi * params.physical_width_in()).floor() as u32;
    //     eprintln!("target_height_px: {}", target_height_px);
    //     let dpi_width = dpi * 2.0;
    //     // let target_width_px: u32 = target_height_px * 2;
    //     // 暂时假设宽高一致
    //     let target_width_px: u32 = (dpi * params.physical_width_in()).floor() as u32;
    //     eprintln!("target_width_px: {}", target_width_px);

    //     // image1
    //     let DecodingResult::U8(img_res) = decoder1.read_image().unwrap() else {
    //         panic!("bad encoding");
    //     };
    //     let (width, height) = decoder1.dimensions().unwrap();
    //     // 等比例缩放
    //     let resized_res =
    //         resize_cmyk8(img_res, width, height, target_width_px, target_height_px).unwrap();
    //     let img1: MatrixImage<Cmyk8Color> =
    //         MatrixImage::from_slice(&resized_res, target_width_px, target_height_px).unwrap();

    //     // image2
    //     let DecodingResult::U8(img_res) = decoder2.read_image().unwrap() else {
    //         panic!("bad encoding");
    //     };
    //     let (width, height) = decoder2.dimensions().unwrap();
    //     // 等比例缩放
    //     let resized_res =
    //         resize_cmyk8(img_res, width, height, target_width_px, target_height_px).unwrap();
    //     let img2: MatrixImage<Cmyk8Color> =
    //         MatrixImage::from_slice(&resized_res, target_width_px, target_height_px).unwrap();

    //     let out_width = target_width_px * 2;
    //     let out_height = target_height_px;
    //     let mut out_img: MatrixImage<Cmyk8Color> = MatrixImage::new(out_width, out_height);

    //     let output_mat = out_img.inner_mut();

    //     // 第一张图
    //     {
    //         let input_mat = img1.inner();
    //         let (_, input_cols) = input_mat.dim();
    //         let (_, output_cols) = output_mat.dim();

    //         // 确保输出图像的列数是输入图像的两倍
    //         assert_eq!(output_cols, input_cols * 2);

    //         let col_mapping = create_line_index_mapping(input_cols, 4, 2, 0);

    //         (0..input_cols).for_each(|col_index| {
    //             let target_index = col_mapping[col_index];

    //             let input_column = input_mat.column(col_index);
    //             output_mat
    //                 .index_axis_mut(Axis(1), target_index)
    //                 .assign(&input_column);
    //         });
    //     }

    //     // 第二张图
    //     {
    //         let input_mat = img2.inner();
    //         let (_, input_cols) = input_mat.dim();
    //         let (_, output_cols) = output_mat.dim();

    //         // 确保输出图像的列数是输入图像的两倍
    //         assert_eq!(output_cols, input_cols * 2);

    //         let col_mapping = create_line_index_mapping(input_cols, 4, 2, 1);

    //         (0..input_cols).for_each(|col_index| {
    //             let target_index = col_mapping[col_index];

    //             let input_column = input_mat.column(col_index);
    //             output_mat
    //                 .index_axis_mut(Axis(1), target_index)
    //                 .assign(&input_column);
    //         });
    //     }

    //     let output_bytes = out_img.into_bytes();

    //     let mut out_writer = std::fs::OpenOptions::new()
    //         .create(true)
    //         .truncate(true)
    //         .write(true)
    //         .open("../../input/光栅色纸测试图_out.tif")
    //         .unwrap();

    //     let mut out_encoder = tiff::encoder::TiffEncoder::new(&mut out_writer).unwrap();
    //     let mut out_tiff_img = out_encoder
    //         .new_image::<colortype::CMYK8>(out_width, out_height)
    //         .unwrap();

    //     let e = out_tiff_img.encoder();
    //     e.write_tag(
    //         Tag::Software,
    //         concat!("lenticular-image-tool", " ", env!("CARGO_PKG_VERSION")),
    //     )
    //     .unwrap();
    //     e.write_tag(Tag::ResolutionUnit, resolution_unit).unwrap();
    //     e.write_tag(
    //         Tag::XResolution,
    //         Rational {
    //             n: dpi_width as u32 * 10000,
    //             d: 10000,
    //         },
    //     )
    //     .unwrap();
    //     e.write_tag(
    //         Tag::YResolution,
    //         Rational {
    //             n: dpi as u32 * 10000,
    //             d: 10000,
    //         },
    //     )
    //     .unwrap();
    //     out_tiff_img.write_data(&output_bytes).unwrap();
    // }

    // #[test]
    // fn test_create_line_index_mapping() {
    //     let result = create_line_index_mapping(12, 3, 3, 0);
    //     eprintln!("result: {:?}", result);
    //     assert_eq!(result, [0, 1, 2, 9, 10, 11, 18, 19, 20, 27, 28, 29]);

    //     let result = create_line_index_mapping(12, 4, 2, 1);
    //     eprintln!("result: {:?}", result);
    //     assert_eq!(result, [4, 5, 6, 7, 12, 13, 14, 15, 20, 21, 22, 23]);
    // }
}
