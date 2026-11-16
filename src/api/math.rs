use std::ops::{Add, Div, Mul, Sub};
use vstd::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[derive(Clone, Copy)]
pub struct Mat2 {
    pub m: [[f32; 2]; 2],
}

#[derive(Clone, Copy)]
pub struct Mat3 {
    pub m: [[f32; 3]; 3],
}

#[derive(Clone, Copy)]
pub struct Mat4 {
    pub m: [[f32; 4]; 4], // Corrected to 4x4 matrix
}

impl Add for Vec2 {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        Self {
            x: self.x + rhs.x,
            y: self.y + rhs.y, // Fixed: changed self.x to self.y
        }
    }
}

impl Sub for Vec2 {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self {
        Self {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
        }
    }
}

impl Mul for Vec2 {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self {
        Self {
            x: self.x * rhs.x,
            y: self.y * rhs.y,
        }
    }
}

impl Mul<Mat2> for Vec2 {
    type Output = Self;

    fn mul(self, rhs: Mat2) -> Self {
        Self {
            x: rhs.m[0][0] * self.x + rhs.m[0][1] * self.y,
            y: rhs.m[1][0] * self.x + rhs.m[1][1] * self.y,
        }
    }
}

impl Div for Vec2 {
    type Output = Self;

    verus! {
        fn div(self, rhs: Vec2) -> Self
            requires
                rhs.x > 0.0, // Fixed: used f32 literal 0.0
                rhs.y > 0.0,
        {
            Self {
                x: self.x / rhs.x,
                y: self.y / rhs.y,
            }
        }
    }
}

impl Add for Vec3 {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        Self {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
            z: self.z + rhs.z,
        }
    }
}

impl Sub for Vec3 {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self {
        Self {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
            z: self.z - rhs.z,
        }
    }
}

impl Mul for Vec3 {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self {
        Self {
            x: self.x * rhs.x,
            y: self.y * rhs.y,
            z: self.z * rhs.z,
        }
    }
}

impl Mul<Mat3> for Vec3 {
    type Output = Self;

    fn mul(self, rhs: Mat3) -> Self {
        Self {
            x: rhs.m[0][0] * self.x + rhs.m[0][1] * self.y + self.z * rhs.m[0][2],
            y: rhs.m[1][0] * self.x + rhs.m[1][1] * self.y + self.z * rhs.m[1][2],
            z: rhs.m[2][0] * self.x + rhs.m[2][1] * self.y + self.z * rhs.m[2][2],
        }
    }
}

impl Div for Vec3 {
    type Output = Self;

    verus! {
        fn div(self, rhs: Self) -> Self
            requires
                rhs.x > 0.0,
                rhs.y > 0.0,
                rhs.z > 0.0,
        {
            Self {
                x: self.x / rhs.x,
                y: self.y / rhs.y,
                z: self.z / rhs.z,
            }
        }
    }
}
