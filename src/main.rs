use dialoguer::{theme::ColorfulTheme, Completion, Input};
use env_logger::{Builder, Env};
use image::{imageops::FilterType, DynamicImage, GenericImageView, Rgba};
use log::{debug, error, info, warn};
use std::{
    fmt, fs,
    path::{Path, PathBuf},
};

use snafu::prelude::*;

#[derive(Snafu)]
enum Error {
    #[snafu(display("I/O error: {source}"))]
    IO { source: std::io::Error },
    #[snafu(display("Image error: {source}"))]
    Image { source: image::ImageError },
    #[snafu(display("{reason}"))]
    Input { reason: String },
}

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self)
    }
}

fn main() -> Result<(), Error> {
    dotenvy::dotenv().ok();
    let result = interact_process();
    // 借用Input来阻止窗口关闭
    let _: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("按Enter键关闭")
        .allow_empty(true)
        .interact()
        .unwrap();
    result
}

fn scan_inputs() -> Result<Vec<PathBuf>, Error> {
    let path = Path::new("input");
    info!("输入目录：{}", path.display());
    let mut inputs: Vec<PathBuf> = Vec::new();
    let entries = match fs::read_dir(path) {
        Ok(entries) => entries,
        Err(_) => {
            return Err(Error::Input {
                reason: format!("输入目录`{}`不存在或无法读取", path.display()),
            })
        }
    };
    for entry in entries {
        let entry = entry.context(IOSnafu)?;
        let file_path = entry.path();
        if file_path.is_file() {
            // 文件名检查
            let file_name = file_path.file_stem().unwrap().to_str().unwrap();
            // 名字必须是数字
            if file_name.chars().all(|c| c.is_digit(10)) {
                inputs.push(file_path);
            };
        }
    }
    // 文件名排序
    inputs.sort_by(|a, b| a.file_name().unwrap().cmp(b.file_name().unwrap()));

    Ok(inputs)
}

fn load_images(inputs: &[PathBuf]) -> Result<Vec<DynamicImage>, Error> {
    let images: Result<Vec<_>, _> = inputs
        .iter()
        .map(|input| image::open(input).context(ImageSnafu))
        .collect();
    Ok(images?)
}

