// Copyright (c) EoE & Nephren 2020. All rights reserved.

//! Common code

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
    ($name:ident<$bits_per_sample_in:ty, $bits_per_sample_out:ty>($src_clips:ident, $src_rows:ident, $i:ident, $pixel:ident) $func:tt) => {
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
