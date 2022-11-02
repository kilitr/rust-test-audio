use anyhow;
use clap::arg;
use clap::Parser;
use cpal::traits::StreamTrait;
use cpal::traits::{DeviceTrait, HostTrait};
use ringbuf::RingBuffer;

#[derive(Parser, Debug)]
#[command(version, about = "CPAL beep example", long_about = None)]
struct Opt {
    /// The audio device to use
    #[arg(short, long, default_value_t = String::from("default"))]
    output_device: String,

    #[arg(short, long, default_value_t = String::from("default"))]
    input_device: String,

    #[arg(short, long, default_value_t = 150f32)]
    latency: f32,
}

fn main() -> anyhow::Result<()> {
    let opt = Opt::parse();

    let host = cpal::default_host();

    let output_device = if opt.output_device == "default" {
        host.default_output_device()
    } else {
        host.output_devices()
            .expect("No output devices recognized")
            .find(|x| x.name().map(|y| y == opt.output_device).unwrap_or(false))
    }
    .expect("failed to find output device");
    println!("Output device: {}", output_device.name().unwrap());

    let input_device = if opt.input_device == "default" {
        host.default_input_device()
    } else {
        host.input_devices()
            .expect("No input devices recognized")
            .find(|x| x.name().map(|y| y == opt.input_device).unwrap_or(false))
    }
    .expect("failed to find output device");
    println!("Input device: {}", input_device.name().unwrap());

    let config: cpal::StreamConfig = input_device.default_input_config().unwrap().into();
    println!("Default config: {:?}", config);

    // Create a delay in case the input and output devices aren't synced.
    let latency_frames = (opt.latency / 1_000.0) * config.sample_rate.0 as f32;
    let latency_samples = latency_frames as usize * config.channels as usize;

    let ring = RingBuffer::new(latency_samples * 2);
    let (mut producer, mut consumer) = ring.split();

    for _ in 0..latency_samples {
        producer.push(0.0).unwrap();
    }

    let input_data_fn = move |data: &[f32], _: &cpal::InputCallbackInfo| {
        let mut output_fell_behind = false;
        for &sample in data {
            if producer.push(sample).is_err() {
                output_fell_behind = true;
            }
        }
        if output_fell_behind {
            eprintln!("output stream fell behind: try increasing latency");
        }
    };

    let output_data_fn = move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
        let mut input_fell_behind = false;
        for sample in data {
            *sample = match consumer.pop() {
                Some(s) => s,
                None => {
                    input_fell_behind = true;
                    0.0
                }
            };
        }
        if input_fell_behind {
            eprintln!("input stream fell behind: try increasing latency");
        }
    };

    println!(
        "Attempting to build both streams with f32 samples and `{:?}`.",
        config
    );
    let input_stream = input_device
        .build_input_stream(&config, input_data_fn, err_fn)
        .unwrap();
    let output_stream = output_device
        .build_output_stream(&config, output_data_fn, err_fn)
        .unwrap();

    println!("Successfully built streams.");

    println!(
        "Starting the input and output streams with `{}` milliseconds of latency.",
        opt.latency
    );
    input_stream.play().unwrap();
    input_stream.play().unwrap();

    println!("Playing for 3 seconds... ");
    std::thread::sleep(std::time::Duration::from_secs(3));
    drop(input_stream);
    drop(output_stream);
    println!("Done!");

    Ok(())
}

fn err_fn(err: cpal::StreamError) {
    eprintln!("an error occurred on stream: {}", err);
}