fn interact_process() -> Result<(), Error> {
    Builder::from_env(Env::default().default_filter_or("info")).init();

    let inputs = match scan_inputs() {
        Ok(inputs) => inputs,
        Err(e) => {
            error!("需要在程序同级目录下建立目录`input`存放输入文件");
            // 借用Input来阻止窗口关闭
            let _: String = Input::with_theme(&ColorfulTheme::default())
                .with_prompt("按Enter键关闭")
                .allow_empty(true)
                .interact()
                .unwrap();
            return Err(Error::Input {
                reason: e.to_string(),
            });
        }
    };
    if inputs.len() == 0 {
        error!("未找到有效的文件；文件名必须为纯数字，例如`0001.jpg`，`2.png`等；输入文件不能为空");
        // 借用Input来阻止窗口关闭
        let _: String = Input::with_theme(&ColorfulTheme::default())
            .with_prompt("按Enter键关闭")
            .allow_empty(true)
            .interact()
            .unwrap();
        return Err(Error::Input {
            reason: "输入文件不能为空".to_string(),
        });
    }
    info!(
        "输入文件：{}",
        inputs
            .iter()
            .map(|i| i.display().to_string())
            .collect::<Vec<_>>()
            .join(", ")
    );

    let images = load_images(&inputs)?;
    let (min_width, min_height) =
        images
            .iter()
            .fold((u32::MAX, u32::MAX), |(min_w, min_h), img| {
                let (w, h) = img.dimensions();
                (min_w.min(w), min_h.min(h))
            });
    let aspect_ratio = min_height as f32 / min_width as f32;

    warn!(
        "目标图片采用所有输入源的最小宽高：{min_width} * {min_height}，宽高比 = 1:{aspect_ratio}"
    );
    warn!("若输入源比例不同，会自动缩放后居中裁切。比例相同，会自动缩放。");
    warn!("如果需要精确控制，请提前自行裁切所有输入源到相同比例");

    // 策略：丢弃多余的光栅，实际影响几乎没有
    let input_lpi = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("输入目标图片的光栅LPI（光栅密度，每英寸光栅线数量）")
        .validate_with(|input: &String| -> Result<(), &str> {
            match input.parse::<f64>() {
                Ok(_) => Ok(()),
                Err(_) => Err("请输入一个有效数字"),
            }
        })
        .interact_text()
        .unwrap();
    let direction_completion = DirectionCompletion::default();
    let input_direction: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("输入目标图片的光栅方向（横向(h)orizontal/纵向(v)erticle）")
        .completion_with(&direction_completion)
        .interact_text()
        .unwrap();
    let phy_height_prompt = if input_direction == "v" {
        "输入目标图片的物理宽度（单位：厘米）"
    } else {
        "输入目标图片的物理高度（单位：厘米）"
    };
    let input_phy_height = Input::with_theme(&ColorfulTheme::default())
        .with_prompt(phy_height_prompt)
        .validate_with(|input: &String| -> Result<(), &str> {
            match input.parse::<f64>() {
                Ok(_) => Ok(()),
                Err(_) => Err("请输入一个有效数字"),
            }
        })
        .interact_text()
        .unwrap();

    let lpi: f64 = input_lpi.parse().unwrap();
    let phy_len: f64 = input_phy_height.parse::<f64>().unwrap() * 0.3937; // 换算英寸
    let lenticular_count = (phy_len * lpi).ceil() as u32; // 理论需要的光栅线数量
    let min_length = if input_direction == "h" {
        min_height
    } else {
        min_width
    };
    let lenticular_pixel_thick = (min_length as f64 / lenticular_count as f64).ceil() as u32; // 理论光栅线像素宽度
    // 反推图片最佳分辨率
    let (min_width, min_height) = if input_direction == "h" {
        let new_height = lenticular_pixel_thick * lenticular_count;
        let new_width = (min_width as f64 * (new_height as f64 / min_height as f64)).ceil() as u32;
        (new_width, new_height)
    } else {
        let new_width = lenticular_pixel_thick * lenticular_count;
        let new_height = (min_height as f64 * (new_width as f64 / min_width as f64)).ceil() as u32;
        (new_width, new_height)
    };

    info!("输出图片光栅数量（向上取整）：{lenticular_count}");
    info!("输出图片光栅像素宽度（向上取整）：{lenticular_pixel_thick}px");
    warn!("为了保证准确光栅尺寸，原图宽(高)将被就近缩放到：{min_width} * {min_height}");

    let mut canvas = image::ImageBuffer::<Rgba<u8>, Vec<u8>>::new(min_width, min_height);
    images.iter().enumerate().for_each(|(img_index, img)| {
        let std_img = if img.width() != min_width || img.height() != min_height {
            img.resize_to_fill(min_width, min_height, FilterType::Lanczos3)
        } else {
            img.clone()
        };

        (0..lenticular_count)
            .skip(img_index)
            .step_by(images.len())
            .for_each(|lenticular_index| {
                let (start_x, start_y, w, h) = if input_direction == "h" {
                    // 横向
                    let start_x = 0;
                    let start_y = lenticular_index * lenticular_pixel_thick;
                    let w = min_width;
                    let h = lenticular_pixel_thick;
                    (start_x, start_y, w, h)
                } else {
                    // 纵向
                    let start_x = lenticular_index * lenticular_pixel_thick;
                    let start_y = 0;
                    let w = lenticular_pixel_thick;
                    let h = min_height;
                    (start_x, start_y, w, h)
                };

                debug!("block: image = {img_index}, lenticular = {lenticular_index}, x = {start_x}-{}, y = {start_y}-{}", start_x + w, start_y + h);
                // 遍历该矩形区域并复制像素
                (start_x..start_x + w).for_each(|x| {
                    (start_y..start_y + h).for_each(|y| {
                        if x < min_width && y < min_height {
                            let _ = canvas.put_pixel(x, y, std_img.get_pixel(x, y));
                        }
                    })
                });
            })
    });

    canvas.save("output.png").context(ImageSnafu)?;

    Ok(())
}

struct DirectionCompletion {
    options: Vec<String>,
}

impl Default for DirectionCompletion {
    fn default() -> Self {
        DirectionCompletion {
            options: vec!["h".to_string(), "v".to_string()],
        }
    }
}

impl Completion for DirectionCompletion {
    /// Simple completion implementation based on substring
    fn get(&self, input: &str) -> Option<String> {
        let matches = self
            .options
            .iter()
            .filter(|option| option.starts_with(input))
            .collect::<Vec<_>>();

        if matches.len() == 1 {
            Some(matches[0].to_string())
        } else {
            None
        }
    }
}
