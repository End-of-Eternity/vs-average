// Copyright (c) EoE & Nephren 2020. All rights reserved.

#![allow(arithmetic_overflow)]

use failure::{Error, bail, format_err};
use vapoursynth::prelude::*;
use vapoursynth::core::CoreRef;
use vapoursynth::plugins::{Filter, FrameContext};
use vapoursynth::video_info::{VideoInfo, Property};
use half::f16;
use crate::{PLUGIN_NAME, loop_frame_func, property};

macro_rules! mean_int {
    ($depth_in:ty, $depth_out:ty, $op:tt, $n:literal, $fname:ident) => {
        loop_frame_func! {
            $fname<$depth_in, $depth_out>(src_clips, src_rows, i, pixel) {
                let mean = src_rows.iter().map(|f| (f[i] as u64) $op $n).sum::<u64>() / src_clips.len() as u64;
                *pixel = mean as $depth_out;
            }
        }
    };
}

// Couple notes on this section
// internally, we're using u64 to do the calculations, and returning the same bitdepth as we input in the first place leads to a rounding error
// however, if we allow outputting at a higher bitdepth than we started at, then we lose (well, a significant portion) of that error
// this means we can get a very high quality output, using lots of far smaller 8 bit clips.

// Q: why aren't all bit depths implemented? 
// A: it'd be long, and bloat the plugin more, and b) who uses 9 bit clips anyway?

// Q: In that case, why did you bother to implement 10 and 12 bit?
// A: Because the user might be working with HDR, and therefore/or 10 bit encodes, where they'd need to convert their source to 16 bit before inputting. which is a pain over multiple sources.

// Q: So why haven't you implemented 10 or 12 bit output?
// A: mainly because I couldn't be bothered, and because internally samples are still stored as u16, just with leading 0s. If the end user is going to directly output from this, and they want 10 bit,
//    they should really output 16 bit from average.Mean, and then dither down to 10 bit using resize.Point or similar.

// Q: Okay so why's there a f16 down there since **litterally nobody** uses 16 bit floats?
// A: idk man, i thought it'd be cool

// 8 bit functions
mean_int!(u8, u8, <<, 0, mean_u8_u8);
mean_int!(u8, u16, <<, 8, mean_u8_u16);
mean_int!(u8, u32, <<, 24, mean_u8_u32);

// 10 bit functions
mean_int!(u16, u8, >>, 2, mean_u10_u8);
mean_int!(u16, u16, <<, 4, mean_u10_u16);
mean_int!(u16, u32, <<, 22, mean_u10_u32);

// 12 bit functions
mean_int!(u16, u8, >>, 4, mean_u12_u8);
mean_int!(u16, u16, <<, 2, mean_u12_u16);
mean_int!(u16, u32, <<, 20, mean_u12_u32);

// 16 bit functions
mean_int!(u16, u8, >>, 8, mean_u16_u8);
mean_int!(u16, u16, <<, 0, mean_u16_u16);
mean_int!(u16, u32, <<, 16, mean_u16_u32);

// 32 bit functions
mean_int!(u32, u8, >>, 24, mean_u32_u8);
mean_int!(u32, u16, >>, 16, mean_u32_u16);
mean_int!(u32, u32, <<, 0, mean_u32_u32);

// f16 -> f16
loop_frame_func! {
    mean_f16_f16<u16, u16>(src_frames, src, i, pixel) {
        let mean = src.iter().map(|f| f16::from_bits(f[i]).to_f32()).sum::<f32>() / src_frames.len() as f32;
        *pixel = f16::from_f32(mean).to_bits();
    }
}

// f16 -> f32
loop_frame_func! {
    mean_f16_f32<u16, f32>(src_frames, src, i, pixel) {
        let mean = src.iter().map(|f| f16::from_bits(f[i]).to_f32()).sum::<f32>() / src_frames.len() as f32;
        *pixel = mean;
    }
}

