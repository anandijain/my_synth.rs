use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    FromSample, Sample, SizedSample,
};
use crossterm::event::{read, KeyCode, KeyEvent};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use std::io::{self, Write};
use std::sync::{Arc, Mutex};
use std::thread::sleep;
use std::time::Duration;

fn main() -> anyhow::Result<()> {
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .expect("failed to find output device");
    println!("Output device: {}", device.name()?);

    let config = device.default_output_config().unwrap().into();
    println!("Default output config: {:?}", config);

    run::<f32>(&device, &config)
}

pub fn run<T>(device: &cpal::Device, config: &cpal::StreamConfig) -> Result<(), anyhow::Error>
where
    T: SizedSample + FromSample<f32>,
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
        None,
    )?;
    stream.play()?;

    std::thread::spawn(move || loop {
        if let Ok(crossterm::event::Event::Key(KeyEvent { code, .. })) = read() {
            if code == KeyCode::Char('t') {
                let mut on = tone_on_clone.lock().unwrap();
                *on = !*on;
            }
            if code == KeyCode::Char('q') {
                break;
            }
        }
    });

    loop {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    Ok(())
}

fn write_data<T>(output: &mut [T], channels: usize, next_sample: &mut dyn FnMut() -> f32)
where
    T: Sample + FromSample<f32>,
{
    for frame in output.chunks_mut(channels) {
        let value: T = T::from_sample(next_sample());
        for sample in frame.iter_mut() {
            *sample = value;
        }
    }
}
