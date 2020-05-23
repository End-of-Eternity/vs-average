// Copyright (c) EoE & Nephren 2020. All rights reserved.

//! Common code

use half::f16;

// Conversion functions to and from f64

macro_rules! int_to_f64 {
    ($($fname:ident<$int:ty>($op:tt $n:literal);)*) => {
        $(
            #[inline]
            pub fn $fname(n: $int) -> f64 { ((n as u64) $op $n) as f64 }
        )*
    };
}

macro_rules! f64_to_int {
    ($($fname:ident<$int:ty>;)*) => {
        $(
            #[inline]
            pub fn $fname(n: f64) -> $int { n as $int }
        )*
    };
}

int_to_f64! {
    u8_u8_to_f64<u8>(<< 0);
    u10_u16_to_f64<u16>(<< 4);
    u12_u16_to_f64<u16>(<< 2);
    u16_u16_to_f64<u16>(<< 0);
    u32_u32_to_f64<u32>(<< 0);
}

f64_to_int! {
    f64_to_u8<u8>;
    f64_to_u16<u16>;
    f64_to_u32<u32>;
}

#[inline]
pub fn f32_to_f64(n: f32) -> f64 { n as f64 }

#[inline]
pub fn f64_to_f32(n: f64) -> f32 { n as f32 }

#[inline]
pub fn f16_to_f64(n: f16) -> f64 { n.to_f64() }

#[inline]
pub fn f64_to_f16(n: f64) -> f16 { f16::from_f64(n) }
