use crate::math::*;
use specs::Component;
use std::ops::{AddAssign, Mul};
use std::time::Instant;

#[derive(Clone, Copy, Default, Debug)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub fn black() -> Color {
        Self::rgb(0.0, 0.0, 0.0)
    }

    #[allow(dead_code)]
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

// TODO: Investigate why tuple and array order are reversed, but the result correct!
impl Into<[f32; 4]> for Color {
    fn into(self) -> [f32; 4] {
        [self.r, self.g, self.b, self.a]
        //        [self.a, self.b, self.g, self.r]
    }
}

impl Into<(f32, f32, f32, f32)> for Color {
    fn into(self) -> (f32, f32, f32, f32) {
        //        (self.r, self.g, self.b, self.a)
        (self.a, self.b, self.g, self.r)
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

// pub struct Transform {
//     pub pos: Vec3,
//     pub rot: Vec3,
// }

// impl Transform {
//     pub fn get_rotation(&self) -> Rotation3 {
//         Rotation3::from_euler_angles(self.rot[0], self.rot[1], self.rot[2])
//     }
// }

// impl Component for Transform {
//     type Storage = specs::VecStorage<Self>;
// }

pub struct FpsCounter {
    elapsed: f32,
    frames: u32,
    calculated_fps: f32,
    frame_begin_time: Instant,
}

impl FpsCounter {
    pub fn new() -> Self {
        FpsCounter {
            elapsed: 0.0,
            frames: 0,
            calculated_fps: 0.0,
            frame_begin_time: Instant::now(),
        }
    }

    pub fn begin_frame(&mut self) {
        self.frame_begin_time = Instant::now();
    }

    pub fn end_frame(&mut self) -> bool {
        let elapsed = self.frame_begin_time.elapsed();
        let delta_time =
            (elapsed.as_secs() as f64 + (elapsed.subsec_nanos() as f64) * 1.0e-9) as f32;
        self.elapsed += delta_time;
        self.frames += 1;

        //        info!("Delta time {}, frames {}", delta_time, self.frames);

        if self.elapsed >= 1.0 && self.frames >= 5 {
            self.calculated_fps = (self.frames as f32) / self.elapsed;
            self.frames = 0;
            self.elapsed = 0.0;

            true
        } else {
            false
        }
    }

    pub fn get_fps(&self) -> f32 {
        return self.calculated_fps;
    }
}
