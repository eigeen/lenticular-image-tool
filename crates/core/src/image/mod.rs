use ndarray::{Array, Array2, Order};

use crate::error::Result;

mod resize;

pub use resize::resize_cmyk8;

pub trait Color: Sized + Clone + Default {
    fn from_slice(slice: &[u8]) -> Vec<Self>;
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Cmyk8Color {
    pub c: u8,
    pub m: u8,
    pub y: u8,
    pub k: u8,
}

impl Color for Cmyk8Color {
    fn from_slice(slice: &[u8]) -> Vec<Self> {
        slice
            .chunks(4)
            .map(|chunk| Cmyk8Color {
                c: chunk[0],
                m: chunk[1],
                y: chunk[2],
                k: chunk[3],
            })
            .collect()
    }
}

#[derive(Debug, Clone)]
pub struct DpiInfo {
    pub dpi_h: f64,
    pub dpi_w: f64,
}

#[derive(Debug, Clone)]
pub struct ImageInfo {
    pub width: u32,
    pub height: u32,
}

#[derive(Clone)]
pub struct MatrixImage<C> {
    mat: Array2<C>,
    info: Option<DpiInfo>,
}

impl<C> MatrixImage<C>
where
    C: Color,
{
    pub fn from_slice(data: &[u8], width: u32, height: u32) -> Result<Self> {
        let colors = C::from_slice(data);
        let shape = (height as usize, width as usize);
        let mat = Array::from_vec(colors)
            .to_shape((shape, Order::RowMajor))?
            .to_owned();

        Ok(MatrixImage { mat, info: None })
    }

    pub fn new(width: u32, height: u32) -> Self {
        let shape = (height as usize, width as usize);
        let mat = Array::default(shape);

        MatrixImage { mat, info: None }
    }

    pub fn inner(&self) -> &Array2<C> {
        &self.mat
    }

    pub fn inner_mut(&mut self) -> &mut Array2<C> {
        &mut self.mat
    }

    pub fn matrix(&self) -> &Array2<C> {
        &self.mat
    }

    pub fn matrix_mut(&mut self) -> &mut Array2<C> {
        &mut self.mat
    }

    pub fn height(&self) -> u32 {
        self.mat.shape()[0] as u32
    }

    pub fn width(&self) -> u32 {
        self.mat.shape()[1] as u32
    }

    pub fn set_info(&mut self, info: DpiInfo) {
        self.info = Some(info)
    }

    pub fn info(&self) -> Option<&DpiInfo> {
        self.info.as_ref()
    }
}

impl MatrixImage<Cmyk8Color> {
    pub fn to_bytes(&self) -> Vec<u8> {
        self.mat
            .iter()
            .flat_map(|c| [c.c, c.m, c.y, c.k])
            .collect::<Vec<u8>>()
    }
}

pub struct TypedRawImage<C> {
    data: Vec<u8>,
    info: ImageInfo,
    dpi_info: DpiInfo,
    _color_marker: std::marker::PhantomData<C>,
}

impl<C> TypedRawImage<C>
where
    C: Color,
{
    pub fn new(data: Vec<u8>, width: u32, height: u32, dpi_h: f64, dpi_w: f64) -> Self {
        TypedRawImage {
            data,
            info: ImageInfo { width, height },
            dpi_info: DpiInfo { dpi_h, dpi_w },
            _color_marker: std::marker::PhantomData,
        }
    }
}
