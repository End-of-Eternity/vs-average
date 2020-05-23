// Copyright (c) EoE & Nephren 2020. All rights reserved.

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
        multipliers: Option<ValueIter<'_, 'core, f64>>
    ) -> Result<Option<Box<dyn Filter<'core> + 'core>>, Error> {
        let clips = clips.collect::<Vec<_>>();
        check_clips(&clips)?;

        let input_depth = property!(clips[0].info().format).bits_per_sample();
        if input_depth != 8 && input_depth != 10 && input_depth != 12 && input_depth != 16 && input_depth != 32 {
            bail!("Input depth can only be 8, 10, 12, 16 or 32")
        }

        let multipliers = match multipliers {
            Some(multipliers) => match &multipliers.collect::<Box<_>>()[..] {
                &[i, p, b] => [i, p, b],
                _ => bail!("Three parameters must be given for multipliers, in the form multipliers=[I, P, B]"),
            },
            _ => match preset {
                Some(1) => [1.82, 1.3, 1.0], // x264 / 5
                _ => [1.0, 1.0, 1.0], // balanced
            },
        };

        Ok(Some(Box::new(Mean { clips, multipliers })))
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
