// Copyright (c) EoE & Nephren 2020. All rights reserved.

use failure::{Error, bail, format_err};
use half::f16;
use vapoursynth::prelude::*;
use vapoursynth::core::CoreRef;
use vapoursynth::plugins::{Filter, FrameContext};
use vapoursynth::video_info::{VideoInfo, Property};
use crate::{PLUGIN_NAME, loop_frame_func, property};
use crate::common::*;

// macro for the int based mean filter. $op and $n are for bitshifting for conversions between different bit depths (this could be done using negative bitshifts too)
macro_rules! mean_func {
    ($($fname:ident<$depth_in:ty => $depth_out:ty>($depth_in_to_f64:path, $f64_to_depth_out:path);)*) => {
        $(
            loop_frame_func! {
                $fname<$depth_in, $depth_out>(src_frames, src_rows, i, pixel, props, multipliers) {
                    let mut total = 0.0;
                    let weighted = src_rows.iter()
                        .map(|f| $depth_in_to_f64(f[i]))
                        .enumerate()
                        .map(|(p, f)| match props[p] {
                            b'I' | b'i' => { total += multipliers[0]; f * multipliers[0] },
                            b'P' | b'p' => { total += multipliers[1]; f * multipliers[1] },
                            b'B' => { total += multipliers[2]; f * multipliers[2] },
                            _ => { total += 1.0; f * 1.0 },
                        });
    
                    *pixel = $f64_to_depth_out(weighted.sum::<f64>() / total);
                }
            }
        )*
    };
}

/* 
Couple notes on this following section,

Internally, we're using f64 to do the calculations, and returning the same bitdepth as we input in the first place leads to a rounding error.
However, if we allow outputting at a higher bitdepth than we started at, then we lose (well, a significant portion of) that error.
This means we can get a high quality output, using lots of far smaller 8 bit clips, rather than lots of 16 bit clips, which are twice as large.

Q: Why aren't all bit depths implemented? 
A: A) it'd be long, and bloat the plugin more, + b) who uses 9 bit clips anyway?

Q: In that case, why did you bother to implement 10 and 12 bit?
A: Because the user might be working with HDR, and therefore/or 10 bit encodes, where they'd need to convert their source to 16 bit before inputting. which is a pain over multiple sources.

Q: So why haven't you implemented 10 or 12 bit output?
A: mainly because I couldn't be bothered, and because internally samples are still stored as u16, just with leading 0s. If the end user is going to directly output from this, and they want 10 bit,
   they should really output 16 bit from average.Mean, and then dither down to 10 bit using resize.Point or similar.

Q: Okay so why's there a f16 down there since **litterally nobody** uses 16 bit floats?
A: f16's are actually stored as two bytes on the CPU, so this is actually worth using *if* you want to do the calculations in float for some reason.
   Why you would want to, idk, but it would work, and it'd again be less ram than the alternative.
*/

mean_func! {
    // Construction of integer based filters

    // 8 bit functions
    mean_u8_u8<u8 => u8>(u8_u8_to_f64, f64_to_u8);
    mean_u8_u16<u8 => u16>(u8_u16_to_f64, f64_to_u16);
    mean_u8_u32<u8 => u32>(u8_u32_to_f64, f64_to_u32);

    // 10 bit functions
    mean_u10_u8<u16 => u8>(u10_u8_to_f64, f64_to_u8);
    mean_u10_u16<u16 => u16>(u10_u16_to_f64, f64_to_u16);
    mean_u10_u32<u16 => u32>(u10_u32_to_f64, f64_to_u32);

    // 12 bit functions
    mean_u12_u8<u16 => u8>(u12_u8_to_f64, f64_to_u8);
    mean_u12_u16<u16 => u16>(u12_u16_to_f64, f64_to_u16);
    mean_u12_u32<u16 => u32>(u12_u32_to_f64, f64_to_u32);

    // 16 bit functions
    mean_u16_u8<u16 => u8>(u16_u8_to_f64, f64_to_u8);
    mean_u16_u16<u16 => u16>(u16_u16_to_f64, f64_to_u16);
    mean_u16_u32<u16 => u32>(u16_u32_to_f64, f64_to_u32);

    // 32 bit functions
    mean_u32_u8<u32 => u8>(u32_u8_to_f64, f64_to_u8);
    mean_u32_u16<u32 => u16>(u32_u16_to_f64, f64_to_u16);
    mean_u32_u32<u32 => u32>(u32_u32_to_f64, f64_to_u32);

    // Construction of floating point based filters
    // we're using u16's here instead of f16's, because rust doesn't implement a half precision float.
    mean_f16_f16<f16 => f16>(f16_to_f64, f64_to_f16);
    mean_f16_f32<f16 => f32>(f16_to_f64, f64_to_f32);
    mean_f32_f16<f32 => f16>(f32_to_f64, f64_to_f16);
    mean_f32_f32<f32 => f32>(f32_to_f64, f64_to_f32);
}

