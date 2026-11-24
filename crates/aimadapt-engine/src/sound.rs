//! The trainer's sound: a pop on every shot, so you know you fired. It's worked out here
//! from a few lines of maths when the trainer starts, so AimAdapt ships no sound files, and
//! it plays on a thread of its own so a shot is heard as soon as it is fired.
//!
//! The pop is a low thump with a short puff of air over it, closer to an air gun than a
//! gunshot. It's the only sound: a tick on a hit and a chime on a kill went on 21 November
//! 2026, as the player wanted just the pop. A player can put a sound of their own in its
//! place: a WAV file at `sounds/shot.wav` in AimAdapt's data folder ([`shot_path`]). It
//! stays on their computer, so nobody else's recording goes into AimAdapt.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Sender};

use aimadapt_sim::Event;
use aimadapt_sim::settings::Sound;
use bevy::prelude::Resource;

/// Most pops playing at once. A new one past this stops the oldest.
const MAX_VOICES: usize = 16;

/// The pop's loudest sample at full volume. A low thump needs more room than a click to
/// sound as loud, and at the default volume this is still under a fifth of full scale.
const PEAK: f32 = 0.7;

/// How loud the pop plays at a volume setting of 0–100, as a share of its full loudness.
/// Ears hear loudness on a curve, so the setting is squared: 50 plays at a quarter.
pub fn gain(volume: u32) -> f32 {
    let v = volume.min(100) as f32 / 100.0;
    v * v
}

/// Whether one frame's events pop: any hitscan shot does, hit or miss. A long frame can
/// fire several shots, and it still pops once. A beam makes no sound: it isn't a click,
/// and it hits 60 times a second.
pub fn pops(events: &[Event], beam: bool, sound: &Sound) -> bool {
    sound.volume > 0 && !beam && events.iter().any(|e| matches!(e, Event::Hit { .. } | Event::Miss))
}

/// The pop's samples at `rate` samples a second: one channel, starting and ending at 0.
pub fn pop(rate: u32) -> Vec<f32> {
    let r = rate.max(1) as f32;
    let len = |seconds: f32| (seconds * r).round() as usize;
    let tau = std::f32::consts::TAU;
    // Rises from 0 over `a` seconds, then holds.
    let attack = |t: f32, a: f32| (t / a).min(1.0);
    // The body: a thump that falls from 220 to 110 Hz in about 10 ms and dies away over
    // 20 ms. Over it, the air: a puff of noise between about 300 Hz and 2.5 kHz, gone in
    // 10 ms. The noise is smoothed twice, so little is left above 2.5 kHz to sound tinny.
    let mut noise = Noise(0x9e37_79b9);
    let smooth = |hz: f32| 1.0 - (-tau * hz / r).exp();
    let (high, low) = (smooth(2500.0), smooth(300.0));
    let (mut a1, mut a2, mut b, mut phase) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
    let mut out: Vec<f32> = (0..len(0.09))
        .map(|i| {
            let t = i as f32 / r;
            let x = noise.next();
            a1 += high * (x - a1);
            a2 += high * (a1 - a2);
            b += low * (a2 - b);
            let puff = 6.0 * (a2 - b) * attack(t, 0.0005) * (-t / 0.010).exp();
            phase += tau * (110.0 + 110.0 * (-t / 0.010).exp()) / r;
            let thump = phase.sin() * attack(t, 0.003) * (-t / 0.020).exp();
            thump + puff
        })
        .collect();
    // The last few milliseconds fade to nothing, so the pop never ends on a click.
    let fade = len(0.004).clamp(1, out.len().max(1));
    let n = out.len();
    for (k, s) in out[n.saturating_sub(fade)..].iter_mut().enumerate() {
        *s *= 1.0 - (k + 1) as f32 / fade as f32;
    }
    scale_to(&mut out, PEAK);
    out
}

/// Where a player's own shot sound goes, in AimAdapt's data folder.
pub fn shot_path(data_dir: &Path) -> PathBuf {
    data_dir.join("sounds").join("shot.wav")
}

/// The player's own shot sound, if there is one: `None` with no file, an error when the
/// file is there but can't be played.
pub fn load_shot(data_dir: &Path) -> Option<Result<Shot, String>> {
    let path = shot_path(data_dir);
    path.is_file().then(|| std::fs::read(&path).map_err(|e| e.to_string()).and_then(|bytes| read_wav(&bytes)))
}

