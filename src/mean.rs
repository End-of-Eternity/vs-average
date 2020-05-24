// Copyright (c) EoE & Nephren 2020. All rights reserved.

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

pub struct Mean<'core> {
    // vector of our input clips
    pub clips: Vec<Node<'core>>,
    // IPB muiltiplier ratios
    pub multipliers: [f64; 3],
}

impl<'core> Mean<'core> {
    pub fn mean<T: F64Convertible>(&self, out_frame: &mut FrameRefMut, src_frames: &[FrameRef]) {
        let weights: Vec<_> = src_frames
            .iter()
            .map(|f| f.props().get::<&'_ [u8]>("_PictType").unwrap_or(b"U")[0])
            .map(|p| match p {
                b'I' | b'i' => self.multipliers[0],
                b'P' | b'p' => self.multipliers[1],
                b'B' => self.multipliers[2],
                _ => 1.0,
            })
            .collect();

        // we do the division once outside of the loop so we only need multiplication in the inner loop
        let multiplier = 1.0 / weights.iter().sum::<f64>();

        // `out_frame` has the same format as the input clips
        let format = out_frame.format();
        for plane in 0..format.plane_count() {
            for row in 0..out_frame.height(plane) {
                let src_rows: Vec<_> = src_frames
                    .iter()
                    .map(|f| f.plane_row::<T>(plane, row))
                    .collect();
                for (i, pixel) in out_frame.plane_row_mut::<T>(plane, row).iter_mut().enumerate() {
                    let weighted_sum: f64 = src_rows
                        .iter()
                        .map(|f| f[i].to_f64())
                        .zip(weights.iter())
                        .map(|(p, w)| p * w)
                        .sum();
                    unsafe { std::ptr::write(pixel, F64Convertible::from_f64(weighted_sum * multiplier)) }
                }
            }
        }
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

        // construct our output frame
        let mut frame = unsafe { FrameRefMut::new_uninitialized(core, None, format, resolution) };
        let src_frames = self
            .clips
            .iter()
            .map(|f| f.get_frame_filter(context, n).ok_or_else(|| format_err!("Could not retrieve source frame")))
            .collect::<Result<Vec<_>, _>>()?;

        // match input sample type and bits per sample
        match (format.sample_type(), format.bits_per_sample()) {
            (SampleType::Integer,       8) => self.mean::<u8> (&mut frame, &src_frames),
            (SampleType::Integer,  9..=16) => self.mean::<u16>(&mut frame, &src_frames),
            (SampleType::Integer, 17..=32) => self.mean::<u32>(&mut frame, &src_frames),
            (SampleType::Float,        16) => self.mean::<f16>(&mut frame, &src_frames),
            (SampleType::Float,        32) => self.mean::<f32>(&mut frame, &src_frames),
            (sample_type, bits_per_sample) => 
                bail!("{}: input depth {} not supported for sample type {}", PLUGIN_NAME, bits_per_sample, sample_type),
        }

        // return our resulting frame
        Ok(frame.into())
    }
}