pub struct Mean<'core> {
    // vector of our input clips
    pub clips: Vec<Node<'core>>,
    // output bitdepth
    pub output_depth: u8, 
    // IPB muiltiplier ratios
    pub multipliers: [f64; 3],
}

impl<'core> Filter<'core> for Mean<'core> {
    fn video_info(&self, _: API, core: CoreRef<'core>) -> Vec<VideoInfo<'core>> {
        // Only change between the input and the output is the format, which is constructed below
        let VideoInfo { format, framerate, resolution, num_frames, flags } = self.clips[0].info();
        
        // register the format for the output --> this needs to be done in case the output format doesn't yet exists
        let format_in = property!(format);
        let format_out = core.register_format(
            format_in.color_family(), 
            format_in.sample_type(), 
            self.output_depth, 
            format_in.sub_sampling_w(), 
            format_in.sub_sampling_h(),
        ).unwrap(); // safe to unwrap since inputs were sanity checked in lib.rs

        vec![VideoInfo { format: Property::Constant(format_out), framerate, resolution, num_frames, flags }]
    }

    fn get_frame_initial(
        &self,
        _: API,
        _: CoreRef<'core>,
        context: FrameContext,
        n: usize,
    ) -> Result<Option<FrameRef<'core>>, Error> {
        // request frame filters fro all clips
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

        // register the format for the output --> this will now exist, but this is the easist way to get the Format back anyway.
        let format_out = core.register_format(
            format.color_family(), 
            format.sample_type(), 
            self.output_depth, 
            format.sub_sampling_w(), 
            format.sub_sampling_h(),
        ).unwrap();

        // construct our output frame
        let mut frame = unsafe { FrameRefMut::new_uninitialized(core, None, format_out, resolution) };
        let src_frames = self.clips.iter()
            .map(|f| f.get_frame_filter(context, n).ok_or_else(|| format_err!("Could not retrieve source frame")))
            .collect::<Result<Vec<_>, _>>()?;

        // this is the frame of the first source, not the first frame of the clip. Bad naming, blame Nephren
        let first_frame = &src_frames[0];

        // match input sample type to Integer or Float
        match first_frame.format().sample_type() {
            SampleType::Integer => {
                let input_depth = property!(info.format).bits_per_sample();
                // match input and output depths to correct functions
                match (input_depth, self.output_depth) {
                    (8, 8)  =>  mean_u8_u8   (&mut frame, &src_frames, &self.multipliers),
                    (8, 16) =>  mean_u8_u16  (&mut frame, &src_frames, &self.multipliers),
                    (8, 32) =>  mean_u8_u32  (&mut frame, &src_frames, &self.multipliers),
                    
                    (10, 8)  => mean_u10_u8  (&mut frame, &src_frames, &self.multipliers),
                    (10, 16) => mean_u10_u16 (&mut frame, &src_frames, &self.multipliers),
                    (10, 32) => mean_u10_u32 (&mut frame, &src_frames, &self.multipliers),

                    (12, 8)  => mean_u12_u8  (&mut frame, &src_frames, &self.multipliers),
                    (12, 16) => mean_u12_u16 (&mut frame, &src_frames, &self.multipliers),
                    (12, 32) => mean_u12_u32 (&mut frame, &src_frames, &self.multipliers),

                    (16, 8)  => mean_u16_u8  (&mut frame, &src_frames, &self.multipliers),
                    (16, 16) => mean_u16_u16 (&mut frame, &src_frames, &self.multipliers),
                    (16, 32) => mean_u16_u32 (&mut frame, &src_frames, &self.multipliers),


                    (32, 8)  => mean_u32_u8  (&mut frame, &src_frames, &self.multipliers),
                    (32, 16) => mean_u32_u16 (&mut frame, &src_frames, &self.multipliers),
                    (32, 32) => mean_u32_u32 (&mut frame, &src_frames, &self.multipliers),
                    // catch all case for if none of the others matched. Theroetically this shouldn't be reachable.
                    _ => bail!("{}: input depth {} not supported with output depth {}", PLUGIN_NAME, input_depth, self.output_depth),
                }
            },
            SampleType::Float => {
                let input_depth = property!(info.format).bits_per_sample();
                match (input_depth, self.output_depth) {
                    (16, 16) => mean_f16_f16(&mut frame, &src_frames, &self.multipliers),
                    (16, 32) => mean_f16_f32(&mut frame, &src_frames, &self.multipliers),
                    
                    (32, 16) => mean_f32_f16(&mut frame, &src_frames, &self.multipliers),
                    (32, 32) => mean_f32_f32(&mut frame, &src_frames, &self.multipliers),
                    // catch all case for if none of the others matched. Theroetically this shouldn't be reachable.
                    _ => bail!("{}: input depth {} not supported with output depth {}", PLUGIN_NAME, input_depth, self.output_depth),
                }
            },
        }

        // return our resulting frame
        Ok(frame.into())
    }
}

