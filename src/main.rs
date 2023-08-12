use anyhow::Result;
use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    Sample,
};
use crossterm::event::{read, Event, KeyCode, KeyEvent};
use crossterm::terminal::enable_raw_mode;
use std::io::{self, Write};
use std::sync::{Arc, Mutex};

fn main() -> Result<()> {
    // Enable raw mode
    enable_raw_mode()?;

    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .expect("failed to find output device");
    println!("Output device: {}", device.name()?);

    let config = device.default_output_config().unwrap();
    println!("Default output config: {:?}", config);

    match config.sample_format() {
        cpal::SampleFormat::F32 => run::<f32>(&device, &config.into())?,
        sample_format => panic!("Unsupported sample format '{:?}'", sample_format),
    }

    Ok(())
}

pub fn run<T>(device: &cpal::Device, config: &cpal::StreamConfig) -> Result<(), anyhow::Error>
where
    T: cpal::Sample + cpal::FromSample<f32> + cpal::SizedSample, // Added SizedSample trait
{
    let sample_rate = config.sample_rate.0 as f32;
    let channels = config.channels as usize;

    let mut sample_clock = 0f32;
    let tone_on = Arc::new(Mutex::new(true));
    let tone_on_clone = tone_on.clone();

    let mut next_value = move || {
        if *tone_on.lock().unwrap() {
            sample_clock = (sample_clock + 1.0) % sample_rate;
            (sample_clock * 440.0 * 2.0 * std::f32::consts::PI / sample_rate).sin()
        } else {
            0.0
        }
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
            if code == KeyCode::Char('t') {
                let mut on = tone_on_clone.lock().unwrap();
                *on = !*on;
            }
            if code == KeyCode::Esc {
                break;
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
