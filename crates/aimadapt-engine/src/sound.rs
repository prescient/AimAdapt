//! The trainer's sound: a pop on every shot, so you know you fired. It's worked out here
//! from a few lines of maths when the trainer starts, so there are no sound files, and it
//! plays on a thread of its own so a shot is heard as soon as it is fired.
//!
//! The pop is a short puff of filtered noise with a dull knock under it, closer to an air
//! gun than a gunshot. It's the only sound: a tick on a hit and a chime on a kill went on
//! 21 November 2026, as the player wanted just the pop.

use std::sync::mpsc::{self, Sender};

use aimadapt_sim::Event;
use aimadapt_sim::settings::Sound;
use bevy::prelude::Resource;

/// Most pops playing at once. A new one past this stops the oldest.
const MAX_VOICES: usize = 16;

/// The pop's loudest sample at full volume: half of full scale, so it's soft at its
/// sharpest moment and the volume setting has room to turn it up.
const PEAK: f32 = 0.5;

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
    // A puff of noise between about 700 Hz and 4.5 kHz (the difference of two smoothed
    // copies), mostly gone in 10 ms, over a knock that falls from about 450 to 240 Hz.
    let mut noise = Noise(0x9e37_79b9);
    let smooth = |hz: f32| 1.0 - (-tau * hz / r).exp();
    let (high, low) = (smooth(4500.0), smooth(700.0));
    let (mut a, mut b, mut phase) = (0.0f32, 0.0f32, 0.0f32);
    let mut out: Vec<f32> = (0..len(0.06))
        .map(|i| {
            let t = i as f32 / r;
            let x = noise.next();
            a += high * (x - a);
            b += low * (x - b);
            let puff = 4.0 * (a - b) * attack(t, 0.0004) * (-t / 0.008).exp();
            phase += tau * (240.0 + 210.0 * (-t / 0.006).exp()) / r;
            let knock = 0.3 * phase.sin() * attack(t, 0.001) * (-t / 0.010).exp();
            puff + knock
        })
        .collect();
    // The last few milliseconds fade to nothing, so the pop never ends on a click.
    let fade = len(0.004).clamp(1, out.len().max(1));
    let n = out.len();
    for (k, s) in out[n.saturating_sub(fade)..].iter_mut().enumerate() {
        *s *= 1.0 - (k + 1) as f32 / fade as f32;
    }
    // Rounding off the puff's sharpest peaks (a soft clip) makes it fuller for the same
    // peak, so it can be heard without being sharp.
    scale_to(&mut out, 1.0);
    out.iter_mut().for_each(|s| *s = (2.0 * *s).tanh());
    scale_to(&mut out, PEAK);
    out
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

/// Plays pops over each other, at the device's rate.
pub struct Mixer {
    pop: Vec<f32>,
    voices: Vec<Voice>,
}

struct Voice {
    at: usize,
    gain: f32,
}

