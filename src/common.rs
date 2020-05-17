// Copyright (c) EoE & Nephren 2020. All rights reserved.

//! Common code

use half::f16;
use vapoursynth::prelude::*;

// Code involving parallel iterators are cursed, especially the `FrameRefMutPointer`
// newtype wrapper. Read at your own risk.
//
// Is the `parallel` version of the code safe?
// Unless I overlooked something, this should be safe.
// The only operation that mutates values is the `for_each` in `plane_row_mut`,
// and unless `plane_row_mut` returns overlapping slices for different planes and rows,
// there should be no aliasing.

pub struct FrameRefMutPointer<'core>(pub *const FrameRefMut<'core>);
unsafe impl<'core> Send for FrameRefMutPointer<'core> {}
unsafe impl<'core> Sync for FrameRefMutPointer<'core> {}

#[macro_export]
macro_rules! loop_frame_func {

    // prop / multiplier macros

    ($name:ident<$bits_per_sample_in:ty, $bits_per_sample_out:ty>($src_clips:ident, $src_rows:ident, $i:ident, $pixel:ident, $props:ident, $multipliers:ident) $func:block) => {
        #[cfg(not(feature = "parallel"))]
        pub fn $name(frame: &mut FrameRefMut, $src_clips: &[FrameRef], $multipliers: &[f64]) {
            let first_frame = &$src_clips[0];
            let $props = $src_clips.iter().map(|f| f.props().get::<&'_[u8]>("_PictType").unwrap_or(b"U")[0]).collect::<Vec<_>>(); 
            for plane in 0..first_frame.format().plane_count() {
                for row in 0..first_frame.height(plane) {
                    let $src_rows = $src_clips.iter().map(|f| f.plane_row::<$bits_per_sample_in>(plane, row)).collect::<Vec<_>>();
                    for ($i, $pixel) in frame.plane_row_mut::<$bits_per_sample_out>(plane, row).iter_mut().enumerate() {
                        $func
                    }
                }
            }
        }

        #[cfg(feature = "parallel")]
        pub fn $name(frame: &mut FrameRefMut, $src_clips: &[FrameRef], $multipliers: &[f64]) {
            use ::rayon::prelude::*;
            use $crate::common::FrameRefMutPointer;

            let first_frame = &$src_clips[0];
            let frame = FrameRefMutPointer(frame as *mut _ as *const _);
            let $props = $src_clips.iter().map(|f| f.props().get::<&'_[u8]>("_PictType").unwrap_or(b"U")[0]).collect::<Vec<_>>();
            (0..first_frame.format().plane_count()).into_par_iter()
                .for_each(|plane| {
                    (0..first_frame.height(plane)).into_par_iter()
                        .for_each(|row| {
                            let frame = unsafe { &mut *(frame.0 as *mut FrameRefMut) };
                            let $src_rows = $src_clips.par_iter().map(|f| f.plane_row::<$bits_per_sample_in>(plane, row)).collect::<Vec<_>>();
                            frame.plane_row_mut::<$bits_per_sample_out>(plane, row)
                                .par_iter_mut()
                                .enumerate()
                                .for_each(|($i, $pixel)| {
                                    $func
                                });
                        });
                });
        }
    };

    // ==============================================================
    // non prop / multiplier macros

    ($name:ident<$bits_per_sample_in:ty, $bits_per_sample_out:ty>($src_clips:ident, $src_rows:ident, $i:ident, $pixel:ident) $func:block) => {
        #[cfg(not(feature = "parallel"))]
        pub fn $name(frame: &mut FrameRefMut, $src_clips: &[FrameRef]) {
            let first_frame = &$src_clips[0];
            for plane in 0..first_frame.format().plane_count() {
                for row in 0..first_frame.height(plane) {
                    let $src_rows = $src_clips.iter().map(|f| f.plane_row::<$bits_per_sample_in>(plane, row)).collect::<Vec<_>>();
                    for ($i, $pixel) in frame.plane_row_mut::<$bits_per_sample_out>(plane, row).iter_mut().enumerate() {
                        $func
                    }
                }
            }
        }

        #[cfg(feature = "parallel")]
        pub fn $name(frame: &mut FrameRefMut, $src_clips: &[FrameRef]) {
            use ::rayon::prelude::*;
            use $crate::common::FrameRefMutPointer;

            let first_frame = &$src_clips[0];
            let frame = FrameRefMutPointer(frame as *mut _ as *const _);
            (0..first_frame.format().plane_count()).into_par_iter()
                .for_each(|plane| {
                    (0..first_frame.height(plane)).into_par_iter()
                        .for_each(|row| {
                            let frame = unsafe { &mut *(frame.0 as *mut FrameRefMut) };
                            let $src_rows = $src_clips.par_iter().map(|f| f.plane_row::<$bits_per_sample_in>(plane, row)).collect::<Vec<_>>();
                            frame.plane_row_mut::<$bits_per_sample_out>(plane, row)
                                .par_iter_mut()
                                .enumerate()
                                .for_each(|($i, $pixel)| {
                                    $func
                                });
                        });
                });
        }
    };
}

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
    u8_u16_to_f64<u8>(<< 8);
    u8_u32_to_f64<u8>(<< 24);

    // 10 bit functions
    u10_u8_to_f64<u16>(>> 2);
    u10_u16_to_f64<u16>(<< 4);
    u10_u32_to_f64<u16>(<< 22);

    // 12 bit functions
    u12_u8_to_f64<u16>(>> 4);
    u12_u16_to_f64<u16>(<< 2);
    u12_u32_to_f64<u16>(<< 20);

    // 16 bit functions
    u16_u8_to_f64<u16>(>> 8);
    u16_u16_to_f64<u16>(<< 0);
    u16_u32_to_f64<u16>(<< 16);

    // 32 bit functions
    u32_u8_to_f64<u32>(>> 24);
    u32_u16_to_f64<u32>(>> 16);
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
pub fn f16_to_f64(n: u16) -> f64 { f16::from_bits(n).to_f64() }

#[inline]
pub fn f64_to_f16(n: f64) -> u16 { f16::from_f64(n).to_bits() }
