// Copyright (c) EoE & Nephren 2020-2021. All rights reserved.

//! # vs-average
//!
//! A VapourSynth plugin for averaging clips together

mod mean;
mod median;
mod common;

use failure::{Error, bail, ensure};
use vapoursynth::prelude::*;
use vapoursynth::core::CoreRef;
use vapoursynth::map::ValueIter;
use vapoursynth::plugins::{Filter, FilterArgument, Metadata};
use vapoursynth::video_info::Property::Constant;
use vapoursynth::{make_filter_function, export_vapoursynth_plugin};
use self::mean::Mean;
use self::median::Median;

pub const PLUGIN_NAME: &str = "vs-average";
pub const PLUGIN_IDENTIFIER: &str = "eoe-nephren.average";

fn check_clips<'core>(clips: &[Node<'core>]) -> Result<(), Error> {
    ensure!(clips.len() > 0, "There should be at least one clip as input");
    if !clips.iter()
        .map(|s| s.info())
        .all(|i| matches!((i.format, i.framerate, i.resolution), (Constant(_), Constant(_), Constant(_))))
    {
        bail!("Variable properties in input clips are not supported");
    }

    let info = clips[0].info();
    if !clips.iter()
        .skip(1)
        .map(|s| s.info())
        .all(|i| info.format == i.format && info.framerate == i.framerate && info.resolution == i.resolution && info.num_frames == i.num_frames)
    {
        bail!("Input clips must have the same format, frame rate, resolution, and frame count");
    }

    Ok(())
}

#[macro_export]
macro_rules! property {
    ($prop:expr) => {
        match $prop {
            ::vapoursynth::video_info::Property::Constant(p) => p,
            ::vapoursynth::video_info::Property::Variable => unreachable!(),
        }
    };
}

make_filter_function! {
    MedianFunction, "Median"

    fn create_median<'core>(
        _api: API,
        _core: CoreRef<'core>,
        clips: ValueIter<'_, 'core, Node<'core>>,
    ) -> Result<Option<Box<dyn Filter<'core> + 'core>>, Error> {
        let clips = clips.collect::<Vec<_>>();
        check_clips(&clips)?;

        Ok(Some(Box::new(Median { clips })))
    }
}

make_filter_function! {
    MeanFunction, "Mean"

    fn create_mean<'core>(
        _api: API,
        _core: CoreRef<'core>,
        clips: ValueIter<'_, 'core, Node<'core>>,
        preset: Option<i64>,
        discard: Option<i64>,
    ) -> Result<Option<Box<dyn Filter<'core> + 'core>>, Error> {
        let clips = clips.collect::<Vec<_>>();
        check_clips(&clips)?;

        let input_depth = property!(clips[0].info().format).bits_per_sample();
        if input_depth < 8 || input_depth > 32 {
            bail!("Input depth can only be between 8 and 32");
        }

        // discard + weights handling
        // this is really horrid, there must be a more elegant way of doing this
        let (discard, weights) = match (discard, preset) {

            // discard exists, and is within bounds + preset unspecified or 0
            (Some(d), Some(0)) | (Some(d), None) if d > 0 && d < ((clips.len() / 2) as i64) => (Some(d as usize), None),

            // discard unspecified or 0 + legal preset
            (None, Some(0)) | (Some(0), Some(0)) | (None, None) | (Some(0), None) => (None, None), // balanced ([1, 1, 1] internally])
            (None, Some(1)) | (Some(0), Some(1)) => (None, Some([1.82, 1.30, 1.00])), // x264/5 defaults    (IP = 1.4, PB = 1.3)
            (None, Some(2)) | (Some(0), Some(2)) => (None, Some([1.21, 1.10, 1.00])), // x264 `--tune grain` (IP = 1.1, PB = 1.1)
            (None, Some(3)) | (Some(0), Some(3)) => (None, Some([1.10, 1.00, 1.00])), // x265 `--tune grain` (IP = 1.1, PB = 1.0)

            // discard OOB
            (Some(_), Some(0)) | (Some(_), None) => bail!("discard cannot be negative, or larger than half the length of input clip list!"),
            // preset OOB
            (None, Some(_)) => bail!("Unknown preset! (Only 0..3 supported, see docs for more information)"),
            // discard & preset both specified. Should ideally be implemented, but not expected for v1.0.0
            (Some(_), Some(_)) => bail!("preset and discard cannot be used simultaneously!"),
        };

        Ok(Some(Box::new(Mean { clips, weights, discard })))
    }
}

export_vapoursynth_plugin! {
    Metadata {
        identifier: PLUGIN_IDENTIFIER,
        namespace: "average",
        name: PLUGIN_NAME,
        read_only: false,
    },
    [
        MeanFunction::new(),
        MedianFunction::new(),
    ]
}