/// Longest shot sound played. Past this it's cut, with a short fade: a long ring piles up
/// when shots come fast.
const SHOT_MAX_SECONDS: f32 = 1.0;

/// A shot sound read from a file: one channel at the file's own rate, its loudest sample
/// at [`PEAK`] like the pop's, so the volume setting means the same for both.
#[derive(Clone, Debug, PartialEq)]
pub struct Shot {
    samples: Vec<f32>,
    rate: u32,
}

impl Shot {
    /// Its length in seconds.
    pub fn seconds(&self) -> f32 {
        self.samples.len() as f32 / self.rate as f32
    }

    /// Its samples at `rate` samples a second, read between the file's own samples.
    fn at_rate(&self, rate: u32) -> Vec<f32> {
        let (from, to) = (self.rate as f64, rate.max(1) as f64);
        let s = &self.samples;
        let len = ((s.len() as f64) * to / from).round().max(1.0) as usize;
        (0..len)
            .map(|i| {
                let pos = i as f64 * from / to;
                let (k, t) = (pos.floor() as usize, (pos - pos.floor()) as f32);
                let a = s.get(k).copied().unwrap_or(0.0);
                let b = s.get(k + 1).copied().unwrap_or(0.0);
                a + (b - a) * t
            })
            .collect()
    }
}

/// Reads a WAV file's sound: PCM at 8, 16, 24 or 32 bits, or 32- or 64-bit floats, in any
/// number of channels (mixed into one). It's cut to [`SHOT_MAX_SECONDS`], fades out over
/// its last millisecond so it never ends on a click, and is scaled to peak at [`PEAK`].
pub fn read_wav(bytes: &[u8]) -> Result<Shot, String> {
    let u16_at = |b: &[u8], i: usize| u16::from_le_bytes([b[i], b[i + 1]]);
    let u32_at = |b: &[u8], i: usize| u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]);
    if bytes.len() < 12 || &bytes[..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err("it isn't a WAV file".into());
    }
    let (mut fmt, mut data) = (None, None);
    let mut at = 12;
    while at + 8 <= bytes.len() {
        let id = &bytes[at..at + 4];
        let size = u32_at(bytes, at + 4) as usize;
        let body = &bytes[at + 8..(at + 8).saturating_add(size).min(bytes.len())];
        match id {
            b"fmt " => fmt = Some(body),
            b"data" => data = Some(body),
            _ => {}
        }
        // Chunks are padded to an even length.
        at = at.saturating_add(8).saturating_add(size).saturating_add(size % 2);
    }
    let fmt = fmt.filter(|f| f.len() >= 16).ok_or("it has no format chunk")?;
    let data = data.ok_or("it has no sound data")?;
    let mut format = u16_at(fmt, 0);
    let channels = usize::from(u16_at(fmt, 2));
    let rate = u32_at(fmt, 4);
    let bits = u16_at(fmt, 14);
    // WAVE_FORMAT_EXTENSIBLE keeps the real format in its subformat's first two bytes.
    if format == 0xFFFE && fmt.len() >= 26 {
        format = u16_at(fmt, 24);
    }
    if channels == 0 || rate == 0 {
        return Err("its format says no channels or no sample rate".into());
    }
    let width = usize::from(bits / 8);
    let sample: fn(&[u8]) -> f32 = match (format, bits) {
        (1, 8) => |b| (f32::from(b[0]) - 128.0) / 128.0,
        (1, 16) => |b| f32::from(i16::from_le_bytes([b[0], b[1]])) / 32_768.0,
        (1, 24) => |b| (i32::from_le_bytes([0, b[0], b[1], b[2]]) >> 8) as f32 / 8_388_608.0,
        (1, 32) => |b| i32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f32 / 2_147_483_648.0,
        (3, 32) => |b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]),
        (3, 64) => |b| f64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]) as f32,
        (1, _) => return Err(format!("its samples are {bits}-bit; 8, 16, 24 or 32 bits play")),
        (3, _) => return Err(format!("its samples are {bits}-bit floats; 32 or 64 bits play")),
        _ => return Err(format!("its samples are compressed (format {format}); save it as plain PCM")),
    };
    let frame = width * channels;
    let most = (SHOT_MAX_SECONDS * rate as f32) as usize;
    let mut samples: Vec<f32> = data
        .chunks_exact(frame)
        .take(most)
        .map(|f| f.chunks_exact(width).map(sample).sum::<f32>() / channels as f32)
        .map(|x| if x.is_finite() { x } else { 0.0 })
        .collect();
    if samples.iter().all(|x| *x == 0.0) {
        return Err("it's silent".into());
    }
    let cut = data.len() / frame > most;
    let fade = ((if cut { 0.01 } else { 0.001 }) * rate as f32).ceil() as usize;
    let n = samples.len();
    let fade = fade.clamp(1, n);
    for (k, s) in samples[n - fade..].iter_mut().enumerate() {
        *s *= 1.0 - (k + 1) as f32 / fade as f32;
    }
    scale_to(&mut samples, PEAK);
    Ok(Shot { samples, rate })
}

