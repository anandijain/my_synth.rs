#[macro_use]
extern crate lazy_static;

use anyhow::Result;
use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    Sample, SupportedStreamConfig,
};
use crossterm::event::{read, Event, KeyCode, KeyEvent};
use crossterm::terminal::enable_raw_mode;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

lazy_static! {
    static ref NOTE_MAPPING: Vec<(&'static str, i32)> = vec![
        ("C", 0),
        ("C#", 1),
        ("D", 2),
        ("D#", 3),
        ("E", 4),
        ("F", 5),
        ("F#", 6),
        ("G", 7),
        ("G#", 8),
        ("A", 9),
        ("A#", 10),
        ("B", 11),
    ];
}

lazy_static! {
    static ref KEY_MAP: HashMap<char, (&'static str, i32)> = {
        let mut map = HashMap::new();
        map.insert('a', ("C", 0));
        map.insert('w', ("C#", 0));
        map.insert('s', ("D", 0));
        map.insert('e', ("D#", 0));
        map.insert('d', ("E", 0));
        map.insert('f', ("F", 0));
        map.insert('t', ("F#", 0));
        map.insert('g', ("G", 0));
        map.insert('y', ("G#", 0));
        map.insert('h', ("A", 0));
        map.insert('u', ("A#", 0));
        map.insert('j', ("B", 0));
        map.insert('k', ("C", 1)); // Next octave
        map.insert('o', ("C#", 1));
        map.insert('l', ("D", 1));
        map
    };
}

fn midi_to_freq(midi_note: i32) -> f64 {
    440.0 * 2.0f64.powf((midi_note - 69) as f64 / 12.0)
}

fn note_to_frequency(note: &str, octave: i32) -> f64 {
    let relative_note_number = NOTE_MAPPING
        .iter()
        .find(|&&(n, _)| n == note)
        .map(|&(_, number)| number)
        .unwrap_or(0);

    let midi_note_number = 12 * (octave + 1) + relative_note_number;
    midi_to_freq(midi_note_number)
}

const RELEASE_TIME_SECONDS: f32 = 3.0;

fn main() -> Result<()> {
    // Enable raw mode
    enable_raw_mode()?;

    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .expect("failed to find output device");
    let def_config = device.default_output_config().unwrap();

    let config = SupportedStreamConfig::new(
        def_config.channels(),
        def_config.sample_rate(),
        cpal::SupportedBufferSize::Range { min: 16, max: 32 },
        def_config.sample_format(),
    );

    println!("Default output config: {:?}", def_config);
    println!("my config: {:?}", config);

    match config.sample_format() {
        cpal::SampleFormat::F32 => run::<f32>(&device, &def_config.into())?,
        sample_format => panic!("Unsupported sample format '{:?}'", sample_format),
    }

    Ok(())
}

pub fn run<T>(device: &cpal::Device, config: &cpal::StreamConfig) -> Result<(), anyhow::Error>
where
    T: cpal::Sample + cpal::FromSample<f32> + cpal::SizedSample,
{
    let mut current_octave = 3;
    let sample_rate = config.sample_rate.0 as f32;
    let channels = config.channels as usize;

    let mut sample_clock = 0f32;
    let frequency = Arc::new(Mutex::new(0.0f32)); // Will hold the frequency
    let frequency_clone = frequency.clone();

    let mut amplitude = 0.0f32;
    let target_amplitude = 0.5; // Desired amplitude
    let ramp_speed = 0.01; // Speed of the ramp for smoothing

    let release_ramp_speed = target_amplitude / (sample_rate * RELEASE_TIME_SECONDS);

    let mut releasing = false;

    let mut next_value = move || {
        let freq = *frequency.lock().unwrap();
        if freq > 0.0 {
            releasing = false;
            if amplitude < target_amplitude {
                amplitude += ramp_speed; // Ramp up the amplitude
            }
        } else {
            if !releasing {
                releasing = true;
            }
            if amplitude > 0.0 {
                amplitude -= release_ramp_speed; // Ramp down the amplitude
            }
        }
        sample_clock = (sample_clock + 1.0) % sample_rate;
        (sample_clock * freq * 2.0 * std::f32::consts::PI / sample_rate).sin() * amplitude
    };

    let err_fn = |err| eprintln!("an error occurred on stream: {}", err);

    let stream = device.build_output_stream(
        config,
        move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
            write_data(data, channels, &mut next_value)
        },
        err_fn,
        None, // Added Option<Duration>
    )?;
    stream.play()?;

    loop {
        if let Ok(Event::Key(KeyEvent { code, .. })) = read() {
            match code {
                KeyCode::Char(' ') => {
                    *frequency_clone.lock().unwrap() = 0.0;
                }
                KeyCode::Char('z') => {
                    if current_octave > 0 {
                        current_octave -= 1;
                    }
                }
                KeyCode::Char('x') => {
                    if current_octave < 8 {
                        current_octave += 1;
                    }
                }
                KeyCode::Char(c) => {
                    if let Some(&(note, octave_offset)) = KEY_MAP.get(&c) {
                        let freq = note_to_frequency(note, current_octave + octave_offset) as f32;
                        *frequency_clone.lock().unwrap() = freq;
                    }
                }
                KeyCode::Esc => break,
                _ => {}
            }
        }
    }

    Ok(())
}

fn write_data<T>(output: &mut [T], channels: usize, next_sample: &mut dyn FnMut() -> f32)
where
    T: cpal::Sample + cpal::FromSample<f32>,
{
    for frame in output.chunks_mut(channels) {
        let value: T = T::from_sample(next_sample());
        for sample in frame.iter_mut() {
            *sample = value;
        }
    }
}
