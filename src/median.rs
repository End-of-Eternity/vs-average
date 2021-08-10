// Copyright (c) EoE & Nephren 2020-2021. All rights reserved.

use failure::{Error, bail, format_err};
use half::f16;
use vapoursynth::prelude::*;
use vapoursynth::core::CoreRef;
use vapoursynth::plugins::{Filter, FrameContext};
use vapoursynth::video_info::VideoInfo;
use crate::common::*;
use crate::{PLUGIN_NAME, property};

// This code looks horrible.
// We need to fix it, Soon(TM).

macro_rules! median_int {
    ($($fname:ident($depth:ty, $internal:ty);)*) => {
        $(
            pub fn $fname(out_frame: &mut FrameRefMut, src_frames: &[FrameRef]) {
                let mut src_rows = Vec::with_capacity(src_frames.len());
                let mut values = Vec::with_capacity(src_frames.len());

                // `out_frame` has the same format as the input clips
                let format = out_frame.format();

                for plane in 0..format.plane_count() {
                    for row in 0..out_frame.height(plane) {
                        // Vec reuse: filling
                        src_rows.extend(src_frames
                            .iter()
                            .map(|f| f.plane_row::<$depth>(plane, row)));
                        for (i, pixel) in out_frame.plane_row_mut::<$depth>(plane, row).iter_mut().enumerate() {
                            // Vec reuse: filling
                            values.extend(src_rows
                                .iter()
                                .map(|f| f[i] as $internal));

                            values.sort_unstable();

                            let data = if values.len() & 1 == 1 {
                                values[(values.len() - 1) >> 1]
                            } else {
                                let middle = values.len() >> 1;
                                (values[middle - 1] + values[middle]) >> 1
                            };

                            unsafe { std::ptr::write(pixel, data as $depth) }

                        }
                        // Vec reuse: (unsafe) clearing; see `set_len` SAFETY.
                        unsafe { src_rows.set_len(0); }
                    }
                }
            }
        )*
    };
}

pub struct Median<'core> {
    pub clips: Vec<Node<'core>>,
}
impl<'core> Median<'core> {
    pub fn median_float<T: F64Convertible>(out_frame: &mut FrameRefMut, src_frames: &[FrameRef]) {
        // See note on reusing vecs in mean.rs
        let mut src_rows = Vec::with_capacity(src_frames.len());
        let mut values = Vec::with_capacity(src_frames.len());

        // `out_frame` has the same format as the input clips
        let format = out_frame.format();

        for plane in 0..format.plane_count() {
            for row in 0..out_frame.height(plane) {
                // Vec reuse: filling
                src_rows.extend(src_frames
                    .iter()
                    .map(|f| f.plane_row::<T>(plane, row)));
                for (i, pixel) in out_frame.plane_row_mut::<T>(plane, row).iter_mut().enumerate() {
                    // Vec reuse: filling
                    values.extend(src_rows
                        .iter()
                        .map(|f| f[i].to_f64()));

                    values.sort_unstable_by(|a, b| a.partial_cmp(b).unwrap());

                    let data = if values.len() & 1 == 1 {
                        values[(values.len() - 1) >> 1]
                    } else {
                        let middle = values.len() >> 1;
                        (values[middle - 1] + values[middle]) / 2.0
                    };

                    unsafe { std::ptr::write(pixel, F64Convertible::from_f64(data)) }
                }
                // Vec reuse: (unsafe) clearing; see `set_len` SAFETY.
                unsafe { src_rows.set_len(0); }
            }
        }
    }

    median_int! {
        median_u8(u8, u16);
        median_u16(u16, u32);
        median_u32(u32, u64);
    }

}

impl<'core> Filter<'core> for Median<'core> {
    fn video_info(&self, _: API, _: CoreRef<'core>) -> Vec<VideoInfo<'core>> {
        vec![self.clips[0].info()]
    }

    fn get_frame_initial(
        &self,
        _: API,
        _: CoreRef<'core>,
        context: FrameContext,
        n: usize,
    ) -> Result<Option<FrameRef<'core>>, Error> {
        self.clips.iter().for_each(|f| f.request_frame_filter(context, n));
        Ok(None)
    }

    fn get_frame(
        &self,
        _: API,
        core: CoreRef<'core>,
        context: FrameContext,
        n: usize,
    ) -> Result<FrameRef<'core>, Error> {
        let info = self.clips[0].info();
        let format = property!(info.format);
        let resolution = property!(info.resolution);

        let src_frames = self.clips.iter()
            .map(|f| f.get_frame_filter(context, n).ok_or_else(|| format_err!("Could not retrieve source frame")))
            .collect::<Result<Vec<_>, _>>()?;

        let prop_src = Some(&*src_frames[0]);
        let mut out_frame = unsafe { FrameRefMut::new_uninitialized(core, prop_src, format, resolution) };

        match (format.sample_type(), format.bits_per_sample()) {
            (SampleType::Integer,       8) => Self::median_u8(&mut out_frame, &src_frames),
            (SampleType::Integer,  9..=16) => Self::median_u16(&mut out_frame, &src_frames),
            (SampleType::Integer, 17..=32) => Self::median_u32(&mut out_frame, &src_frames),
            (SampleType::Float,        16) => Self::median_float::<f16>(&mut out_frame, &src_frames),
            (SampleType::Float,        32) => Self::median_float::<f32>(&mut out_frame, &src_frames),
            (sample_type, bits_per_sample) =>
                bail!("{}: input depth {} not supported for sample type {}. This shouldn't be possible", PLUGIN_NAME, bits_per_sample, sample_type),
        }

        Ok(out_frame.into())
    }
}