/// Scales `s` so its loudest sample is `peak`.
fn scale_to(s: &mut [f32], peak: f32) {
    let loudest = s.iter().fold(0.0f32, |m, x| m.max(x.abs()));
    if loudest > 0.0 {
        let k = peak / loudest;
        s.iter_mut().for_each(|x| *x *= k);
    }
}

/// White noise from a fixed seed, so every pop sounds the same.
struct Noise(u32);

impl Noise {
    fn next(&mut self) -> f32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        (x >> 8) as f32 / (1u32 << 23) as f32 - 1.0
    }
}

/// Plays pops over each other, at the device's rate. The pop is the player's own shot
/// sound when they have one.
pub struct Mixer {
    pop: Vec<f32>,
    voices: Vec<Voice>,
}

struct Voice {
    at: usize,
    gain: f32,
}

impl Mixer {
    pub fn new(rate: u32, shot: Option<&Shot>) -> Mixer {
        let pop = shot.map_or_else(|| pop(rate), |s| s.at_rate(rate));
        Mixer { pop, voices: Vec::with_capacity(MAX_VOICES) }
    }

    /// Starts a pop at `gain` (0–1) of its full loudness.
    pub fn play(&mut self, gain: f32) {
        if gain.is_nan() || gain <= 0.0 {
            return;
        }
        if self.voices.len() == MAX_VOICES {
            self.voices.remove(0);
        }
        self.voices.push(Voice { at: 0, gain: gain.min(1.0) });
    }

    /// Fills `out` with the pops playing, the same in each of `channels` interleaved
    /// channels, and forgets those that have ended.
    pub fn fill(&mut self, out: &mut [f32], channels: usize) {
        let Mixer { pop, voices } = self;
        for frame in out.chunks_mut(channels.max(1)) {
            let mut v = 0.0;
            for voice in voices.iter_mut() {
                if let Some(s) = pop.get(voice.at) {
                    v += s * voice.gain;
                    voice.at += 1;
                }
            }
            frame.fill(v.clamp(-1.0, 1.0));
        }
        voices.retain(|v| v.at < pop.len());
    }
}

/// Where the trainer sends its pops. With no sound device, or in a bot test, there is
/// nowhere to send them and the trainer plays silently.
#[derive(Resource, Default)]
pub struct Speaker {
    to: Option<Sender<f32>>,
}

impl Speaker {
    /// Opens the default sound device on a thread that keeps it open until the trainer
    /// closes, playing `shot` on each shot, or the pop without one. If the device can't be
    /// opened, that is printed and the trainer plays silently.
    pub fn open(shot: Option<Shot>) -> Speaker {
        let (to, from) = mpsc::channel();
        let thread =
            std::thread::Builder::new().name("aimadapt sound".into()).spawn(move || match device::start(from, shot) {
                Ok(_stream) => loop {
                    std::thread::park();
                },
                Err(e) => eprintln!("aimadapt: no sound: {e}"),
            });
        Speaker { to: thread.is_ok().then_some(to) }
    }

    /// Plays a pop at `gain` (0–1). Does nothing without a device.
    pub fn play(&self, gain: f32) {
        if let Some(to) = &self.to {
            // The device thread has gone only when there is no device: play silently.
            let _ = to.send(gain);
        }
    }
}

/// The sound device, through cpal (WASAPI on Windows, ALSA on Linux).
mod device {
    use std::sync::mpsc::Receiver;
    use std::sync::{Arc, Mutex};

    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    use cpal::{BufferSize, FromSample, SampleFormat, SizedSample, Stream, StreamConfig, SupportedBufferSize};

