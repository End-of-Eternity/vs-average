# vs-average

Copyright © 2020-2021 EoE & Nephren

A VapourSynth plugin for averaging clips together.<br />
_Early release, not all features are tested. Please report any issues either here, or in EoE's dms (See bottom)_

## Description

vs-average is a VapourSynth plugin for averaging multiple clips together. The general idea is to be able to take multiple sources of the same video, and average them together, to negate effects of lossy compression. The plugin can also be effective for temporal blurring.

## Supported Formats

Sample Type & Bits Per Sample: 
 - Mean: All supported by VapourSynth
 - Median: All integer formats (8..32), only 32 bit float. (f16 will be added in a later commit)

Color Family: Gray, RGB, YUV or YCoCg.

Sub-Sampling: Any.

Note that the input format must be the same for all inputted clips.

## Usage

Whilst both `average.Mean` and `average.Median` will accept a wide range of input formats, it is suggested to input a higher precision than your source to negate rounding errors. For example, given some 8 bit source files, you should increase their precision using something akin to the following:

```python
# some 8 bit source clips
clips = [clip_a, clip_b, clip_c]
# increase precision to 16 bits
clips = [core.fmtc.bitdepth(clip, bits=16) for clip in clips]
# get back a higher precision 16 bit clip
mean = core.average.Mean(clips)
```

### Mean

Mean will set the output pixel to the average (or mean) of the input pixels from each clip. Returns a clip of the same format as the inputs.

```python
average.Mean(clip[] clips[, int preset])
```

- clips:<br />
    List of clips to be processed. Must be the of the same format, length, fps, ect.
  
- preset:<br />
    Integer based preset value for per frame type weightings. See below for how this works. Any other inputs than the ones stated below (or none) will be interpreted as `multipliers=[0, 0, 0]` (no weighting).
    
    1. Reverse (default) x264/5 based IP/PB qp offset ratios. (`--ipratio 1.4 --pbratio 1.3`). Works for other encoders/ratios as well (though may be less effective)<br />
    2. Reverse x264 `--tune grain` offset ratios (`--ipratio 1.1 --pbratio 1.1`)
    3. Reverse x265 `--tune grain` offset ratios (`--ipratio 1.1 --pbratio 1.0`)


### Median

Median will set the output pixel to the Median (middle value of the sorted data) of the input pixels from each clip. Returns a clip of the same format as the inputs.

```python
average.Median(clip[] clips])
```

- clips:<br />
    List of clips to be processed. Must be the of the same format, length, fps, ect.

## Examples

- Take the Mean of 3 input clips, encoded using the x264 `--tune grain` preset

```python
clips = [clip_a, clip_b, clip_c]

mean = core.average.Mean(clips, preset=2)
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
fast_clip = clip[1:] + clip[-1]

# average our slow, original, and fast clips together to get a temporal blur.
temporal_blur = core.average.Mean([slow_clip, clip, fast_clip])
```

## Compilation

```
cargo build --release
```

## FAQ

 - _Q: Why did you implement support for 16 bit floats?_ <br />
   A: Why not?

- _Q: How fast is `vs-average`?_ <br />
   A: For the current release (v0.3.0), I have had no noticable speed losses using `average.Mean` vs doing nothing at all to some clips in both generated clips by `std.BlankClip`, and decoded clips from `lsmas.LWLibavSource`. It should be noted however that if you did intend to use a significant number of file based source, that you should use an SSD which can handle heavy random reads, and either a CPU which can handle decoding multiple streams, or a GPU based decoder such as [dgdecodenv](http://rationalqm.us/dgdecnv/dgdecnv.html).

   As of now, `average.Median` is still fairly badly implemented, and it's nowhere near as fast as `average.Mean`, however fixing it up is next on the agenda.

 - _Q: Do you plan to write more useless plugins?_ <br />
   A: Yup, rust is pretty cool, [vapoursynth-rs](https://github.com/YaLTeR/vapoursynth-rs) is brilliant, and I'm full of dumb ideas :^)

## Contributors

 - EoE
    - Discord: `End of Eternity#6292`
 - Nephren
    - Discord: `Rin-go#8647`
 - Kageru
    - Discord: `kageru#1337`
