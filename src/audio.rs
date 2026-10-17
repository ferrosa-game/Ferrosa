use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

use bevy::prelude::*;
use cpal::{
    SampleFormat,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
use rodio::{DeviceSinkBuilder, MixerDeviceSink, mixer::Mixer};

use crate::config::config_dir;

pub const MIC_RATE: u32 = 48000;
const MIC_KEPT: usize = MIC_RATE as usize;
const DEVICES_FILE: &str = "audio_devices.txt";
const DEFAULT_DEVICE: &str = "Default";

#[derive(Resource)]
pub struct AudioDevices {
    pub output: Vec<String>,
    pub input: Vec<String>,
    pub chosen_output: usize,
    pub chosen_input: usize,
}

#[derive(Resource, Default)]
pub struct Speakers(pub Option<Mixer>);

#[derive(Resource, Clone, Default)]
pub struct MicSamples(pub Arc<Mutex<VecDeque<i16>>>);

#[derive(Default)]
struct OpenDevices {
    speakers: Option<MixerDeviceSink>,
    microphone: Option<cpal::Stream>,
}

pub struct DevicesPlugin;

impl Plugin for DevicesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Speakers>()
            .init_resource::<MicSamples>()
            .insert_non_send(OpenDevices::default())
            .add_systems(Startup, list_devices)
            .add_systems(PostUpdate, open_devices.run_if(resource_exists_and_changed::<AudioDevices>));
    }
}

fn device_name(d: &cpal::Device) -> Option<String> {
    let description = d.description().ok()?;

    Some(description.extended().first().map_or_else(|| description.name().to_string(), Clone::clone))
}

fn names(devices: Option<impl Iterator<Item = cpal::Device>>) -> Vec<String> {
    std::iter::once(DEFAULT_DEVICE.to_string()).chain(devices.into_iter().flatten().filter_map(|d| device_name(&d))).collect()
}

fn saved_choices() -> (String, String) {
    let text = std::fs::read_to_string(config_dir().join(DEVICES_FILE)).unwrap_or_default();
    let line = |key: &str| text.lines().find_map(|l| l.strip_prefix(key)).unwrap_or(DEFAULT_DEVICE).to_string();

    (line("output="), line("input="))
}

fn list_devices(mut commands: Commands) {
    let host = cpal::default_host();
    let output = names(host.output_devices().ok());
    let input = names(host.input_devices().ok());
    let (saved_output, saved_input) = saved_choices();
    let chosen = |list: &[String], saved: &str| list.iter().position(|n| n == saved).unwrap_or(0);

    commands.insert_resource(AudioDevices { chosen_output: chosen(&output, &saved_output), chosen_input: chosen(&input, &saved_input), output, input });
}

fn find(devices: Option<impl Iterator<Item = cpal::Device>>, name: &str) -> Option<cpal::Device> {
    devices.into_iter().flatten().find(|d| device_name(d).as_deref() == Some(name))
}

fn open_speakers(name: &str) -> Option<MixerDeviceSink> {
    let host = cpal::default_host();
    let device = if name == DEFAULT_DEVICE { host.default_output_device() } else { find(host.output_devices().ok(), name) }?;
    let mut sink = DeviceSinkBuilder::from_device(device).and_then(|b| b.open_stream()).inspect_err(|e| warn!("audio output {name} not opened: {e}")).ok()?;

    sink.log_on_drop(false);

    Some(sink)
}

fn open_microphone(name: &str, samples: MicSamples) -> Option<cpal::Stream> {
    let host = cpal::default_host();
    let device = if name == DEFAULT_DEVICE { host.default_input_device() } else { find(host.input_devices().ok(), name) }?;
    let config = device.default_input_config().inspect_err(|e| warn!("audio input {name} has no config: {e}")).ok()?;
    let (channels, rate, format) = (config.channels() as usize, config.sample_rate(), config.sample_format());
    let mut resampler = Resampler { step: MIC_RATE as f64 / rate as f64, phase: 0.0, samples };
    let error = |e: cpal::StreamError| warn!("microphone: {e}");

    let stream = match format {
        SampleFormat::F32 => device.build_input_stream(&config.into(), move |data: &[f32], _: &_| resampler.push(data, channels, |s| s), error, None),
        SampleFormat::I16 => device.build_input_stream(&config.into(), move |data: &[i16], _: &_| resampler.push(data, channels, |s| s as f32 / 32768.0), error, None),
        SampleFormat::U16 => device.build_input_stream(&config.into(), move |data: &[u16], _: &_| resampler.push(data, channels, |s| (s as f32 - 32768.0) / 32768.0), error, None),
        other => {
            warn!("audio input {name} gives {other:?} samples");
            return None;
        }
    }
    .inspect_err(|e| warn!("audio input {name} not opened: {e}"))
    .ok()?;

    stream.play().inspect_err(|e| warn!("audio input {name} not started: {e}")).ok()?;

    Some(stream)
}

struct Resampler {
    step: f64,
    phase: f64,
    samples: MicSamples,
}

impl Resampler {
    fn push<T: Copy>(&mut self, data: &[T], channels: usize, to_float: impl Fn(T) -> f32) {
        let Ok(mut out) = self.samples.0.lock() else { return };

        for frame in data.chunks(channels.max(1)) {
            let mono = frame.iter().map(|&s| to_float(s)).sum::<f32>() / frame.len() as f32;
            self.phase += self.step;

            while self.phase >= 1.0 {
                out.push_back((mono * 32767.0).clamp(-32768.0, 32767.0) as i16);
                self.phase -= 1.0;
            }
        }

        let excess = out.len().saturating_sub(MIC_KEPT);
        out.drain(..excess);
    }
}

fn open_devices(devices: Res<AudioDevices>, mut open: NonSendMut<OpenDevices>, mut speakers: ResMut<Speakers>, mic: Res<MicSamples>) {
    let output = devices.output.get(devices.chosen_output).map_or(DEFAULT_DEVICE, String::as_str);
    let input = devices.input.get(devices.chosen_input).map_or(DEFAULT_DEVICE, String::as_str);

    open.speakers = None;
    open.speakers = open_speakers(output);
    speakers.0 = open.speakers.as_ref().map(|s| s.mixer().clone());

    open.microphone = None;
    mic.0.lock().map(|mut m| m.clear()).ok();
    open.microphone = open_microphone(input, mic.clone());

    let saved = format!("output={output}\ninput={input}\n");

    if let Err(e) = std::fs::write(config_dir().join(DEVICES_FILE), saved) {
        warn!("{DEVICES_FILE} not saved: {e}");
    }
}