    use super::{Mixer, Shot};

    /// Frames in each block the device asks for, when it lets the trainer choose: about
    /// 5 ms at 48 kHz, so a pop is heard a few milliseconds after the click.
    const BLOCK: u32 = 256;

    /// Each pop's gain, from the game.
    type Pops = Arc<Mutex<Receiver<f32>>>;

    /// Starts the default output device playing whatever `from` sends. The stream plays
    /// until it is dropped.
    pub(super) fn start(from: Receiver<f32>, shot: Option<Shot>) -> Result<Stream, String> {
        let device = cpal::default_host().default_output_device().ok_or("no sound output device")?;
        let supported = device.default_output_config().map_err(|e| e.to_string())?;
        let format = supported.sample_format();
        let usual = supported.config();
        let from: Pops = Arc::new(Mutex::new(from));
        // A short block if the device has a range to pick from, and its own if that fails.
        if let SupportedBufferSize::Range { min, max } = *supported.buffer_size() {
            let short = StreamConfig { buffer_size: BufferSize::Fixed(BLOCK.clamp(min, max)), ..usual };
            if let Ok(stream) = build(&device, format, short, from.clone(), shot.as_ref()) {
                return Ok(stream);
            }
        }
        build(&device, format, usual, from, shot.as_ref())
    }

    fn build(
        device: &cpal::Device,
        format: SampleFormat,
        config: StreamConfig,
        from: Pops,
        shot: Option<&Shot>,
    ) -> Result<Stream, String> {
        match format {
            SampleFormat::F32 => build_as::<f32>(device, config, from, shot),
            SampleFormat::I16 => build_as::<i16>(device, config, from, shot),
            SampleFormat::U16 => build_as::<u16>(device, config, from, shot),
            SampleFormat::I32 => build_as::<i32>(device, config, from, shot),
            other => Err(format!("the device's sample format ({other:?}) isn't one the trainer plays")),
        }
    }

    fn build_as<T: SizedSample + FromSample<f32>>(
        device: &cpal::Device,
        config: StreamConfig,
        from: Pops,
        shot: Option<&Shot>,
    ) -> Result<Stream, String> {
        let channels = usize::from(config.channels);
        let mut mixer = Mixer::new(config.sample_rate, shot);
        let mut mixed: Vec<f32> = Vec::new();
        let stream = device
            .build_output_stream(
                config,
                move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
                    // Only this stream reads the pops, so the lock is always free.
                    if let Ok(pops) = from.try_lock() {
                        while let Ok(gain) = pops.try_recv() {
                            mixer.play(gain);
                        }
                    }
                    mixed.resize(data.len(), 0.0);
                    mixer.fill(&mut mixed, channels);
                    for (out, v) in data.iter_mut().zip(&mixed) {
                        *out = T::from_sample(*v);
                    }
                },
                |e| eprintln!("aimadapt: sound: {e}"),
                None,
            )
            .map_err(|e| e.to_string())?;
        stream.play().map_err(|e| e.to_string())?;
        Ok(stream)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: u32 = 48_000;

    fn ms(samples: usize) -> f32 {
        samples as f32 * 1000.0 / RATE as f32
    }

    fn energy(s: &[f32]) -> f32 {
        s.iter().map(|x| x * x).sum()
    }

    /// Share of `s`'s energy below `hz`, from its spectrum (zero-padded to 8192 samples at `RATE`).
    fn share_below(s: &[f32], hz: f64) -> f64 {
        let n = 8192usize.max(s.len().next_power_of_two());
        let total: f64 = s.iter().map(|&x| f64::from(x) * f64::from(x)).sum();
        let top = (hz * n as f64 / f64::from(RATE)).ceil() as usize;
        let mut below = 0.0;
        for k in 0..top.min(n / 2) {
            let step = -std::f64::consts::TAU * k as f64 / n as f64;
            let (mut re, mut im) = (0.0, 0.0);
            for (i, &x) in s.iter().enumerate() {
                let (sin, cos) = (step * i as f64).sin_cos();
                re += f64::from(x) * cos;
                im += f64::from(x) * sin;
            }
            // Each bin past 0 stands for itself and its mirror above half the rate.
            below += if k == 0 { 1.0 } else { 2.0 } * (re * re + im * im);
        }
        below / (n as f64 * total)
    }

