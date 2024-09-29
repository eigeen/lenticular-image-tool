use std::io::{Read, Seek, SeekFrom, Write};

use fast_image_resize::{FilterType, ResizeAlg};
use log::{debug, warn};
use ndarray::Axis;
use tiff::{
    decoder::{ifd::Value as TiffValue, DecodingResult as TiffDecodingResult},
    encoder::{colortype, Rational},
    tags::Tag as TiffTag,
};

use crate::{
    error::{Error, Result},
    image::{resize_cmyk8, Cmyk8Color, DpiInfo, MatrixImage},
    lenticular::create_line_index_mapping_advanced,
};

use super::{ImageOptions, ProcessOptions, ScaleAlgorithm};

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

    pub fn image_options(&self) -> &ImageOptions {
        &self.image_options
    }

    pub fn image_options_mut(&mut self) -> &mut ImageOptions {
        &mut self.image_options
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

/// 输出图像信息
#[derive(Debug, Clone)]
pub struct OutputInfo {
    pub width: u32,
    pub height: u32,
    pub dpi_w: f64,
    pub dpi_h: f64,

    pub source_params: SourceParams,
}

#[derive(Debug, Clone, Default)]
pub struct SourceParams {
    pub color_type: Option<tiff::ColorType>,
    pub width: u32,
    pub height: u32,
    pub resolution_unit: u32,
    pub x_resolution: Option<TiffValue>,
    pub y_resolution: Option<TiffValue>,
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

/// 计算输出图像信息
pub fn calc_output_info<R>(
    inputs: &mut [InputImageContext<R>],
    options: &ProcessOptions,
) -> Result<OutputInfo>
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

    // 还原状态
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
    // 以最宽光栅线的图像为基准，其他小于该宽度的图像后续进行横向缩放
    let dpi = params.lpi * max_lenticular_width_px as f64;
    let dpi_w = dpi * (total_lenticular_width_px as f64 / max_lenticular_width_px as f64);
    debug!("DPI_OUT: {:.2}", dpi);
    // 计算缩放目标分辨率
    let f_scale = dpi * params.physical_width_in() / params.source_params.width as f64;
    let f_scale_extra_w = total_lenticular_width_px as f64 / max_lenticular_width_px as f64;

    let output_width_px: u32 =
        (params.source_params.width as f64 * f_scale * f_scale_extra_w) as u32;
    let output_height_px = (params.source_params.height as f64 * f_scale) as u32;

    Ok(OutputInfo {
        width: output_width_px,
        height: output_height_px,
        dpi_w,
        dpi_h: dpi,
        source_params: params.source_params,
    })
}

/// 处理CMYK8图像
pub fn process_tiff_cmyk8<R>(
    mut inputs: Vec<InputImageContext<R>>,
    output_info: &OutputInfo,
    scale_alg: ScaleAlgorithm,
) -> Result<MatrixImage<Cmyk8Color>>
where
    R: Read + Seek,
{
    if inputs.is_empty() {
        return Err(Error::InvalidInput("输入图像数量不可为空".to_string()));
    }

    // 各种参数
    let lenticular_width_table = inputs
        .iter()
        .map(|c| c.image_options.lenticular_width_px as usize)
        .collect::<Vec<_>>();
    let total_lenticular_width_px = lenticular_width_table.iter().sum::<usize>() as u32;

    // 创建输出图像
    let mut output_img: MatrixImage<Cmyk8Color> =
        MatrixImage::new(output_info.width, output_info.height);
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
            if !is_matching_params(&output_info.source_params, &img_params) {
                return Err(Error::InvalidInput(format!(
                    "输入图像参数与基准图像参数不匹配: 预期：{:?}, 实际输入：{:?}",
                    output_info.source_params, img_params,
                )));
            }

            let width_ratio =
                img_options.lenticular_width_px as f64 / total_lenticular_width_px as f64;
            let target_width_px = (width_ratio * output_info.width as f64).floor() as u32;
            let target_height_px = output_info.height;
            debug!(
                "width_ratio: {:.2}, target_width_px: {}, target_height_px: {}",
                width_ratio, target_width_px, target_height_px
            );

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
            // 对宽度进行缩放
            let resized_res = resize_cmyk8(
                img_res,
                img_params.width,
                img_params.height,
                target_width_px,
                target_height_px,
                scale_alg.into(),
            )?;
            // 创建矩阵图像封装
            let input_img: MatrixImage<Cmyk8Color> =
                MatrixImage::from_slice(&resized_res, target_width_px, output_info.height)?;
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
    output_img.set_info(DpiInfo {
        dpi_h: output_info.dpi_h,
        dpi_w: output_info.dpi_w,
    });

    Ok(output_img)
}

/// 基于DPI还原分辨率
pub fn restore_resolution_cmyk8(
    image: &MatrixImage<Cmyk8Color>,
) -> Result<MatrixImage<Cmyk8Color>> {
    let Some(info) = image.info() else {
        return Err(Error::InvalidInput(
            "图像信息缺失，无法进行分辨率还原".to_string(),
        ));
    };
    if info.dpi_h == info.dpi_w {
        return Ok(image.clone());
    }
    // 计算缩放因子
    let scale_factor = info.dpi_h / info.dpi_w;
    let (target_width_px, target_height_px, dpi) = {
        if scale_factor > 1.0 {
            // 宽度缩放
            let target_width_px = (image.width() as f64 * scale_factor).floor() as u32;
            let target_height_px = image.height();
            (target_width_px, target_height_px, info.dpi_h)
        } else {
            // 高度缩放
            let target_height_px = (image.height() as f64 / scale_factor).floor() as u32;
            let target_width_px = image.width();
            (target_width_px, target_height_px, info.dpi_w)
        }
    };
    debug!(
        "Restore resolution: source DPI: {:.2}x{:.2}. target px: {:.2}x{:.2}",
        info.dpi_w, info.dpi_h, target_width_px, target_height_px
    );

    let img_res = image.to_bytes();
    let resized_res = resize_cmyk8(
        img_res,
        image.width(),
        image.height(),
        target_width_px,
        target_height_px,
        ResizeAlg::Convolution(FilterType::Bilinear),
    )?;

    let mut out = MatrixImage::from_slice(&resized_res, target_width_px, target_height_px)?;
    out.set_info(DpiInfo {
        dpi_h: dpi,
        dpi_w: dpi,
    });

    Ok(out)
}

pub fn write_tiff_cmyk8<W>(writer: W, out: &MatrixImage<Cmyk8Color>) -> Result<()>
where
    W: Write + Seek,
{
    let mut out_encoder = tiff::encoder::TiffEncoder::new(writer)?;

    // 还原分辨率
    let out_restored = restore_resolution_cmyk8(out)?;
    let mut out_tiff_img =
        out_encoder.new_image::<colortype::CMYK8>(out_restored.width(), out_restored.height())?;

    // 写入元数据
    if let Some(info) = out_restored.info() {
        let dpi_w_n = (info.dpi_w * 10000.0) as u32;
        let dpi_h_n = (info.dpi_h * 10000.0) as u32;
        debug!(
            "Write tags into tiff image: DPI_H: {}, DPI_W: {:.2}",
            dpi_w_n / 10000,
            dpi_h_n / 10000
        );

        let e = out_tiff_img.encoder();
        e.write_tag(
            TiffTag::Software,
            concat!("lenticular-image-tool", " ", env!("CARGO_PKG_VERSION")),
        )?;
        e.write_tag(TiffTag::ResolutionUnit, 2)?;
        e.write_tag(
            TiffTag::XResolution,
            Rational {
                n: (info.dpi_w * 10000.0) as u32,
                d: 10000,
            },
        )?;
        e.write_tag(
            TiffTag::YResolution,
            Rational {
                n: (info.dpi_h * 10000.0) as u32,
                d: 10000,
            },
        )?;
    } else {
        warn!("图像信息缺失，无法写入 TIFF 信息");
    }

    out_tiff_img.write_data(&out_restored.to_bytes())?;

    Ok(())
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

        let file = std::fs::File::open("../../input/01.tif").unwrap();
        let reader = std::io::BufReader::new(file);
        inputs.push(InputImageContext::new(
            reader,
            ImageOptions {
                lenticular_width_px: 7,
            },
        ));

        let file = std::fs::File::open("../../input/02.tif").unwrap();
        let reader = std::io::BufReader::new(file);
        inputs.push(InputImageContext::new(
            reader,
            ImageOptions {
                lenticular_width_px: 7,
            },
        ));

        let opt = ProcessOptions::new(91.60, 10.6);
        let output_info = opt.calc_output_info(&mut inputs).unwrap();
        // let out = ProcessOptions::new(100.41, 10.6)
        //     .process_tiff_cmyk8(inputs, &output_info, ScaleAlgorithm::Bilinear)
        //     .unwrap();
        let out = opt
            .process_tiff_cmyk8(inputs, &output_info, ScaleAlgorithm::Bilinear)
            .unwrap();
        // 缩放到原始比例
        let out = restore_resolution_cmyk8(&out).unwrap();

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
        out_tiff_img.write_data(&out.to_bytes()).unwrap();
    }
}