impl Mixer {
    pub fn new(rate: u32) -> Mixer {
        Mixer { pop: pop(rate), voices: Vec::with_capacity(MAX_VOICES) }
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
    /// closes. If it can't be opened, that is printed and the trainer plays silently.
    pub fn open() -> Speaker {
        let (to, from) = mpsc::channel();
        let thread =
            std::thread::Builder::new().name("aimadapt sound".into()).spawn(move || match device::start(from) {
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

    use super::Mixer;

    /// Frames in each block the device asks for, when it lets the trainer choose: about
    /// 5 ms at 48 kHz, so a pop is heard a few milliseconds after the click.
    const BLOCK: u32 = 256;

    /// Each pop's gain, from the game.
    type Pops = Arc<Mutex<Receiver<f32>>>;

    /// Starts the default output device playing whatever `from` sends. The stream plays
    /// until it is dropped.
    pub(super) fn start(from: Receiver<f32>) -> Result<Stream, String> {
        let device = cpal::default_host().default_output_device().ok_or("no sound output device")?;
        let supported = device.default_output_config().map_err(|e| e.to_string())?;
        let format = supported.sample_format();
        let usual = supported.config();
        let from: Pops = Arc::new(Mutex::new(from));
        // A short block if the device has a range to pick from, and its own if that fails.
        if let SupportedBufferSize::Range { min, max } = *supported.buffer_size() {
            let short = StreamConfig { buffer_size: BufferSize::Fixed(BLOCK.clamp(min, max)), ..usual };
            if let Ok(stream) = build(&device, format, short, from.clone()) {
                return Ok(stream);
            }
        }
        build(&device, format, usual, from)
    }

    fn build(device: &cpal::Device, format: SampleFormat, config: StreamConfig, from: Pops) -> Result<Stream, String> {
        match format {
            SampleFormat::F32 => build_as::<f32>(device, config, from),
            SampleFormat::I16 => build_as::<i16>(device, config, from),
            SampleFormat::U16 => build_as::<u16>(device, config, from),
            SampleFormat::I32 => build_as::<i32>(device, config, from),
            other => Err(format!("the device's sample format ({other:?}) isn't one the trainer plays")),
        }
    }

    fn build_as<T: SizedSample + FromSample<f32>>(
        device: &cpal::Device,
        config: StreamConfig,
        from: Pops,
    ) -> Result<Stream, String> {
        let channels = usize::from(config.channels);
        let mut mixer = Mixer::new(config.sample_rate);
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

    /// The pop has to be short and soft: a click that tells you a shot went off, not a gunshot that wears on you
    /// over hundreds of shots (asked for Nov 19, 2026).
    #[test]
    fn the_pop_is_short_and_soft() {
        let pop = pop(RATE);
        assert!(ms(pop.len()) <= 80.0, "{} ms", ms(pop.len()));
        let loudest = pop.iter().fold(0.0f32, |m, s| m.max(s.abs()));
        assert!((loudest - PEAK).abs() < 1e-6 && PEAK <= 0.5, "{loudest}");
        // Nearly all of its energy is in the first 25 ms: a pop, not a hiss.
        let early = energy(&pop[..(RATE as usize * 25 / 1000)]);
        assert!(early > 0.9 * energy(&pop), "{early} of {}", energy(&pop));
        // And most of it is above 500 Hz, where small speakers and headsets play it: a
        // snap, not a thump. The first version was mostly a 150 Hz knock (29% of its
        // energy was up there).
        let k = 1.0 - (-std::f32::consts::TAU * 500.0 / RATE as f32).exp();
        let mut low = 0.0;
        let high: Vec<f32> = pop
            .iter()
            .map(|x| {
                low += k * (x - low);
                x - low
            })
            .collect();
        let share = energy(&high) / energy(&pop);
        assert!(share > 0.7, "{share}");
    }

    /// Bug (21 Nov 2026): the pop couldn't be heard. It was over in a few milliseconds, so over the 100 ms or so the
    /// ear takes in at once it came to -34 dBFS at full volume, 13 dB under the kill chime that played with it.
    #[test]
    fn the_pop_is_loud_enough_to_hear() {
        let pop = pop(RATE);
        let db = 10.0 * (energy(&pop) / (RATE as f32 * 0.1)).log10();
        assert!(db >= -24.0, "{db:.1} dBFS over 100 ms");
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
        let mut m = Mixer::new(RATE);
        let pop = pop(RATE);
        m.play(1.0);
        m.play(0.5);
        let mut out = vec![9.0; 2 * pop.len()];
        m.fill(&mut out, 2);
        for (i, s) in pop.iter().enumerate() {
            assert!((out[2 * i] - 1.5 * s).abs() < 1e-6, "sample {i}");
            assert_eq!(out[2 * i], out[2 * i + 1], "both channels");
        }
        assert!(m.voices.is_empty(), "both ended");
        let mut after = vec![9.0; 8];
        m.fill(&mut after, 2);
        assert_eq!(after, [0.0; 8], "silence once nothing plays");
    }

    #[test]
    fn the_mixer_never_goes_past_full_scale_or_plays_too_many_at_once() {
        let mut m = Mixer::new(RATE);
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

    /// Plays a few pops on this computer's speakers at the default volume, to hear them. Needs a sound device, so
    /// it's run by hand: `cargo test -p aimadapt-engine -- --ignored hear_the_pop`.
    #[test]
    #[ignore = "plays sound; run by hand"]
    fn hear_the_pop() {
        let speaker = Speaker::open();
        let g = gain(Sound::default().volume);
        for wait in [500, 500, 150, 150, 150, 500] {
            speaker.play(g);
            std::thread::sleep(std::time::Duration::from_millis(wait));
        }
    }
}