    /// The pop has to be short and soft: a pop that tells you a shot went off, not a gunshot that wears on you over
    /// hundreds of shots (asked for Nov 19, 2026).
    #[test]
    fn the_pop_is_short_and_soft() {
        let pop = pop(RATE);
        assert!(ms(pop.len()) <= 100.0, "{} ms", ms(pop.len()));
        let loudest = pop.iter().fold(0.0f32, |m, s| m.max(s.abs()));
        assert!((loudest - PEAK).abs() < 1e-6 && PEAK <= 0.7, "{loudest}");
        // Nearly all of its energy is in the first 50 ms: a pop, not a hiss or a boom.
        let early = energy(&pop[..(RATE as usize * 50 / 1000)]);
        assert!(early > 0.9 * energy(&pop), "{early} of {}", energy(&pop));
    }

    /// Asked for Nov 23, 2026: the pop was "tingy", with no bass or oomph. Nearly half of its energy was above 5 kHz
    /// and 2% below 300 Hz. Now a low thump carries much of it and little is left up high. (Before that, on Nov 19,
    /// the first version had gone too far the other way: a 150 Hz knock with 29% of its energy above 500 Hz, which
    /// small speakers barely play, so the mid-range puff stays.)
    #[test]
    fn the_pop_has_bass_and_isnt_tinny() {
        let pop = pop(RATE);
        let bass = share_below(&pop, 300.0);
        let tinny = 1.0 - share_below(&pop, 5000.0);
        assert!(bass >= 0.4, "{:.0}% below 300 Hz", bass * 100.0);
        assert!(tinny <= 0.1, "{:.0}% above 5 kHz", tinny * 100.0);
        let mid = share_below(&pop, 5000.0) - share_below(&pop, 500.0);
        assert!(mid >= 0.2, "{:.0}% between 500 Hz and 5 kHz, where every speaker plays", mid * 100.0);
    }

    /// Bug (21 Nov 2026): the pop couldn't be heard. It was over in a few milliseconds, so over the 100 ms or so the
    /// ear takes in at once it came to -34 dBFS at full volume, 13 dB under the kill chime that played with it.
    /// Bass adds energy the ear hears less of, so the part above 300 Hz has to be loud enough on its own too.
    #[test]
    fn the_pop_is_loud_enough_to_hear() {
        let pop = pop(RATE);
        let db = |share: f64| 10.0 * (share * f64::from(energy(&pop)) / (f64::from(RATE) * 0.1)).log10();
        assert!(db(1.0) >= -24.0, "{:.1} dBFS over 100 ms", db(1.0));
        let above = 1.0 - share_below(&pop, 300.0);
        assert!(db(above) >= -27.0, "{:.1} dBFS over 100 ms above 300 Hz", db(above));
    }

    #[test]
    fn the_pop_starts_and_ends_at_silence_and_is_the_same_each_time() {
        for rate in [44_100, 48_000, 96_000] {
            let s = pop(rate);
            assert!(!s.is_empty());
            assert_eq!(s[0], 0.0, "starts with a jump");
            assert_eq!(*s.last().unwrap(), 0.0, "ends with a jump");
            assert!(s.iter().all(|x| x.is_finite() && x.abs() <= PEAK + 1e-6));
            assert_eq!(s, pop(rate), "changes from run to run");
        }
    }

    fn sound(volume: u32) -> Sound {
        Sound { volume }
    }

    /// Hits and kills used to add a tick and a chime; since 21 Nov 2026 every shot only pops.
    #[test]
    fn every_shot_pops_hit_or_miss_and_nothing_else_does() {
        let on = sound(50);
        assert!(pops(&[Event::Miss], false, &on));
        assert!(pops(&[Event::Hit { id: 1 }], false, &on));
        assert!(pops(&[Event::Hit { id: 1 }, Event::Killed { id: 1 }], false, &on));
        assert!(!pops(&[Event::Spawned { id: 2 }, Event::TimedOut { id: 1 }, Event::Finished], false, &on));
        assert!(!pops(&[], false, &on));
    }

    #[test]
    fn a_beam_makes_no_sound() {
        let on = sound(50);
        assert!(!pops(&[Event::Hit { id: 1 }, Event::Hit { id: 1 }], true, &on));
        assert!(!pops(&[Event::Hit { id: 1 }, Event::Killed { id: 1 }], true, &on));
    }

