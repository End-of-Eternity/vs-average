// Copyright (c) EoE & Nephren 2020. All rights reserved.

use failure::{Error, bail, format_err};
use half::f16;
use vapoursynth::prelude::*;
use vapoursynth::core::CoreRef;
use vapoursynth::plugins::{Filter, FrameContext};
use vapoursynth::video_info::VideoInfo;
use crate::{PLUGIN_NAME, property};
use crate::common::*;

macro_rules! mean {
    ($($fname:ident<$depth:ty>($depth_in_to_f64:path, $f64_to_depth_out:path);)*) => {
        $(
            pub fn $fname(frame: &mut FrameRefMut, src_clips: &[FrameRef], multipliers: &[f64; 3]) {
                let first_frame = &src_clips[0];
                let props = src_clips.iter().map(|f| f.props().get::<&'_[u8]>("_PictType").unwrap_or(b"U")[0]).collect::<Vec<_>>(); 
                for plane in 0..first_frame.format().plane_count() {
                    for row in 0..first_frame.height(plane) {
                        let src_rows = src_clips.iter().map(|f| f.plane_row::<$depth>(plane, row)).collect::<Vec<_>>();
                        for (i, pixel) in frame.plane_row_mut::<$depth>(plane, row).iter_mut().enumerate() {
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

mean! {
    mean_u8<u8>(u8_u8_to_f64, f64_to_u8);
    mean_u10<u16>(u10_u16_to_f64, f64_to_u16);
    mean_u12<u16>(u12_u16_to_f64, f64_to_u16);
    mean_u16<u16>(u16_u16_to_f64, f64_to_u16);
    mean_u32<u32>(u32_u32_to_f64, f64_to_u32);

    mean_f16<f16>(f16_to_f64, f64_to_f16);
    mean_f32<f32>(f32_to_f64, f64_to_f32);
}

pub struct Mean<'core> {
    // vector of our input clips
    pub clips: Vec<Node<'core>>,
    // IPB muiltiplier ratios
    pub multipliers: [f64; 3],
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

        // construct our output frame
        let mut frame = unsafe { FrameRefMut::new_uninitialized(core, None, format, resolution) };
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
                match input_depth {
                    8  => mean_u8 (&mut frame, &src_frames, &self.multipliers),
                    10 => mean_u10(&mut frame, &src_frames, &self.multipliers),
                    12 => mean_u12(&mut frame, &src_frames, &self.multipliers),
                    16 => mean_u16(&mut frame, &src_frames, &self.multipliers),
                    32 => mean_u32(&mut frame, &src_frames, &self.multipliers),
                    // catch all case for if none of the others matched. Theroetically this shouldn't be reachable.
                    _ => bail!("{}: input depth {} not supported", PLUGIN_NAME, input_depth),
                }
            },
            SampleType::Float => {
                let input_depth = property!(info.format).bits_per_sample();
                match input_depth {
                    16 => mean_f16(&mut frame, &src_frames, &self.multipliers),
                    32 => mean_f32(&mut frame, &src_frames, &self.multipliers),
                    // catch all case for if none of the others matched. Theroetically this shouldn't be reachable.
                    _ => bail!("{}: input depth {} not supported", PLUGIN_NAME, input_depth),
                }
            },
        }

        // return our resulting frame
        Ok(frame.into())
    }
}

