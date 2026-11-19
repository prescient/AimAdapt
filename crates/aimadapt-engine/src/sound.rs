//! The trainer's sounds: a soft pop on every shot, a tick when a shot hits and a chime
//! when it kills. Each is worked out here from a few lines of maths when the trainer
//! starts, so there are no sound files, and they play on a thread of their own so a
//! shot is heard as soon as it is fired.
//!
//! The pop only says "you fired": a short puff of filtered noise with a dull knock
//! under it, closer to an air gun than a gunshot, and quiet enough to hear hundreds of
//! times a session.

use std::sync::mpsc::{self, Sender};

use aimadapt_sim::Event;
use aimadapt_sim::settings::Sound;
use bevy::prelude::Resource;

/// One of the trainer's sounds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cue {
    Shot,
    Hit,
    Kill,
}

/// Every cue, in the order of their numbers (`cue as usize`).
const CUES: [Cue; 3] = [Cue::Shot, Cue::Hit, Cue::Kill];

/// Most sounds playing at once. A new one past this stops the oldest.
const MAX_VOICES: usize = 16;

/// How loud a sound plays at a volume setting of 0–100, as a share of its full
/// loudness. Ears hear loudness on a curve, so the setting is squared: 50 plays at a
/// quarter.
pub fn gain(volume: u32) -> f32 {
    let v = volume.min(100) as f32 / 100.0;
    v * v
}

/// The sounds for one frame's events. A hitscan shot is a pop, plus a tick when it hits
/// or a chime when it kills. A beam hits many times a second, so it only gets the chime.
/// A long frame can fire several shots, and each sound still plays once.
pub fn cues(events: &[Event], beam: bool, sound: &Sound) -> Vec<Cue> {
    let mut out = Vec::new();
    if sound.volume == 0 {
        return out;
    }
    let hit = !beam && events.iter().any(|e| matches!(e, Event::Hit { .. }));
    let shot = hit || (!beam && events.iter().any(|e| matches!(e, Event::Miss)));
    let kill = events.iter().any(|e| matches!(e, Event::Killed { .. }));
    if sound.shots && shot {
        out.push(Cue::Shot);
    }
    if sound.hits {
        if kill {
            out.push(Cue::Kill);
        } else if hit {
            out.push(Cue::Hit);
        }
    }
    out
}

/// Loudest sample of each cue at full volume. The pop is the quietest, since it plays
/// on every click.
fn peak(cue: Cue) -> f32 {
    match cue {
        Cue::Shot => 0.3,
        Cue::Hit => 0.3,
        Cue::Kill => 0.35,
    }
}