    #[test]
    fn volume_0_turns_the_pop_off() {
        assert!(!pops(&[Event::Miss], false, &sound(0)));
        assert!(pops(&[Event::Miss], false, &sound(5)));
    }

    #[test]
    fn volume_is_heard_on_a_curve() {
        assert_eq!(gain(0), 0.0);
        assert_eq!(gain(50), 0.25);
        assert_eq!(gain(100), 1.0);
        assert_eq!(gain(250), 1.0, "past 100 is 100");
    }

    #[test]
    fn the_mixer_adds_pops_up_in_every_channel_and_forgets_finished_ones() {
        let mut m = Mixer::new(RATE, None);
        let pop = pop(RATE);
        // Quiet enough that the two together stay under full scale.
        m.play(0.5);
        m.play(0.25);
        let mut out = vec![9.0; 2 * pop.len()];
        m.fill(&mut out, 2);
        for (i, s) in pop.iter().enumerate() {
            assert!((out[2 * i] - 0.75 * s).abs() < 1e-6, "sample {i}");
            assert_eq!(out[2 * i], out[2 * i + 1], "both channels");
        }
        assert!(m.voices.is_empty(), "both ended");
        let mut after = vec![9.0; 8];
        m.fill(&mut after, 2);
        assert_eq!(after, [0.0; 8], "silence once nothing plays");
    }

    #[test]
    fn the_mixer_never_goes_past_full_scale_or_plays_too_many_at_once() {
        let mut m = Mixer::new(RATE, None);
        for _ in 0..40 {
            m.play(1.0);
        }
        assert_eq!(m.voices.len(), MAX_VOICES);
        m.play(0.0);
        m.play(f32::NAN);
        assert_eq!(m.voices.len(), MAX_VOICES, "silent pops aren't played");
        let mut out = vec![0.0; 4096];
        m.fill(&mut out, 1);
        assert!(out.iter().all(|s| s.abs() <= 1.0));
    }

    /// A WAV file's bytes: `format` 1 (PCM), 3 (floats) or 0xFFFE (extensible, holding
    /// `sub`), with `frames` of one value per channel.
    fn wav(format: u16, sub: u16, bits: u16, rate: u32, frames: &[Vec<f64>]) -> Vec<u8> {
        let channels = frames.first().map_or(1, Vec::len) as u16;
        let mut data = Vec::new();
        for x in frames.iter().flatten() {
            match (if format == 0xFFFE { sub } else { format }, bits) {
                (1, 8) => data.push(((x * 127.0).round() + 128.0) as u8),
                (1, 16) => data.extend_from_slice(&((x * 32_767.0).round() as i16).to_le_bytes()),
                (1, 24) => data.extend_from_slice(&((x * 8_388_607.0).round() as i32).to_le_bytes()[..3]),
                (1, 32) => data.extend_from_slice(&((x * 2_147_483_647.0).round() as i32).to_le_bytes()),
                (3, 32) => data.extend_from_slice(&(*x as f32).to_le_bytes()),
                (3, 64) => data.extend_from_slice(&x.to_le_bytes()),
                _ => data.extend_from_slice(&vec![0; usize::from(bits / 8)]),
            }
        }
        let align = channels * bits / 8;
        let mut fmt = Vec::new();
        fmt.extend_from_slice(&format.to_le_bytes());
        fmt.extend_from_slice(&channels.to_le_bytes());
        fmt.extend_from_slice(&rate.to_le_bytes());
        fmt.extend_from_slice(&(rate * u32::from(align)).to_le_bytes());
        fmt.extend_from_slice(&align.to_le_bytes());
        fmt.extend_from_slice(&bits.to_le_bytes());
        if format == 0xFFFE {
            fmt.extend_from_slice(&22u16.to_le_bytes());
            fmt.extend_from_slice(&bits.to_le_bytes());
            fmt.extend_from_slice(&0u32.to_le_bytes());
            fmt.extend_from_slice(&sub.to_le_bytes());
            fmt.extend_from_slice(&[0; 14]);
        }
        let mut out = b"RIFF\0\0\0\0WAVE".to_vec();
        // A chunk the reader skips, with an odd length and its pad byte, as Pro Tools writes.
        for (id, body) in [(b"bext", vec![7u8; 3]), (b"fmt ", fmt), (b"data", data)] {
            out.extend_from_slice(id);
            out.extend_from_slice(&(body.len() as u32).to_le_bytes());
            out.extend_from_slice(&body);
            if body.len() % 2 == 1 {
                out.push(0);
            }
        }
        let size = (out.len() - 8) as u32;
        out[4..8].copy_from_slice(&size.to_le_bytes());
        out
    }

