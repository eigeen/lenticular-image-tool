use std::io::{Read, Seek, SeekFrom};

use log::debug;
use ndarray::Axis;
use tiff::{
    decoder::{ifd::Value as TiffValue, DecodingResult as TiffDecodingResult},
    tags::Tag as TiffTag,
};

use crate::{
    error::{Error, Result},
    image::{resize_cmyk8, Cmyk8Color, MatrixImage, MatrixImageInfo},
    lenticular::create_line_index_mapping_advanced,
};

use super::{ImageOptions, Options};

/// 带上下文的输入文件
pub struct InputImageContext<R> {
    reader: R,
    image_options: ImageOptions,
}

impl<R> InputImageContext<R>
where
    R: Read + Seek,
{
    pub fn new(reader: R, options: ImageOptions) -> Self {
        Self {
            reader,
            image_options: options,
        }
    }
}

#[derive(Debug, Clone, Default)]
/// 计算过程所需的参数表
struct Params {
    lpi: f64,
    physical_width_cm: f64,

    source_params: SourceParams,
}

impl Params {
    pub fn new(lpi: f64, physical_width_cm: f64) -> Self {
        Self {
            lpi,
            physical_width_cm,

            ..Default::default()
        }
    }

    pub fn physical_width_in(&self) -> f64 {
        self.physical_width_cm * 0.3937
    }
}

#[derive(Debug, Clone, Default)]
struct SourceParams {
    color_type: Option<tiff::ColorType>,
    width: u32,
    height: u32,
    resolution_unit: u32,
    x_resolution: Option<TiffValue>,
    y_resolution: Option<TiffValue>,
}

impl SourceParams {
    pub fn set_color_type(&mut self, color_type: tiff::ColorType) {
        self.color_type = Some(color_type);
    }

    pub fn set_source_dimensions(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
    }

    pub fn set_resolution(
        &mut self,
        resolution_unit: u32,
        x_resolution: TiffValue,
        y_resolution: TiffValue,
    ) {
        self.resolution_unit = resolution_unit;
        self.x_resolution = Some(x_resolution);
        self.y_resolution = Some(y_resolution);
    }
}

/// 处理CMYK8图像
pub fn process_tiff_cmyk8<R>(
    mut inputs: Vec<InputImageContext<R>>,
    options: &Options,
) -> Result<MatrixImage<Cmyk8Color>>
where
    R: Read + Seek,
{
    if inputs.is_empty() {
        return Err(Error::InvalidInput("输入图像数量不可为空".to_string()));
    }

    let mut params = Params::new(options.lpi, options.physical_width_cm);

    // 读取第一张图作为基准
    let first_input = &mut inputs[0];
    let mut decoder = tiff::decoder::Decoder::new(&mut first_input.reader).unwrap();
    debug!("Reading first image as baseline");

    {
        let source_params = read_params_from_tiff(&mut decoder, true)?;
        debug!("color type: {:?}", source_params.color_type);
        debug!(
            "dimensions: {}x{}",
            source_params.width, source_params.height
        );
        debug!("resolution_unit: {}", source_params.resolution_unit);
        debug!("x_resolution: {:?}", source_params.x_resolution);
        debug!("y_resolution: {:?}", source_params.y_resolution);
        params.source_params = source_params;
    }

    // 丢弃第一张图的Decoder，方便后续迭代器访问
    drop(decoder);
    first_input.reader.seek(SeekFrom::Start(0))?;

    // 各种参数
    let lenticular_width_table = inputs
        .iter()
        .map(|c| c.image_options.lenticular_width_px as usize)
        .collect::<Vec<_>>();
    let max_lenticular_width_px = *lenticular_width_table.iter().max().unwrap() as u32;
    let total_lenticular_width_px = lenticular_width_table.iter().sum::<usize>() as u32;
    // 计算缩放目标
    // let physical_width_in_per_px =
    //     1.0 / (params.lpi * img_options.lenticular_width_px as f64);
    // let dpi = 1.0 / physical_width_in_per_px;
    // 以最宽光栅线的图像为基准，其他小于该宽度的图像后续进行横向缩放
    let dpi = params.lpi * max_lenticular_width_px as f64;
    let dpi_out_w = dpi * (total_lenticular_width_px as f64 / max_lenticular_width_px as f64);
    debug!("DPI_H: {:.2}", dpi);
    debug!("DPI_OUT_W: {:.2}", dpi_out_w);
    // 计算缩放目标分辨率
    let output_width_px: u32 = (dpi_out_w * params.physical_width_in()).floor() as u32;
    // let target_height_px = (target_width_px as f64
    //     * (params.source_params.height as f64 / params.source_params.width as f64))
    //     .floor() as u32;
    let output_height_px = (dpi * params.physical_width_in()).floor() as u32;

    // 创建输出图像
    let mut output_img: MatrixImage<Cmyk8Color> =
        MatrixImage::new(output_width_px, output_height_px);
    debug!(
        "output image: {}x{}",
        output_img.width(),
        output_img.height()
    );

    inputs
        .iter_mut()
        .enumerate()
        .try_for_each(|(input_index, input_ctx)| -> Result<()> {
            let mut decoder = tiff::decoder::Decoder::new(&mut input_ctx.reader)?;
            let img_options = &input_ctx.image_options;
            let img_params = read_params_from_tiff(&mut decoder, false)?;
            debug!("Image {:02} source: params: {:?}", input_index, img_params);
            if !is_matching_params(&params.source_params, &img_params) {
                return Err(Error::InvalidInput(format!(
                    "输入图像参数与基准图像参数不匹配: 预期：{:?}, 实际输入：{:?}",
                    params.source_params, img_params,
                )));
            }

            let width_ratio =
                img_options.lenticular_width_px as f64 / total_lenticular_width_px as f64;
            let target_width_px = (width_ratio * output_width_px as f64).floor() as u32;
            let target_height_px = output_height_px;
            // 非均匀线宽横向压缩修正
            // let mut target_width_px = output_width_px;
            // if img_options.lenticular_width_px < max_lenticular_width_px {
            //     let ratio: f64 =
            //         img_options.lenticular_width_px as f64 / max_lenticular_width_px as f64;
            //     target_width_px = (target_width_px as f64 * ratio).floor() as u32;
            // }
            debug!(
                "Image {:02} resized: {}x{}",
                input_index, target_width_px, target_height_px
            );

            // 读取图像数据
            let TiffDecodingResult::U8(img_res) = decoder.read_image()? else {
                return Err(Error::InvalidInput(
                    "图像数据读取失败: 非预期的编码类型，仅接受 CMYK 8位图像".to_string(),
                ));
            };
            //
            let resized_res = resize_cmyk8(
                img_res,
                img_params.width,
                img_params.height,
                target_width_px,
                target_height_px,
            )?;
            // 创建矩阵图像封装
            let input_img: MatrixImage<Cmyk8Color> =
                MatrixImage::from_slice(&resized_res, target_width_px, output_height_px)?;
            let input_mat = input_img.inner();
            // 写入输出图像
            let output_width = output_img.width() as usize;
            let output_mat = output_img.inner_mut();
            let col_mapping = create_line_index_mapping_advanced(
                input_img.width() as usize,
                &lenticular_width_table,
                input_index,
            );
            (0..input_img.width() as usize).for_each(|col_index| {
                let target_index = col_mapping[col_index];
                if target_index >= output_width {
                    debug!(
                        "Image {:02}: skipping out of range column {}",
                        input_index, target_index
                    );
                    return;
                }

                let input_column = input_mat.column(col_index);
                output_mat
                    .index_axis_mut(Axis(1), target_index)
                    .assign(&input_column);
            });

            Ok(())
        })?;

    // 写入一些信息
    output_img.set_info(MatrixImageInfo {
        dpi_h: dpi,
        dpi_w: dpi_out_w,
    });

    Ok(output_img)
}

