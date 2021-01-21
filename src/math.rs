/// ! General mathematics operation.

use std::f32::consts;
use std::ops::{Div, Mul, Sub, Add};

pub use glam::*;

pub type Perspective = cgmath::PerspectiveFov<f32>;

pub const PI: f32 = consts::PI;
pub const DEG_2_RAD: f32 = PI / 180.0;
pub const RAD_2_DEG: f32 = 180.0 / PI;

pub use num_traits::{One, real::Real};

/// Linearly interpolates between a and b with parameter t.
#[inline]
pub fn lerp<T>(a: T, b: T, t: T) -> T
where T: Add<Output=T> + Sub<Output=T> + Mul<Output=T> + Copy {
    a + (b - a) * t
}

/// Limit x in the range of [min, max].
#[inline]
pub fn clamp<T: PartialOrd>(x: T, min: T, max: T) -> T {
    if x < min {
        min
    } else if x > max {
        max
    } else {
        x
    }
}

// Currently same as unity, maybe can be better?
#[inline]
pub fn approx_eq(lhs: f32, rhs: f32) -> bool {
    let delta = (rhs - lhs).abs();
    delta < f32::max(f32::EPSILON * 8., 1e-6 *
        f32::max(lhs.abs(), rhs.abs()))
}

#[inline]
pub fn vec2_approx_eq(v1: Vec2, v2: Vec2) -> bool {
    approx_eq(v1.x, v2.x) &&
        approx_eq(v1.y, v2.y)
}

#[inline]
pub fn deg2rad(deg: f32) -> f32 {
    DEG_2_RAD * deg
}

#[inline]
pub fn rad2deg(rad: f32) -> f32 {
    RAD_2_DEG * rad
}

/// Calculates floor(a / b) for a > 0 and b > 0.
#[inline]
pub fn div_floor<T>(a: T, b: T) -> T
where
    T: Copy + One + Ord + Sub<Output = T> + Div<Output = T> + Mul<Output = T>,
{
    let one = T::one();
    let res = a / b;
    if res * b > a {
        res - one
    } else {
        res
    }
}

#[derive(Copy, Clone, Default, Debug)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {

    pub fn new_origin(width: f32, height: f32) -> Self {
        Self { x: 0., y: 0., width, height }
    }

    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self { x, y, width, height }
    }

    pub fn approx_eq(lhs: &Rect, rhs: &Rect) -> bool {
        approx_eq(lhs.x, rhs.x) &&
            approx_eq(lhs.y, rhs.y) &&
            approx_eq(lhs.width, rhs.width) &&
            approx_eq(lhs.height, rhs.height)
    }

    pub fn size(&self) -> Vec2 {
        vec2(self.width, self.height)
    }

    pub fn contains(&self, v: &Vec2) -> bool {
        self.x <= v.x && v.x <= self.x + self.width &&
            self.y <= v.y && v.y <= self.y + self.height
    }

}

pub mod quat {
    use super::*;

    pub fn get_forward_dir(q: Quat) -> Vec3 {
        q * vec3(0., 0., -1.)
    }

    pub fn get_right_dir(q: Quat) -> Vec3 {
        q * vec3(1., 0., 0.)
    }

}

// TODO
pub trait Mat3Ext {
}

pub mod mat3ex {
    use super::*;

    #[inline]
    pub fn extend_to_mat4(m: &Mat3) -> Mat4 {
        #[inline]
        fn ext_col(v: &Vec3) -> Vec4 {
            Vec4::new(v.x, v.y, 0., v.z)
        }

        Mat4::from_cols(
            ext_col(&m.x_axis),
            ext_col(&m.y_axis),
            Vec4::new(0., 0., 1., 0.),
            ext_col(&m.z_axis)
        )
    }

    pub fn translate(p: Vec2) -> Mat3 {
        #[cfg_attr(rustfmt, rustfmt_skip)]
        Mat3::from_cols_array(&[
            1., 0., 0.,
            0., 1., 0.,
            p.x, p.y, 1.,
        ])
    }