    /// A shot-like sound: a 200 Hz tone dying away over `seconds`, in one channel.
    fn bang(rate: u32, seconds: f64) -> Vec<Vec<f64>> {
        let n = (seconds * f64::from(rate)) as usize;
        (0..n)
            .map(|i| {
                let t = i as f64 / f64::from(rate);
                vec![0.8 * (std::f64::consts::TAU * 200.0 * t).sin() * (-t / 0.03).exp()]
            })
            .collect()
    }

    #[test]
    fn your_own_wav_file_plays_in_place_of_the_pop() {
        let shot = read_wav(&wav(1, 0, 16, RATE, &bang(RATE, 0.15))).unwrap();
        assert_eq!(shot.rate, RATE);
        assert_eq!(shot.samples.len(), (0.15 * RATE as f32) as usize);
        let peak = shot.samples.iter().fold(0.0f32, |m, x| m.max(x.abs()));
        assert!((peak - PEAK).abs() < 1e-6, "peaks where the pop does, so the volume means the same: {peak}");
        assert_eq!(*shot.samples.last().unwrap(), 0.0, "ends at silence, never on a click");

        let mut m = Mixer::new(RATE, Some(&shot));
        m.play(0.5);
        let mut out = vec![0.0; shot.samples.len() + 64];
        m.fill(&mut out, 1);
        for (o, s) in out.iter().zip(&shot.samples) {
            assert!((o - 0.5 * s).abs() < 1e-6);
        }
        assert!(out[shot.samples.len()..].iter().all(|o| *o == 0.0));
        assert!(m.voices.is_empty());
    }

    #[test]
    fn every_common_wav_format_reads_as_the_same_sound() {
        let frames = bang(RATE, 0.05);
        let want = read_wav(&wav(3, 0, 64, RATE, &frames)).unwrap();
        let stereo: Vec<Vec<f64>> = frames.iter().map(|f| vec![f[0], f[0]]).collect();
        for (what, file, tolerance) in [
            ("8-bit", wav(1, 0, 8, RATE, &frames), 0.02),
            ("16-bit", wav(1, 0, 16, RATE, &frames), 1e-3),
            ("24-bit", wav(1, 0, 24, RATE, &frames), 1e-4),
            ("32-bit", wav(1, 0, 32, RATE, &frames), 1e-4),
            ("32-bit floats", wav(3, 0, 32, RATE, &frames), 1e-4),
            ("24-bit stereo", wav(1, 0, 24, RATE, &stereo), 1e-4),
            ("24-bit extensible", wav(0xFFFE, 1, 24, RATE, &frames), 1e-4),
            ("float extensible", wav(0xFFFE, 3, 32, RATE, &frames), 1e-4),
        ] {
            let got = read_wav(&file).unwrap_or_else(|e| panic!("{what}: {e}"));
            assert_eq!(got.samples.len(), want.samples.len(), "{what}");
            let worst = got.samples.iter().zip(&want.samples).map(|(a, b)| (a - b).abs()).fold(0.0, f32::max);
            assert!(worst < tolerance, "{what}: off by {worst}");
        }
    }

    #[test]
    fn stereo_is_mixed_into_one_channel() {
        let left: Vec<Vec<f64>> = bang(RATE, 0.05).into_iter().map(|f| vec![f[0], 0.0]).collect();
        let mono: Vec<Vec<f64>> = bang(RATE, 0.05);
        let (a, b) = (read_wav(&wav(3, 0, 32, RATE, &left)).unwrap(), read_wav(&wav(3, 0, 32, RATE, &mono)).unwrap());
        // Scaled to the same peak, a sound in one channel only is the same sound.
        assert!(a.samples.iter().zip(&b.samples).all(|(x, y)| (x - y).abs() < 1e-5));
    }