/// 从解码器中读取图片元数据参数
fn read_params_from_tiff<R>(
    decoder: &mut tiff::decoder::Decoder<R>,
    read_tags: bool,
) -> Result<SourceParams>
where
    R: Read + Seek,
{
    let mut params = SourceParams::default();

    params.set_color_type(decoder.colortype()?);
    let (width, height) = decoder.dimensions()?;
    params.set_source_dimensions(width, height);

    if read_tags {
        let resolution_unit = decoder.get_tag(TiffTag::ResolutionUnit)?.into_u32()?;
        let x_resolution = decoder.get_tag(TiffTag::XResolution)?;
        let y_resolution = decoder.get_tag(TiffTag::YResolution)?;
        params.set_resolution(resolution_unit, x_resolution, y_resolution);
    }

    Ok(params)
}

/// 判断两个图片的基础参数是否一致
fn is_matching_params(base: &SourceParams, other: &SourceParams) -> bool {
    other.color_type.is_some()
        && base.color_type == other.color_type
        && base.width == other.width
        && base.height == other.height
}

#[cfg(test)]
mod tests {
    use std::{fs::File, io::BufReader};

    use tiff::encoder::{colortype, Rational};

    use super::*;

    #[test]
    fn test_process_tiff_cmyk8() {
        env_logger::builder()
            .filter_level(log::LevelFilter::Debug)
            .init();

        let mut inputs: Vec<InputImageContext<BufReader<File>>> = vec![];

        let file = std::fs::File::open("../../input/光栅色纸测试图1.tif").unwrap();
        let reader = std::io::BufReader::new(file);
        inputs.push(InputImageContext::new(
            reader,
            ImageOptions {
                lenticular_width_px: 4,
            },
        ));

        let file = std::fs::File::open("../../input/光栅色纸测试图2.tif").unwrap();
        let reader = std::io::BufReader::new(file);
        inputs.push(InputImageContext::new(
            reader,
            ImageOptions {
                lenticular_width_px: 2,
            },
        ));

        let options = Options {
            lpi: 100.41,
            physical_width_cm: 10.6,
        };
        let out = process_tiff_cmyk8(inputs, &options).unwrap();

        let mut out_writer = std::fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open("../../input/光栅色纸测试图_out.tif")
            .unwrap();

        let mut out_encoder = tiff::encoder::TiffEncoder::new(&mut out_writer).unwrap();
        let mut out_tiff_img = out_encoder
            .new_image::<colortype::CMYK8>(out.width(), out.height())
            .unwrap();

        if let Some(info) = out.info() {
            let e = out_tiff_img.encoder();
            e.write_tag(
                TiffTag::Software,
                concat!("lenticular-image-tool", " ", env!("CARGO_PKG_VERSION")),
            )
            .unwrap();
            e.write_tag(TiffTag::ResolutionUnit, 2).unwrap();
            e.write_tag(
                TiffTag::XResolution,
                Rational {
                    n: (info.dpi_w * 10000.0) as u32,
                    d: 10000,
                },
            )
            .unwrap();
            e.write_tag(
                TiffTag::YResolution,
                Rational {
                    n: (info.dpi_h * 10000.0) as u32,
                    d: 10000,
                },
            )
            .unwrap();
        }
        out_tiff_img.write_data(&out.into_bytes()).unwrap();
    }
}
