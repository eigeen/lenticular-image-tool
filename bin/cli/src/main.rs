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
    /// 指定每个文件的采用数量。
    ///
    /// 若输入多个文件，则每个文件对应一个值。
    ///
    /// 若该参数只设置一个，则所有文件都使用该值。不输入时，默认为1。
    #[clap(short, long)]
    count: Option<Vec<u32>>,

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

    let cli = Cli::parse();

    if cli.debug {
        log::set_max_level(log::LevelFilter::Debug);
    } else {
        log::set_max_level(log::LevelFilter::Info);
    }

    if cli.input.is_empty() {
        return Err(anyhow::anyhow!("输入文件为空"));
    }
    let mut counts = cli.count.unwrap_or_else(|| vec![1]);
    if counts.len() > 1 && cli.input.len() != counts.len() {
        return Err(anyhow::anyhow!("输入文件数量与 --repeat 的参数数量不一致"));
    }
    if counts.iter().any(|&w| w == 0) {
        return Err(anyhow::anyhow!("重复次数必须大于0"));
    }
    if counts.len() == 1 && cli.input.len() > 1 {
        // 若只有一个光栅宽度，则所有文件都使用该值
        counts = vec![counts[0]; cli.input.len()];
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
    info!("文件重复采用数量：{:?}", counts);
    info!("LPI：{:?}", cli.lpi);
    info!("输出图像宽度：{:?}", cli.output_width);
    // info!("缩放输出：{:?}", cli.resize_output);
    info!("输出文件：{:?}", cli.output);
    info!("缩放算法：{:?}", cli.scale_algorithm.unwrap_or_default());

    let inputs: anyhow::Result<Vec<InputImageContext<BufReader<File>>>> = cli
        .input
        .iter()
        .zip(counts.iter())
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

    let opt = ProcessOptions::new(cli.lpi, cli.output_width)
        .with_scale_algorithm(cli.scale_algorithm.unwrap_or_default().into());
    let output_info = opt.calc_output_info(&mut inputs)?;

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