    #[test]
    fn a_shot_plays_at_the_devices_rate_at_its_own_pitch() {
        let shot = read_wav(&wav(1, 0, 24, 48_000, &bang(48_000, 0.2))).unwrap();
        // Upward crossings of zero over the first 0.1 s, while the tone is strong: 20 cycles at 200 Hz.
        let cycles = |s: &[f32], rate: u32| {
            s[..(0.1 * rate as f32) as usize].windows(2).filter(|w| w[0] < 0.0 && w[1] >= 0.0).count()
        };
        assert!(cycles(&shot.samples, 48_000).abs_diff(20) <= 1, "{}", cycles(&shot.samples, 48_000));
        for rate in [44_100, 48_000, 96_000] {
            let s = shot.at_rate(rate);
            assert_eq!(s.len(), (0.2 * rate as f32).round() as usize, "{rate}");
            assert!(cycles(&s, rate).abs_diff(20) <= 1, "{rate}: {} cycles in 0.1 s", cycles(&s, rate));
        }
        assert_eq!(shot.at_rate(48_000), shot.samples, "the file's own rate plays it as it is");
    }

    #[test]
    fn a_long_shot_is_cut_to_a_second_with_a_fade() {
        let ring: Vec<Vec<f64>> = (0..3 * RATE).map(|i| vec![(i as f64 * 0.05).sin() * 0.5]).collect();
        let shot = read_wav(&wav(1, 0, 16, RATE, &ring)).unwrap();
        assert_eq!(shot.seconds(), SHOT_MAX_SECONDS);
        let end = &shot.samples[shot.samples.len() - 480..];
        assert_eq!(*end.last().unwrap(), 0.0);
        let early = shot.samples[..RATE as usize / 2].iter().fold(0.0f32, |m, x| m.max(x.abs()));
        let last_ms = end[end.len() - 48..].iter().fold(0.0f32, |m, x| m.max(x.abs()));
        assert!(last_ms < early * 0.15, "the cut fades out: {last_ms} against {early}");
    }

    #[test]
    fn a_file_that_cant_play_says_why() {
        let one = bang(RATE, 0.01);
        let mut no_data = wav(1, 0, 16, RATE, &one);
        let at = no_data.windows(4).position(|w| w == b"data").unwrap();
        no_data[at..at + 4].copy_from_slice(b"junk");
        let mut truncated = wav(1, 0, 16, RATE, &one);
        truncated.truncate(truncated.len() - 101);
        for (file, says) in [
            (b"ID3 an mp3 file".to_vec(), "isn't a WAV file"),
            (Vec::new(), "isn't a WAV file"),
            (no_data, "no sound data"),
            (wav(2, 0, 4, RATE, &one), "compressed"),
            (wav(1, 0, 12, RATE, &one), "12-bit"),
            (wav(3, 0, 16, RATE, &one), "16-bit floats"),
            (wav(1, 0, 16, RATE, &vec![vec![0.0]; 100]), "silent"),
            (wav(1, 0, 16, 0, &one), "no sample rate"),
        ] {
            let e = read_wav(&file).expect_err(says);
            assert!(e.contains(says), "{e}");
        }
        // A file cut short plays what it has.
        assert!(read_wav(&truncated).is_ok());
    }

    #[test]
    fn your_shot_sound_is_found_in_the_data_folder() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(shot_path(dir.path()), dir.path().join("sounds").join("shot.wav"));
        assert!(load_shot(dir.path()).is_none(), "no file plays the pop");
        std::fs::create_dir_all(dir.path().join("sounds")).unwrap();
        std::fs::write(shot_path(dir.path()), wav(1, 0, 16, RATE, &bang(RATE, 0.1))).unwrap();
        assert!(matches!(load_shot(dir.path()), Some(Ok(_))));
        std::fs::write(shot_path(dir.path()), b"not a sound").unwrap();
        assert_eq!(load_shot(dir.path()), Some(Err("it isn't a WAV file".into())));
    }

    /// Plays a few pops on this computer's speakers at the default volume, to hear them. Needs a sound device, so
    /// it's run by hand: `cargo test -p aimadapt-engine -- --ignored hear_the_pop`.
    #[test]
    #[ignore = "plays sound; run by hand"]
    fn hear_the_pop() {
        let speaker = Speaker::open(None);
        let g = gain(Sound::default().volume);
        for wait in [500, 500, 150, 150, 150, 500] {
            speaker.play(g);
            std::thread::sleep(std::time::Duration::from_millis(wait));
        }
    }
}