    pub fn ortho(left: f32, right: f32, bottom: f32, top: f32) -> Mat3 {
        let c0r0 = 2. / (right - left);
        let c0r1 = 0.;
        let c0r2 = 0.;

        let c1r0 = 0.;
        let c1r1 = 2. / (top - bottom);
        let c1r2 = 0.;

        let c2r0 = -(right + left) / (right - left);
        let c2r1 = -(top + bottom) / (top - bottom);
        let c2r2 = 1.;

        #[cfg_attr(rustfmt, rustfmt_skip)]
        Mat3::from_cols_array(&[
            c0r0, c0r1, c0r2,
            c1r0, c1r1, c1r2,
            c2r0, c2r1, c2r2
        ])
    }

    pub fn rotate_around(p: Vec2, angle: f32) -> Mat3 {
        let c = angle.cos();
        let s = angle.sin();
        let dx = p.x;
        let dy = p.y;

        let c0r0 = c;
        let c1r0 = -s;
        let c2r0 = dx - dx * c + dy * s;
        let c0r1 = s;
        let c1r1 = c;
        let c2r1 = dy - dx * s - dy * c;
        let c0r2 = 0.;
        let c1r2 = 0.;
        let c2r2 = 1.;

        Mat3::from_cols_array(&[
            c0r0, c0r1, c0r2,
            c1r0, c1r1, c1r2,
            c2r0, c2r1, c2r2
        ])
    }

    pub fn scale_around(p: Vec2, scl: Vec2) -> Mat3 {
        let (dx, dy) = (p.x, p.y);
        let (sx, sy) = (scl.x, scl.y);

        let c0r0 = sx;
        let c1r0 = 0.;
        let c2r0 = dx * (1. - sx);
        let c0r1 = 0.;
        let c1r1 = sy;
        let c2r1 = dy * (1. - sy);
        let c0r2 = 0.;
        let c1r2 = 0.;
        let c2r2 = 1.;

        Mat3::from_cols_array(&[
            c0r0, c0r1, c0r2,
            c1r0, c1r1, c1r2,
            c2r0, c2r1, c2r2
        ])
    }
}

/// Convenient projection operations.
pub mod projection {
    use super::*;

    pub fn ortho(left: f32, right: f32, bottom: f32, top: f32, near: f32, far: f32) -> Mat4 {
        let c0r0 = 2.0 / (right - left);
        let c0r1 = 0.;
        let c0r2 = 0.;
        let c0r3 = 0.;

        let c1r0 = 0.;
        let c1r1 = 2.0 / (top - bottom);
        let c1r2 = 0.;
        let c1r3 = 0.;

        let c2r0 = 0.;
        let c2r1 = 0.;
        let c2r2 = -2.0 / (far - near);
        let c2r3 = 0.;

        let c3r0 = -(right + left) / (right - left);
        let c3r1 = -(top + bottom) / (top - bottom);
        let c3r2 = -(far + near) / (far - near);
        let c3r3 = 1.;

        #[cfg_attr(rustfmt, rustfmt_skip)]
        Mat4::from_cols_array(&[
            c0r0, c0r1, c0r2, c0r3,
            c1r0, c1r1, c1r2, c1r3,
            c2r0, c2r1, c2r2, c2r3,
            c3r0, c3r1, c3r2, c3r3,
        ])
    }

    pub fn perspective(fov: f32, aspect: f32, z_near: f32, z_far: f32) -> Mat4 {
        let f = 1.0 / (fov * 0.5).tan();

        let c0r0 = f / aspect;
        let c0r1 = 0.0;
        let c0r2 = 0.0;
        let c0r3 = 0.0;

        let c1r0 = 0.0;
        let c1r1 = f;
        let c1r2 = 0.0;
        let c1r3 = 0.0;

        let c2r0 = 0.0;
        let c2r1 = 0.0;
        let c2r2 = (z_far + z_near) / (z_near - z_far);
        let c2r3 = -1.0;

        let c3r0 = 0.0;
        let c3r1 = 0.0;
        let c3r2 = (2.0 * z_far * z_near) / (z_near - z_far);
        let c3r3 = 0.0;

        #[cfg_attr(rustfmt, rustfmt_skip)]
        Mat4::from_cols_array(&[
            c0r0, c0r1, c0r2, c0r3,
            c1r0, c1r1, c1r2, c1r3,
            c2r0, c2r1, c2r2, c2r3,
            c3r0, c3r1, c3r2, c3r3,
        ])
    }
}


pub mod rand {
    pub use rand::prelude::*;

    pub fn range(from: f32, to: f32) -> f32 {
        from + (to - from) * random::<f32>()
    }
}
