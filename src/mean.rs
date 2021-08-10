// Copyright (c) EoE & Nephren 2020-2021. All rights reserved.

use failure::{bail, format_err, Error};
use half::f16;
use vapoursynth::core::CoreRef;
use vapoursynth::plugins::{Filter, FrameContext};
use vapoursynth::prelude::*;
use vapoursynth::video_info::VideoInfo;
use crate::{property, PLUGIN_NAME};
use crate::common::*;

/*
Couple notes on this following section,

Internally, we're using f64 to do the calculations, and returning the same bitdepth as we input in the first place leads to a rounding error.
However, if we allow outputting at a higher bitdepth than we started at, then we lose (well, a significant portion of) that error.
This means we can get a high quality output, using lots of far smaller 8 bit clips, rather than lots of 16 bit clips, which are twice as large.

Q: Okay so why's there a f16 down there since **litterally nobody** uses 16 bit floats?
A: f16's are actually stored as two bytes on the CPU, so this is actually worth using *if* you want to do the calculations in float for some reason.
   Why you would want to, idk, but it would work, and it'd again be less ram than the alternative.
*/

// Reusing Vecs:
// Collecting a vec from an iterator inside a loop allocates a new vec (not surprising).
// However, the vec gets deallocated at the end of the loop iteration. This means that every iteration,
// a vec gets allocated, and then deallocated, which is slow (at least, according to benchmarks).
// The idea with reusing vecs is to allocate a vec once, and inside the loop, it gets filled and cleared.
// Filling is done using `extend`, which takes an iterator. Clearing is done by "unsafely" setting the length to 0.

// `set_len` SAFETY:
// `set_len` directly sets the length of a vec. It is used here to "clear" a vec.
// Compared to using the `clear` method, `set_len` does not run the drop code of the cleared elements.
// In this case, the elements stored in the vec do not have special drop code. Therefore, it is safe to do so.

macro_rules! mean_int {
    ($($fname:ident($depth:ty, $internal:ty);)*) => {
        $(
            pub fn $fname(out_frame: &mut FrameRefMut, src_frames: &[FrameRef]) {
                // See note on reusing vecs.
                let mut src_rows = Vec::with_capacity(src_frames.len());

                // `out_frame` has the same format as the input clips
                let format = out_frame.format();

                for plane in 0..format.plane_count() {
                    for row in 0..out_frame.height(plane) {
                        // Vec reuse: filling
                        src_rows.extend(src_frames
                            .iter()
                            .map(|f| f.plane_row::<$depth>(plane, row)));
                        for (i, pixel) in out_frame.plane_row_mut::<$depth>(plane, row).iter_mut().enumerate() {
                            let sum: $internal = src_rows
                                .iter()
                                .map(|f| f[i] as $internal)
                                .sum();
                            unsafe { std::ptr::write(pixel, (sum / src_frames.len() as $internal) as $depth) }
                        }
                        // Vec reuse: (unsafe) clearing; see `set_len` SAFETY.
                        unsafe { src_rows.set_len(0); }
                    }
                }
            }
        )*
    };
}

macro_rules! mean_int_discard {
    ($($fname:ident($depth:ty, $internal:ty);)*) => {
        $(
            pub fn $fname(out_frame: &mut FrameRefMut, src_frames: &[FrameRef], discard: usize) {
                // See note on reusing vecs.
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
                            unsafe { ultra_pepega(&mut values, discard); }
                            let sum: $internal = values.drain(0..src_frames.len() - discard*2).sum();
                            unsafe { std::ptr::write(pixel, (sum / (src_frames.len() - discard * 2) as $internal) as $depth) }
                            // Vec reuse: (unsafe) clearing; see `set_len` SAFETY.
                            unsafe { values.set_len(0); }
                        }
                        // Vec reuse: (unsafe) clearing; see `set_len` SAFETY.
                        unsafe { src_rows.set_len(0); }
                    }
                }
            }
        )*
    };
}

pub struct Mean<'core> {
    // vector of our input clips
    pub clips: Vec<Node<'core>>,
    // IPB muiltiplier ratios
    pub weights: Option<[f64; 3]>,
    pub discard: Option<usize>,
}

