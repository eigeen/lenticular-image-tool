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
}
