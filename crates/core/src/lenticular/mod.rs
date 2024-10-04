pub(crate) mod tiff;

use std::io::{Read, Seek};

pub use tiff::*;

use crate::{
    error::Result,
    image::{Cmyk8Color, MatrixImage},
};

/// 全局选项
pub struct ProcessOptions {
    pub(crate) lpi: f64,
    pub(crate) physical_width_cm: f64,
    pub(crate) scale_algorithm: Option<ScaleAlgorithm>,
}

impl ProcessOptions {
    pub fn new(lpi: f64, physical_width_cm: f64) -> Self {
        Self {
            lpi,
            physical_width_cm,
            scale_algorithm: None,
        }
    }

    pub fn with_scale_algorithm(mut self, algorithm: ScaleAlgorithm) -> Self {
        self.scale_algorithm = Some(algorithm);
        self
    }

    pub fn calc_output_info<R>(&self, inputs: &mut [InputImageContext<R>]) -> Result<OutputInfo>
    where
        R: Read + Seek,
    {
        calc_output_info(inputs, self)
    }

    pub fn process_tiff_cmyk8<R>(
        &self,
        inputs: Vec<InputImageContext<R>>,
        output_info: &OutputInfo,
        resize_alg: ScaleAlgorithm,
    ) -> Result<MatrixImage<Cmyk8Color>>
    where
        R: Read + Seek,
    {
        process_tiff_cmyk8(inputs, output_info, resize_alg)
    }
}

/// 缩放算法
#[derive(Debug, Clone, Copy, Default)]
pub enum ScaleAlgorithm {
    Nearest,
    #[default]
    Bilinear,
    Lanczos3,
}

impl From<ScaleAlgorithm> for fast_image_resize::ResizeAlg {
    fn from(val: ScaleAlgorithm) -> Self {
        match val {
            ScaleAlgorithm::Nearest => fast_image_resize::ResizeAlg::Nearest,
            ScaleAlgorithm::Bilinear => {
                fast_image_resize::ResizeAlg::Convolution(fast_image_resize::FilterType::Bilinear)
            }
            ScaleAlgorithm::Lanczos3 => {
                fast_image_resize::ResizeAlg::Convolution(fast_image_resize::FilterType::Lanczos3)
            }
        }
    }
}

/// 针对每张图的选项
#[derive(Debug, Clone)]
pub struct ImageOptions {
    pub lenticular_width_px: u32,
}

fn create_line_index_mapping_advanced(
    output_width: u32,
    lenticular_width_map: &[u32],
    img_index: usize,
) -> Vec<u32> {
    // todo: 验证索引不得超过map尺寸
    let mut output = vec![];

    // 光栅线宽度
    let lenticular_width: u32 = lenticular_width_map.iter().sum::<u32>();
    // 光栅线数量
    let lenticular_count: f64 = output_width as f64 / lenticular_width as f64;
    // 当前图之前还有多少光栅线宽度
    let image_offset_px: u32 = lenticular_width_map.iter().take(img_index).sum::<u32>();
    // 当前图片的光栅宽度
    let image_lent_width: u32 = lenticular_width_map[img_index];

    // 遍历光栅
    for group_index in 0..(lenticular_count.ceil() as u32) {
        let pos = group_index * lenticular_width + image_offset_px;
        for i in 0..image_lent_width {
            let pos1 = pos + i;
            output.push(pos1);
        }
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_line_index_mapping_advanced() {
        // // 均匀，2*3
        // let result = create_line_index_mapping_advanced(12, &[3, 3], 0);
        // eprintln!("result: {:?}", result);
        // assert_eq!(result, [0, 1, 2, 6, 7, 8, 12, 13, 14, 18, 19, 20]);

        // // 均匀，3*4，第二张图
        // let result = create_line_index_mapping_advanced(12, &[4, 4, 4], 1);
        // eprintln!("result: {:?}", result);
        // assert_eq!(result, [4, 5, 6, 7, 16, 17, 18, 19, 28, 29, 30, 31]);

        // // 不均匀，3+3+2，第一张图
        // let result = create_line_index_mapping_advanced(12, &[3, 3, 2], 0);
        // eprintln!("result: {:?}", result);
        // assert_eq!(result, [0, 1, 2, 8, 9, 10, 16, 17, 18, 24, 25, 26]);

        // // 均匀，1*4，第二张图
        // let result = create_line_index_mapping_advanced(16, &[1, 1, 1, 1], 1);
        // eprintln!("result: {:?}", result);
        // assert_eq!(
        //     result,
        //     [1, 5, 9, 13, 17, 21, 25, 29, 33, 37, 41, 45, 49, 53, 57, 61]
        // );

        let result = create_line_index_mapping_advanced(16, &[4, 4], 0);
        eprintln!("result: {:?}", result);

        let result = create_line_index_mapping_advanced(17, &[4, 4], 0);
        eprintln!("result: {:?}", result);
    }
}