/// A cue's samples at `rate` samples a second: one channel, starting and ending at 0.
pub fn synth(cue: Cue, rate: u32) -> Vec<f32> {
    let r = rate.max(1) as f32;
    let len = |seconds: f32| (seconds * r).round() as usize;
    let tau = std::f32::consts::TAU;
    // Rises from 0 over `a` seconds, then holds.
    let attack = |t: f32, a: f32| (t / a).min(1.0);
    let mut out: Vec<f32> = match cue {
        Cue::Shot => {
            // A puff of noise between about 700 Hz and 4.5 kHz (the difference of two
            // smoothed copies), gone in a few milliseconds, over a knock that falls
            // from about 450 to 240 Hz.
            let mut noise = Noise(0x9e37_79b9);
            let smooth = |hz: f32| 1.0 - (-tau * hz / r).exp();
            let (high, low) = (smooth(4500.0), smooth(700.0));
            let (mut a, mut b, mut phase) = (0.0f32, 0.0f32, 0.0f32);
            // The puff carries about 12 dB more energy than the knock: a snap with a
            // little body, not a thump.
            (0..len(0.04))
                .map(|i| {
                    let t = i as f32 / r;
                    let x = noise.next();
                    a += high * (x - a);
                    b += low * (x - b);
                    let puff = 4.0 * (a - b) * attack(t, 0.0004) * (-t / 0.004).exp();
                    phase += tau * (240.0 + 210.0 * (-t / 0.006).exp()) / r;
                    let knock = 0.3 * phase.sin() * attack(t, 0.001) * (-t / 0.006).exp();
                    puff + knock
                })
                .collect()
        }
        Cue::Hit => (0..len(0.045))
            .map(|i| {
                let t = i as f32 / r;
                let tone = (tau * 2000.0 * t).sin() + 0.35 * (tau * 3000.0 * t).sin();
                tone * attack(t, 0.001) * (-t / 0.010).exp()
            })
            .collect(),
        Cue::Kill => (0..len(0.22))
            .map(|i| {
                let t = i as f32 / r;
                let tone = (tau * 1046.5 * t).sin() + 0.5 * (tau * 1568.0 * t).sin() + 0.2 * (tau * 2093.0 * t).sin();
                tone * attack(t, 0.002) * (-t / 0.055).exp()
            })
            .collect(),
    };
    // The last few milliseconds fade to nothing, so a cue never ends on a click.
    let fade = len(0.004).clamp(1, out.len().max(1));
    let n = out.len();
    for (k, s) in out[n.saturating_sub(fade)..].iter_mut().enumerate() {
        *s *= 1.0 - (k + 1) as f32 / fade as f32;
    }
    let loudest = out.iter().fold(0.0f32, |m, s| m.max(s.abs()));
    if loudest > 0.0 {
        let k = peak(cue) / loudest;
        out.iter_mut().for_each(|s| *s *= k);
    }
    out
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

/// Plays cues over each other, at the device's rate.
pub struct Mixer {
    /// Each cue's samples, by its number.
    bank: Vec<Vec<f32>>,
    voices: Vec<Voice>,
}

struct Voice {
    cue: usize,
    at: usize,
    gain: f32,
}

impl Mixer {
    pub fn new(rate: u32) -> Mixer {
        Mixer { bank: CUES.iter().map(|&c| synth(c, rate)).collect(), voices: Vec::with_capacity(MAX_VOICES) }
    }

    /// Starts `cue` at `gain` (0–1) of its full loudness.
    pub fn play(&mut self, cue: Cue, gain: f32) {
        if gain.is_nan() || gain <= 0.0 {
            return;
        }
        if self.voices.len() == MAX_VOICES {
            self.voices.remove(0);
        }
        self.voices.push(Voice { cue: cue as usize, at: 0, gain: gain.min(1.0) });
    }

    /// Fills `out` with the sounds playing, the same in each of `channels` interleaved
    /// channels, and forgets those that have ended.
    pub fn fill(&mut self, out: &mut [f32], channels: usize) {
        let Mixer { bank, voices } = self;
        for frame in out.chunks_mut(channels.max(1)) {
            let mut v = 0.0;
            for voice in voices.iter_mut() {
                if let Some(s) = bank[voice.cue].get(voice.at) {
                    v += s * voice.gain;
                    voice.at += 1;
                }
            }
            frame.fill(v.clamp(-1.0, 1.0));
        }
        voices.retain(|v| v.at < bank[v.cue].len());
    }
}

/// Where the trainer sends its sounds. With no sound device, or in a bot test, there
/// is nowhere to send them and the trainer plays silently.
#[derive(Resource, Default)]
pub struct Speaker {
    to: Option<Sender<(Cue, f32)>>,
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

    /// Plays `cue` at `gain` (0–1). Does nothing without a device.
    pub fn play(&self, cue: Cue, gain: f32) {
        if let Some(to) = &self.to {
            // The device thread has gone only when there is no device: play silently.
            let _ = to.send((cue, gain));
        }
    }
}

/// The sound device, through cpal (WASAPI on Windows, ALSA on Linux).
mod device {
    use std::sync::mpsc::Receiver;
    use std::sync::{Arc, Mutex};

    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    use cpal::{BufferSize, FromSample, SampleFormat, SizedSample, Stream, StreamConfig, SupportedBufferSize};

    use super::{Cue, Mixer};

    /// Frames in each block the device asks for, when it lets the trainer choose: about
    /// 5 ms at 48 kHz, so a pop is heard a few milliseconds after the click.
    const BLOCK: u32 = 256;

    type Cues = Arc<Mutex<Receiver<(Cue, f32)>>>;

    /// Starts the default output device playing whatever `from` sends. The stream plays
    /// until it is dropped.
    pub(super) fn start(from: Receiver<(Cue, f32)>) -> Result<Stream, String> {
        let device = cpal::default_host().default_output_device().ok_or("no sound output device")?;
        let supported = device.default_output_config().map_err(|e| e.to_string())?;
        let format = supported.sample_format();
        let usual = supported.config();
        let from: Cues = Arc::new(Mutex::new(from));
        // A short block if the device has a range to pick from, and its own if that fails.
        if let SupportedBufferSize::Range { min, max } = *supported.buffer_size() {
            let short = StreamConfig { buffer_size: BufferSize::Fixed(BLOCK.clamp(min, max)), ..usual };
            if let Ok(stream) = build(&device, format, short, from.clone()) {
                return Ok(stream);
            }
        }
        build(&device, format, usual, from)
    }

    fn build(device: &cpal::Device, format: SampleFormat, config: StreamConfig, from: Cues) -> Result<Stream, String> {
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
        from: Cues,
    ) -> Result<Stream, String> {
        let channels = usize::from(config.channels);
        let mut mixer = Mixer::new(config.sample_rate);
        let mut mixed: Vec<f32> = Vec::new();
        let stream = device
            .build_output_stream(
                config,
                move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
                    // Only this stream reads the cues, so the lock is always free.
                    if let Ok(cues) = from.try_lock() {
                        while let Ok((cue, gain)) = cues.try_recv() {
                            mixer.play(cue, gain);
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

    /// The pop has to be short and soft: a click that tells you a shot went off, not a gunshot that wears on you
    /// over hundreds of shots (asked for Nov 19, 2026).
    #[test]
    fn the_pop_is_short_and_quieter_than_full_scale() {
        let pop = synth(Cue::Shot, RATE);
        assert!(ms(pop.len()) <= 60.0, "{} ms", ms(pop.len()));
        let loudest = pop.iter().fold(0.0f32, |m, s| m.max(s.abs()));
        assert!((loudest - 0.3).abs() < 1e-6, "{loudest}");
        // Most of its energy is in the first 15 ms: a pop, not a hiss.
        let energy = |s: &[f32]| s.iter().map(|x| x * x).sum::<f32>();
        let early = energy(&pop[..(RATE as usize * 15 / 1000)]);
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

    #[test]
    fn every_cue_starts_and_ends_at_silence_and_is_the_same_each_time() {
        for cue in CUES {
            for rate in [44_100, 48_000, 96_000] {
                let s = synth(cue, rate);
                assert!(!s.is_empty(), "{cue:?}");
                assert_eq!(s[0], 0.0, "{cue:?} starts with a jump");
                assert_eq!(*s.last().unwrap(), 0.0, "{cue:?} ends with a jump");
                assert!(s.iter().all(|x| x.is_finite() && x.abs() <= peak(cue) + 1e-6), "{cue:?}");
                assert_eq!(s, synth(cue, rate), "{cue:?} changes from run to run");
            }
        }
        assert!(ms(synth(Cue::Hit, RATE).len()) <= 60.0);
        assert!(ms(synth(Cue::Kill, RATE).len()) <= 250.0);
    }

    fn sound(volume: u32, shots: bool, hits: bool) -> Sound {
        Sound { volume, shots, hits }
    }

    #[test]
    fn a_shot_pops_and_ticks_on_a_hit_or_chimes_on_a_kill() {
        let all = sound(50, true, true);
        assert_eq!(cues(&[Event::Miss], false, &all), [Cue::Shot]);
        assert_eq!(cues(&[Event::Hit { id: 1 }], false, &all), [Cue::Shot, Cue::Hit]);
        assert_eq!(cues(&[Event::Hit { id: 1 }, Event::Killed { id: 1 }], false, &all), [Cue::Shot, Cue::Kill]);
        assert_eq!(cues(&[Event::Spawned { id: 2 }, Event::TimedOut { id: 1 }], false, &all), []);
        // Three shots in one long frame still play one of each.
        let three = [Event::Miss, Event::Hit { id: 1 }, Event::Miss];
        assert_eq!(cues(&three, false, &all), [Cue::Shot, Cue::Hit]);
    }

    /// A beam hits 60 times a second while it's on a target: a tick on each would be a buzz.
    #[test]
    fn a_beam_only_chimes_on_a_kill() {
        let all = sound(50, true, true);
        assert_eq!(cues(&[Event::Hit { id: 1 }, Event::Hit { id: 1 }], true, &all), []);
        assert_eq!(cues(&[Event::Hit { id: 1 }, Event::Killed { id: 1 }], true, &all), [Cue::Kill]);
    }

    #[test]
    fn each_sound_can_be_turned_off() {
        let kill = [Event::Hit { id: 1 }, Event::Killed { id: 1 }];
        assert_eq!(cues(&kill, false, &sound(50, false, true)), [Cue::Kill]);
        assert_eq!(cues(&kill, false, &sound(50, true, false)), [Cue::Shot]);
        assert_eq!(cues(&kill, false, &sound(0, true, true)), []);
    }

    #[test]
    fn volume_is_heard_on_a_curve() {
        assert_eq!(gain(0), 0.0);
        assert_eq!(gain(50), 0.25);
        assert_eq!(gain(100), 1.0);
        assert_eq!(gain(250), 1.0, "past 100 is 100");
    }

    #[test]
    fn the_mixer_adds_sounds_up_in_every_channel_and_forgets_finished_ones() {
        let mut m = Mixer::new(RATE);
        let hit = synth(Cue::Hit, RATE);
        m.play(Cue::Hit, 1.0);
        m.play(Cue::Hit, 0.5);
        let mut out = vec![9.0; 2 * hit.len()];
        m.fill(&mut out, 2);
        for (i, s) in hit.iter().enumerate() {
            assert!((out[2 * i] - 1.5 * s).abs() < 1e-6, "sample {i}");
            assert_eq!(out[2 * i], out[2 * i + 1], "both channels");
        }
        assert!(m.voices.is_empty(), "both ended");
        let mut after = vec![9.0; 8];
        m.fill(&mut after, 2);
        assert_eq!(after, [0.0; 8], "silence once nothing plays");
    }

    /// Plays a pop, a pop with a tick and a pop with a chime on this computer's speakers at
    /// the default volume, to hear them. Needs a sound device, so it's run by hand:
    /// `cargo test -p aimadapt-engine -- --ignored hear_the_sounds`.
    #[test]
    #[ignore = "plays sound; run by hand"]
    fn hear_the_sounds() {
        let speaker = Speaker::open();
        let g = gain(Sound::default().volume);
        for pair in [[Some(Cue::Shot), None], [Some(Cue::Shot), Some(Cue::Hit)], [Some(Cue::Shot), Some(Cue::Kill)]] {
            for cue in pair.into_iter().flatten() {
                speaker.play(cue, g);
            }
            std::thread::sleep(std::time::Duration::from_millis(500));
        }
    }

    #[test]
    fn the_mixer_never_goes_past_full_scale_or_plays_too_many_at_once() {
        let mut m = Mixer::new(RATE);
        for _ in 0..40 {
            m.play(Cue::Kill, 1.0);
        }
        assert_eq!(m.voices.len(), MAX_VOICES);
        m.play(Cue::Hit, 0.0);
        m.play(Cue::Hit, f32::NAN);
        assert_eq!(m.voices.len(), MAX_VOICES, "silent cues aren't played");
        let mut out = vec![0.0; 4096];
        m.fill(&mut out, 1);
        assert!(out.iter().all(|s| s.abs() <= 1.0));
    }
}
