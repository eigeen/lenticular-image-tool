use std::{
    fs::{File, OpenOptions},
    io::BufReader,
};

use anyhow::Context;
use clap::{Parser, ValueEnum};
use lenticular_core::lenticular::{self, ImageOptions, InputImageContext, ProcessOptions};
use log::{debug, info};

#[derive(Debug, Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    // 输入参数
    /// 输入文件，可以为多个。若输入多个文件，请保证文件数量与后续多个参数数量一致。
    #[clap(short, long)]
    input: Vec<String>,
    /// 为每个文件指定目标光栅宽度。
    ///
    /// 若输入多个文件，则每个文件对应一个光栅宽度。
    /// 若该参数只设置一个，则所有文件都使用该值。
    #[clap(short, long)]
    lenticular_width: Vec<u32>,
    /// 不自动分配光栅像素宽度。
    ///
    /// 若指定此参数，则将采用输入值作为光栅像素宽度绝对值。
    /// 这可以精确分配光栅像素宽度，但可能导致输出图像过小或过大。
    #[clap(long)]
    no_auto_assign_width: bool,

    // 调整
    /// 缩放算法
    #[clap(long)]
    scale_algorithm: Option<ScaleAlgorithm>,

    // 输出参数
    /// 光栅线宽，单位：光栅数/英寸(LPI)
    #[clap(long)]
    lpi: f64,
    /// 输出图像宽度，单位：毫米(mm)
    #[clap(long)]
    output_width: f64,
    // /// 对输出图像进行缩放，使得分辨率与输入图像一致
    // #[clap(long)]
    // resize_output: bool,
    /// 输出文件
    #[clap(short, long)]
    output: String,

    /// 启用调试输出
    #[clap(long)]
    debug: bool,
}

#[derive(Debug, Clone, Copy, Default, ValueEnum)]
enum ScaleAlgorithm {
    Nearest,
    #[default]
    Bilinear,
    Lanczos3,
}

impl From<ScaleAlgorithm> for lenticular::ScaleAlgorithm {
    fn from(val: ScaleAlgorithm) -> Self {
        match val {
            ScaleAlgorithm::Nearest => lenticular::ScaleAlgorithm::Nearest,
            ScaleAlgorithm::Bilinear => lenticular::ScaleAlgorithm::Bilinear,
            ScaleAlgorithm::Lanczos3 => lenticular::ScaleAlgorithm::Lanczos3,
        }
    }
}

fn main() -> anyhow::Result<()> {
    env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .init();

    let mut cli = Cli::parse();

    if cli.debug {
        log::set_max_level(log::LevelFilter::Debug);
    } else {
        log::set_max_level(log::LevelFilter::Info);
    }

    if cli.input.is_empty() {
        return Err(anyhow::anyhow!("输入文件为空"));
    }
    if cli.lenticular_width.len() > 1 && cli.input.len() != cli.lenticular_width.len() {
        return Err(anyhow::anyhow!(
            "输入文件数量与 --lenticular-width 的参数数量不一致"
        ));
    }
    if cli.lenticular_width.iter().any(|&w| w == 0) {
        return Err(anyhow::anyhow!("光栅像素宽度必须大于0"));
    }
    if cli.lenticular_width.len() == 1 && cli.input.len() > 1 {
        // 若只有一个光栅宽度，则所有文件都使用该值
        cli.lenticular_width = vec![cli.lenticular_width[0]; cli.input.len()];
    }
    if cli.lpi <= 0.0 {
        return Err(anyhow::anyhow!("LPI必须大于0"));
    }
    if cli.output_width <= 0.0 {
        return Err(anyhow::anyhow!("输出图像宽度必须大于0"));
    }

    // 核心功能
    info!("参数输入：");
    info!("输入文件：{:?}", cli.input);
    info!("光栅宽度：{:?}", cli.lenticular_width);
    info!("自动分配光栅宽度：{:?}", !cli.no_auto_assign_width);
    info!("LPI：{:?}", cli.lpi);
    info!("输出图像宽度：{:?}", cli.output_width);
    // info!("缩放输出：{:?}", cli.resize_output);
    info!("输出文件：{:?}", cli.output);
    info!("缩放算法：{:?}", cli.scale_algorithm.unwrap_or_default());

    let inputs: anyhow::Result<Vec<InputImageContext<BufReader<File>>>> = cli
        .input
        .iter()
        .zip(cli.lenticular_width.iter())
        .map(|(input, lenticular_width)| {
            let file = File::open(input).context(format!("打开文件 {} 失败", input))?;
            let reader = BufReader::new(file);
            Ok(InputImageContext::new(
                reader,
                ImageOptions {
                    lenticular_width_px: *lenticular_width,
                },
            ))
        })
        .collect();
    let mut inputs = inputs?;

    info!("");
    info!("开始计算输出...");

    let start = std::time::Instant::now();

    // 测试最佳光栅宽度
    let opt = ProcessOptions::new(cli.lpi, cli.output_width)
        .with_scale_algorithm(cli.scale_algorithm.unwrap_or_default().into());
    let mut output_info = opt.calc_output_info(&mut inputs)?;
    // 自动模式，自动计算最优光栅宽度
    if !cli.no_auto_assign_width && output_info.height < output_info.source_params.height {
        // 输出太小，尝试重新计算
        // 计算最简光栅宽度
        let gcd = cli
            .lenticular_width
            .iter()
            .cloned()
            .reduce(num::integer::gcd)
            .unwrap_or(1);
        let delta = cli
            .lenticular_width
            .iter()
            .map(|w| w / gcd)
            .collect::<Vec<u32>>();
        let mut best_lenticular_width = cli.lenticular_width.clone();
        loop {
            // 应用到输入
            inputs
                .iter_mut()
                .zip(best_lenticular_width.iter())
                .for_each(|(input, w)| input.image_options_mut().lenticular_width_px = *w);
            // 计算输出
            output_info = opt.calc_output_info(&mut inputs)?;
            debug!("trying new output_info: {:?}", output_info);
            if output_info.height >= output_info.source_params.height {
                break;
            }
            // 尝试更大的光栅宽度
            best_lenticular_width
                .iter_mut()
                .zip(delta.iter())
                .for_each(|(w, b)| {
                    *w += b;
                });
        }

        info!("自动光栅宽度(px)：{:?}", best_lenticular_width);
        inputs
            .iter_mut()
            .zip(best_lenticular_width.iter())
            .for_each(|(input, w)| {
                input.image_options_mut().lenticular_width_px = *w;
            });
    }

    debug!(
        "inputs: {:?}",
        inputs.iter().map(|i| i.image_options()).collect::<Vec<_>>()
    );

    let out = opt.process_tiff_cmyk8(
        inputs,
        &output_info,
        cli.scale_algorithm.unwrap_or_default().into(),
    )?;

    let output_file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&cli.output)?;
    lenticular::write_tiff_cmyk8(output_file, &out)?;

    let elapsed = start.elapsed().as_millis();
    info!("处理完成，耗时 {} 毫秒", elapsed);

    Ok(())
}