// f32 -> f16
loop_frame_func! {
    mean_f32_f16<f32, u16>(src_frames, src, i, pixel) {
        let mean = src.iter().map(|f| f[i]).sum::<f32>() / src_frames.len() as f32;
        *pixel = f16::from_f32(mean).to_bits();
    }
}

// f32 -> f32
loop_frame_func! {
    mean_f32_f32<f32, f32>(src_frames, src, i, pixel) {
        let mean = src.iter().map(|f| f[i]).sum::<f32>() / src_frames.len() as f32;
        *pixel = mean;
    }
}

pub struct Mean<'core> {
    pub clips: Vec<Node<'core>>,
    pub output_depth: u8, 
}

impl<'core> Filter<'core> for Mean<'core> {
    fn video_info(&self, _: API, core: CoreRef<'core>) -> Vec<VideoInfo<'core>> {
        
        let format_in: vapoursynth::format::Format<'core> = property!(self.clips[0].info().format);
        let format_out = core.register_format(
            format_in.color_family(), 
            format_in.sample_type(), 
            self.output_depth, 
            format_in.sub_sampling_w(), 
            format_in.sub_sampling_h(),
        ).unwrap();

        let VideoInfo { framerate, resolution, num_frames, flags, .. } = self.clips[0].info();

        vec![VideoInfo {format:Property::Constant(format_out), framerate, resolution, num_frames, flags }]
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

        let format_out = core.register_format(
            format.color_family(), 
            format.sample_type(), 
            self.output_depth, 
            format.sub_sampling_w(), 
            format.sub_sampling_h(),
        ).unwrap();

        let mut frame = unsafe { FrameRefMut::new_uninitialized(core, None, format_out, resolution) };
        let src_frames = self.clips.iter()
            .map(|f| f.get_frame_filter(context, n).ok_or_else(|| format_err!("Could not retrieve source frame")))
            .collect::<Result<Vec<_>, _>>()?;

        let first_frame = &src_frames[0];

        match first_frame.format().sample_type() {
            SampleType::Integer => {
                let input_depth = property!(info.format).bits_per_sample();
                match (input_depth, self.output_depth) {
                    (8, 8)  =>  mean_u8_u8   (&mut frame, &src_frames),
                    (8, 16) =>  mean_u8_u16  (&mut frame, &src_frames),
                    (8, 32) =>  mean_u8_u32  (&mut frame, &src_frames),
                    
                    (10, 8)  => mean_u10_u8  (&mut frame, &src_frames),
                    (10, 16) => mean_u10_u16 (&mut frame, &src_frames),
                    (10, 32) => mean_u10_u32 (&mut frame, &src_frames),

                    (12, 8)  => mean_u12_u8  (&mut frame, &src_frames),
                    (12, 16) => mean_u12_u16 (&mut frame, &src_frames),
                    (12, 32) => mean_u12_u32 (&mut frame, &src_frames),

                    (16, 8)  => mean_u16_u8  (&mut frame, &src_frames),
                    (16, 16) => mean_u16_u16 (&mut frame, &src_frames),
                    (16, 32) => mean_u16_u32 (&mut frame, &src_frames),


                    (32, 8)  => mean_u32_u8  (&mut frame, &src_frames),
                    (32, 16) => mean_u32_u16 (&mut frame, &src_frames),
                    (32, 32) => mean_u32_u32 (&mut frame, &src_frames),
                    _ => bail!("{}: input depth {} not supported with output depth {}", PLUGIN_NAME, input_depth, self.output_depth),
                }
            },
            SampleType::Float => {
                let input_depth = property!(info.format).bits_per_sample();
                match (input_depth, self.output_depth) {
                    (16, 16) => mean_f16_f16(&mut frame, &src_frames),
                    (16, 32) => mean_f16_f32(&mut frame, &src_frames),
                    
                    (32, 16) => mean_f32_f16(&mut frame, &src_frames),
                    (32, 32) => mean_f32_f32(&mut frame, &src_frames),
                    _ => bail!("{}: input depth {} not supported with output depth {}", PLUGIN_NAME, input_depth, self.output_depth),
                }
            },
        }

        Ok(frame.into())
    }
}

