use std::ops::{Add, Mul, Sub};

#[derive(Clone, Copy)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

#[derive(Clone, Copy)]
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
    pub m: [[f32; 4]; 3],
}

impl Add for Vec2 {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        Self {
            x: self.x + rhs.x,
            y: self.x + rhs.y,
        }
    }
}

impl Sub for Vec2 {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self {
        Self {
            x: self.x - rhs.x,
            y: self.x - rhs.y,
        }
    }
}

impl Mul for Vec2 {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self {
        Self {
            x: self.x * rhs.x,
            y: self.x * rhs.y,
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
