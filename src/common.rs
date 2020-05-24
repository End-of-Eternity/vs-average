// Copyright (c) EoE & Nephren 2020. All rights reserved.

//! Common code

use half::f16;
use vapoursynth::component::Component;

// Conversion trait to and from f64

// This is a weird name...
pub trait F64Convertible: Sized + Copy + Component {
    fn to_f64(self) -> f64;
    fn from_f64(n: f64) -> Self;
}

impl F64Convertible for u8 {
    #[inline]
    fn to_f64(self) -> f64 {
        self as f64
    }

    #[inline]
    fn from_f64(n: f64) -> Self {
        n as u8
    }
}

impl F64Convertible for u16 {
    #[inline]
    fn to_f64(self) -> f64 {
        self as f64
    }

    #[inline]
    fn from_f64(n: f64) -> Self {
        n as u16
    }
}

impl F64Convertible for u32 {
    #[inline]
    fn to_f64(self) -> f64 {
        self as f64
    }

    #[inline]
    fn from_f64(n: f64) -> Self {
        n as u32
    }
}

impl F64Convertible for f16 {
    #[inline]
    fn to_f64(self) -> f64 {
        self.to_f64()
    }
    
    #[inline]
    fn from_f64(n: f64) -> Self {
        f16::from_f64(n)
    }
}

impl F64Convertible for f32 {
    #[inline]
    fn to_f64(self) -> f64 {
        self as f64
    }

    #[inline]
    fn from_f64(n: f64) -> Self {
        n as f32
    }
}
