# vs-average

Copyright © 2020 EoE & Nephren

A VapourSynth plugin for averaging clips together.<br />
_Early release, not all features are tested. Please report any issues either here, or in EoE's dms (See bottom)_

## Description

vs-average is a VapourSynth plugin for averaging multiple clips together. The general idea is to be able to take multiple sources of the same video, and average them together, to negate effects of lossy compression. The plugin can also be effective for temporal blurring.

## Supported Formats

Sample Type & Bits Per Sample: 
 - Mean: 8, 10, 12, 16 and 32 bit integer, 16 and 32 bit float.
 - Median: All integer formats (8..32), only 32 bit float. (f16 will be added in a later commit)

Color Family: Gray, RGB, YUV or YCoCg.

Sub-Sampling: any.

Note that the input format must be the same for all inputted clips.

## Usage

### Mean

Mean will set the output pixel to the average (or mean) of the input pixels from each clip.

```python
average.Mean(clip[] clips[, output_depth=clips[0].format.bits_per_sample])
```

- clips:<br />
    List of clips to be processed. Must be the of the same format, length, fps, ect.

- output_depth:<br />
    Bitdepth of the output. Will default to the same as `clips[0]`.<br />
    Only 8, 16 or 32 bit is supported.<br />
    Since all calculations are done interally as u64's (or f32's), it's far more efficient to input your sources as 8 bit, and return as 16 bit with the increased precision. In the case that you want to directly output the clip returned by `Mean`, I'd suggest you return a 16 bit clip, and dither down using `resize.Point` or similar, even for 16 -> 8 bit, due to an internal rounding error. Significant improvements can be observed over returning a higher bitdepth clip, and dithering down, than a lower bitdepth clip. For this same reason, returning a 10 or 12 bit clip is not supported (And also because I'm lazy). For more information, see the comments in `mean.rs`.

### Median

Median will set the output pixel to the Median (middle value of the sorted data) of the input pixels from each clip.

```python
average.Mean(clip[] clips])
```

- clips:<br />
    List of clips to be processed. Must be the of the same format, length, fps, ect.

Note that since `Median` does not define an `output_depth` parameter, any input of an even number of clips (where the average of the middle two valeus is taken to be the mean) will likely induce another (though smaller) rounding error. I'll fix this at some point too.

## Examples

- Take the Mean of 3, 8 bit input clips, and output as 16 bit.

```python
clips = [clip_a, clip_b, clip_c]

mean = core.average.Mean(clips, output_depth=16)
```

- Take the Median of 3 clips.

```python
clips = [clip_a, clip_b, clip_c]

mean = core.average.Median(clips)
```

- Simple temporal blur

```python
# add an extra frame to the start of our clip so it's one frame behind
slow_clip = clip[0] + clip
# drop the first frame of our clip so it's one frame ahead
fast_clip = clip[1:]

# average our slow, original, and fast clips together to get a temporal blur.
temporal_blur = core.average.Mean([slow_clip, clip, fast_clip], output_depth=16)
```

## Compilation

```
cargo build --release
```

## FAQ

 - _Q: Why did you implement support for 16 bit floats?_ <br />
   A: Mainly because if the end user wanted to work in floats, and had 8 bit sources, f16s are far smaller in memory usage, and are implemented on all not-under-a-rock CPUs since the cavemen were around circa 2009. Further processing can be done in 32 bit. For more information, see `mean.rs` or contact me on Discord (See below).

- _Q: How fast is `vs-average`?_ <br />
   A: Pretty fast, `vs-average` implements multithreading in the default build, allowing for a huge throughput. In fact, the main bottlenecks I've noticed is drive latency (Observed up to ~100MiB/s sustained random reads), and decode speed (decoding 11 AVC bitstreams isn't easy). For fastest speeds with multiple clips, I'd suggest using something akin to [dgdecodenv](http://rationalqm.us/dgdecnv/dgdecnv.html) for decoding acceleration via CUDA, and all source files on one (or multiple) SSDs.

   Note that all my tests have been performed using `lsmas.LWLibavSource` to index and decode souce files, on an R9 3900x with 32GiB of ram.

 - _Q: Do you plan to write more useless plugins?_ <br />
   A: Yup, rust is pretty cool, [vapoursynth-rs](https://github.com/YaLTeR/vapoursynth-rs) is brilliant, and I'm full of dumb ideas :^)

## Contributors

 - EoE
    - Discord: `End of Eternity#6292`
 - Nephren
    - Discord: `Rin-go#8647`