impl<'core> Mean<'core> {
    pub fn weighted_mean<T: F64Convertible>(out_frame: &mut FrameRefMut, src_frames: &[FrameRef], weights: [f64; 3]) {
        let weights: Vec<_> = src_frames
            .iter()
            .map(|f| f.props().get::<&'_ [u8]>("_PictType").unwrap_or(b"U")[0])
            .map(|p| match p {
                b'I' | b'i' => weights[0],
                b'P' | b'p' => weights[1],
                b'B' => weights[2],
                _ => 1.0,
            })
            .collect();

        // we do the division once outside of the loop so we only need multiplication in the inner loop
        let reciprocal = 1.0 / weights.iter().sum::<f64>();

        // See note on reusing vecs.
        let mut src_rows = Vec::with_capacity(src_frames.len());

        // `out_frame` has the same format as the input clips
        let format = out_frame.format();

        for plane in 0..format.plane_count() {
            for row in 0..out_frame.height(plane) {
                // Vec reuse: filling
                src_rows.extend(src_frames
                    .iter()
                    .map(|f| f.plane_row::<T>(plane, row)));
                for (i, pixel) in out_frame.plane_row_mut::<T>(plane, row).iter_mut().enumerate() {
                    let weighted_sum: f64 = src_rows
                        .iter()
                        .map(|f| f[i].to_f64())
                        .zip(weights.iter())
                        .map(|(p, w)| p * w)
                        .sum();
                    unsafe { std::ptr::write(pixel, F64Convertible::from_f64(weighted_sum * reciprocal)) }
                }
                // Vec reuse: (unsafe) clearing; see `set_len` SAFETY.
                unsafe { src_rows.set_len(0); }
            }
        }
    }

    pub fn mean_float_discard<T: F64Convertible>(out_frame: &mut FrameRefMut, src_frames: &[FrameRef], discard: usize) {
        let reciprocal = 1.0 / (src_frames.len() - discard * 2) as f64;

        // See note on reusing vecs.
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
                    unsafe { ultra_pepega(&mut values, discard); }
                    let sum: f64 = values.drain(0..src_frames.len() - discard*2).sum();
                    unsafe { std::ptr::write(pixel, F64Convertible::from_f64(sum * reciprocal)) }
                    // Vec reuse: (unsafe) clearing; see `set_len` SAFETY.
                    unsafe { values.set_len(0); }
                }
                // Vec reuse: (unsafe) clearing; see `set_len` SAFETY.
                unsafe { src_rows.set_len(0); }
            }
        }
    }

    pub fn mean_float<T: F64Convertible>(out_frame: &mut FrameRefMut, src_frames: &[FrameRef]) {
        let reciprocal = 1.0 / src_frames.len() as f64;

        // See note on reusing vecs.
        let mut src_rows = Vec::with_capacity(src_frames.len());

        // `out_frame` has the same format as the input clips
        let format = out_frame.format();

        for plane in 0..format.plane_count() {
            for row in 0..out_frame.height(plane) {
                // Vec reuse: filling
                src_rows.extend(src_frames
                    .iter()
                    .map(|f| f.plane_row::<T>(plane, row)));
                for (i, pixel) in out_frame.plane_row_mut::<T>(plane, row).iter_mut().enumerate() {
                    let sum: f64 = src_rows
                        .iter()
                        .map(|f| f[i].to_f64())
                        .sum();
                    unsafe { std::ptr::write(pixel, F64Convertible::from_f64(sum * reciprocal)) }
                }
                // Vec reuse: (unsafe) clearing; see `set_len` SAFETY.
                unsafe { src_rows.set_len(0); }
            }
        }
    }

    mean_int! {
        mean_u8(u8, u16);
        mean_u16(u16, u32);
        mean_u32(u32, u64);
    }

    mean_int_discard! {
        mean_u8_discard(u8, u16);
        mean_u16_discard(u16, u32);
        mean_u32_discard(u32, u64);
    }
}

impl<'core> Filter<'core> for Mean<'core> {
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
        // request frame filters fro all clips
        self.clips
            .iter()
            .for_each(|f| f.request_frame_filter(context, n));
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


        let src_frames = self
            .clips
            .iter()
            .map(|f| f.get_frame_filter(context, n).ok_or_else(|| format_err!("Could not retrieve source frame")))
            .collect::<Result<Vec<_>, _>>()?;

        let prop_src = Some(&*src_frames[0]);
        let mut out_frame = unsafe { FrameRefMut::new_uninitialized(core, prop_src, format, resolution) };

        // match input sample type and bits per sample
        match (self.weights, self.discard) {
            (Some(weights), None) => match (format.sample_type(), format.bits_per_sample()) {
                (SampleType::Integer,       8) => Self::weighted_mean::<u8> (&mut out_frame, &src_frames, weights),
                (SampleType::Integer,  9..=16) => Self::weighted_mean::<u16>(&mut out_frame, &src_frames, weights),
                (SampleType::Integer, 17..=32) => Self::weighted_mean::<u32>(&mut out_frame, &src_frames, weights),
                (SampleType::Float,        16) => Self::weighted_mean::<f16>(&mut out_frame, &src_frames, weights),
                (SampleType::Float,        32) => Self::weighted_mean::<f32>(&mut out_frame, &src_frames, weights),
                (sample_type, bits_per_sample) =>
                    bail!("{}: input depth {} not supported for sample type {}", PLUGIN_NAME, bits_per_sample, sample_type),
            },
            (None, Some(discard)) => match (format.sample_type(), format.bits_per_sample()) {
                (SampleType::Integer,       8) => Self::mean_u8_discard(&mut out_frame, &src_frames, discard),
                (SampleType::Integer,  9..=16) => Self::mean_u16_discard(&mut out_frame, &src_frames, discard),
                (SampleType::Integer, 17..=32) => Self::mean_u32_discard(&mut out_frame, &src_frames, discard),
                (SampleType::Float,        16) => Self::mean_float_discard::<f16>(&mut out_frame, &src_frames, discard),
                (SampleType::Float,        32) => Self::mean_float_discard::<f32>(&mut out_frame, &src_frames, discard),
                (sample_type, bits_per_sample) =>
                    bail!("{}: input depth {} not supported for sample type {}", PLUGIN_NAME, bits_per_sample, sample_type),
            },
            (None, None) => match (format.sample_type(), format.bits_per_sample()) {
                (SampleType::Integer,       8) => Self::mean_u8 (&mut out_frame, &src_frames),
                (SampleType::Integer,  9..=16) => Self::mean_u16(&mut out_frame, &src_frames),
                (SampleType::Integer, 17..=32) => Self::mean_u32(&mut out_frame, &src_frames),
                (SampleType::Float,        16) => Self::mean_float::<f16>(&mut out_frame, &src_frames),
                (SampleType::Float,        32) => Self::mean_float::<f32>(&mut out_frame, &src_frames),
                (sample_type, bits_per_sample) =>
                    bail!("{}: input depth {} not supported for sample type {}", PLUGIN_NAME, bits_per_sample, sample_type),
            },
            (Some(_), Some(_)) =>
                bail!("Tried to use weighting and discard. This shouldn't be possible."),
        }

        // return our resulting frame
        Ok(out_frame.into())
    }
}
