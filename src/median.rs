// Copyright (c) EoE & Nephren 2020. All rights reserved.

use failure::{Error, bail, format_err};
use vapoursynth::prelude::*;
use vapoursynth::core::CoreRef;
use vapoursynth::plugins::{Filter, FrameContext};
use vapoursynth::video_info::VideoInfo;
use crate::{PLUGIN_NAME, loop_frame_func, property};

macro_rules! median_int {
    ($($fname:ident<$depth:ty>;)*) => {
        $(
            loop_frame_func! {
                $fname<$depth, $depth>(src_frames, src, i, pixel) {
                    let mut values = src.iter().map(|f| f[i]).collect::<Vec<_>>();
                    values.sort_unstable();
    
                    let median = if values.len() & 1 == 1 {
                        // odd length
                        values[(values.len() - 1) / 2]
                    } else {
                        // even length
                        let middle = values.len() / 2;
                        (values[middle - 1] + values[middle]) / 2
                    };
    
                    *pixel = median;
                }
            }
        )*
    };
}

median_int! {
    median_u8<u8>;
    median_u16<u16>;
    median_u32<u32>;
}

loop_frame_func! {
    median_float<f32, f32>(src_frames, src, i, pixel) {
        let mut values = src.iter().map(|f| f[i]).collect::<Vec<_>>();
        values.sort_unstable_by(|a, b| a.partial_cmp(b).unwrap());
        
        let median = if values.len() & 1 == 1 {
            // odd length
            values[(values.len() - 1) / 2]
        } else {
            // even length
            let middle = values.len() / 2;
            (values[middle - 1] + values[middle]) / 2.0
        };
        
        *pixel = median;
    }
}

pub struct Median<'core> {
    pub clips: Vec<Node<'core>>,
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

        let mut frame = unsafe { FrameRefMut::new_uninitialized(core, None, format, resolution) };
        let src_frames = self.clips.iter()
            .map(|f| f.get_frame_filter(context, n).ok_or_else(|| format_err!("Could not retrieve source frame")))
            .collect::<Result<Vec<_>, _>>()?;

        let first_frame = &src_frames[0];

        match first_frame.format().sample_type() {
            SampleType::Integer => {
                let depth = property!(info.format).bits_per_sample();
                match depth {
                    0..=8 => median_u8(&mut frame, &src_frames),
                    9..=16 => median_u16(&mut frame, &src_frames),
                    17..=32 => median_u32(&mut frame, &src_frames),
                    _ => bail!("{}: input depth {} not supported", PLUGIN_NAME, depth),
                }
            },
            SampleType::Float => median_float(&mut frame, &src_frames),
        }

        Ok(frame.into())
    }
}
