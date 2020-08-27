use crate::math::*;
use std::ops::{AddAssign, Mul};
use serde::{Serialize, Deserialize};

/// Generic RGBA color.
#[derive(Clone, Copy, Default, Debug, Serialize, Deserialize)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub const fn black() -> Color {
        Self {
            r: 0.0, g: 0.0, b: 0.0, a: 1.0
        }
    }

    pub const fn white() -> Color {
        Self {
            r: 1.0, g: 1.0, b: 1.0, a: 1.0
        }
    }

    pub fn mono(lum: f32) -> Color {
        Color {
            r: lum,
            g: lum,
            b: lum,
            a: 1.0,
        }
    }

    pub fn hex_abgr(hex: u32) -> Color {
        let a = (hex >> 24) & 0xFF;
        let b = (hex >> 16) & 0xFF;
        let g = (hex >> 8) & 0xFF;
        let r = hex & 0xFF;

        Color {
            r: (r as f32) / 255.0,
            g: (g as f32) / 255.0,
            b: (b as f32) / 255.0,
            a: (a as f32) / 255.0,
        }
    }

    pub fn hex_rgba(hex: u32) -> Color {
        let r = (hex >> 24) & 0xFF;
        let g = (hex >> 16) & 0xFF;
        let b = (hex >> 8) & 0xFF;
        let a = hex & 0xFF;

        Color {
            r: (r as f32) / 255.0,
            g: (g as f32) / 255.0,
            b: (b as f32) / 255.0,
            a: (a as f32) / 255.0,
        }
    }

    pub fn hex_rgb(hex: u32) -> Color {
        let r = (hex >> 16) & 0xFF;
        let g = (hex >> 8) & 0xFF;
        let b = (hex >> 0) & 0xFF;
        let a = 255;

        Color {
            r: (r as f32) / 255.0,
            g: (g as f32) / 255.0,
            b: (b as f32) / 255.0,
            a: (a as f32) / 255.0,
        }
    }

    pub fn rgba(r: f32, g: f32, b: f32, a: f32) -> Color {
        Color { r, g, b, a }
    }

    pub fn rgb(r: f32, g: f32, b: f32) -> Color {
        Color { r, g, b, a: 1.0 }
    }

    #[allow(dead_code)]
    pub fn lerp(a: &Color, b: &Color, t: f32) -> Color {
        Color::rgba(
            lerp(a.r, b.r, t),
            lerp(a.g, b.g, t),
            lerp(a.b, b.b, t),
            lerp(a.a, b.a, t),
        )
    }
}

impl Into<[f32; 4]> for Color {
    fn into(self) -> [f32; 4] {
        [self.r, self.g, self.b, self.a]
    }
}

impl Into<(f32, f32, f32, f32)> for Color {
    fn into(self) -> (f32, f32, f32, f32) {
        (self.r, self.g, self.b, self.a)
    }
}

impl Into<wgpu::Color> for Color {
    fn into(self) -> wgpu::Color {
        wgpu::Color {
            r: self.r as f64,
            g: self.g as f64,
            b: self.b as f64,
            a: self.a as f64,
        }
    }
}

impl Into<u32> for Color {
    fn into(self) -> u32 {
        ((self.r * 255.0) as u32 & 0xFF)
            | ((((self.g * 255.0) as u32) & 0xFF) << 8)
            | ((((self.b * 255.0) as u32) & 0xFF) << 16)
            | ((((self.a * 255.0) as u32) & 0xFF) << 24)
    }
}

impl AddAssign<Color> for Color {
    fn add_assign(&mut self, rhs: Color) {
        self.r = self.r + rhs.r;
        self.g = self.g + rhs.g;
        self.b = self.b + rhs.b;
        self.a = self.a + rhs.a;
    }
}

impl Mul<f32> for Color {
    type Output = Color;

    fn mul(self, rhs: f32) -> Self::Output {
        Color {
            r: self.r * rhs,
            g: self.g * rhs,
            b: self.b * rhs,
            a: self.a * rhs,
        }
    }
}

