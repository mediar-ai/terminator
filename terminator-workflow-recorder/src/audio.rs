use std::fs::File;
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream};
use hound::{SampleFormat as HoundSampleFormat, WavSpec, WavWriter};

use crate::{Result, WorkflowRecorderError};

/// Simple audio recorder that captures microphone input and saves it to a WAV file.
///
/// This implementation is intentionally minimal and intended primarily for
/// prototype purposes.  It captures the default **input** device using the
/// default configuration returned by `cpal` and encodes samples as 16-bit PCM
/// within a `.wav` container via the `hound` crate.
///
/// It **does not** capture system/loopback audio on all platforms.  Capturing
/// loopback audio requires host-specific APIs (eg WASAPI loopback on Windows,
/// PulseAudio/ PipeWire monitor on Linux, etc).  Those enhancements can be
/// added later on top of this baseline implementation.
pub struct AudioRecorder {
    file_path: PathBuf,
    writer: Option<Arc<Mutex<WavWriter<BufWriter<File>>>>>,
    stream: Option<Stream>,
}

impl AudioRecorder {
    /// Create a new `AudioRecorder` that will write PCM samples to the file at
    /// `output_path`. The file will be created (overwriting if it already
    /// exists).
    pub fn new<P: AsRef<Path>>(output_path: P) -> Self {
        Self {
            file_path: output_path.as_ref().to_path_buf(),
            writer: None,
            stream: None,
        }
    }

    /// Start recording. This spawns the CPAL stream and begins writing samples
    /// to the WAV file. This function is idempotent – subsequent calls after a
    /// successful start will be ignored.
    pub fn start(&mut self) -> Result<()> {
        if self.stream.is_some() {
            // Already started – nothing to do.
            return Ok(());
        }

        // Prepare WAV writer first so that we can enqueue samples immediately
        // once the audio callback starts firing.
        let spec = {
            // We will overwrite this later with the real format once we know
            // it, but we need *some* placeholder to satisfy the type system.
            WavSpec {
                channels: 1,
                sample_rate: 44_100,
                bits_per_sample: 16,
                sample_format: HoundSampleFormat::Int,
            }
        };
        let writer = WavWriter::create(&self.file_path, spec).map_err(|e| {
            WorkflowRecorderError::AudioError(format!(
                "Failed to create WAV writer: {e}"
            ))
        })?;
        let writer = Arc::new(Mutex::new(writer));

        // Discover default input device + its default configuration.
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or_else(|| {
                WorkflowRecorderError::AudioError(
                    "No default input audio device found".to_string(),
                )
            })?;
        let config = device
            .default_input_config()
            .map_err(|e| {
                WorkflowRecorderError::AudioError(format!(
                    "Failed to query default input config: {e}"
                ))
            })?;

        // Build the input stream according to sample format.
        let sample_format = config.sample_format();
        let config = config.into();

        let writer_clone = Arc::clone(&writer);

        let stream = match sample_format {
            SampleFormat::F32 => self.build_stream::<f32>(&device, &config, writer_clone)?,
            SampleFormat::I16 => self.build_stream::<i16>(&device, &config, writer_clone)?,
            SampleFormat::U16 => self.build_stream::<u16>(&device, &config, writer_clone)?,
        };

        // Start the stream.
        stream.play().map_err(|e| {
            WorkflowRecorderError::AudioError(format!("Failed to start audio stream: {e}"))
        })?;

        // Update WAV spec with the real parameters now that we know them.
        {
            let mut guard = writer.lock().unwrap();
            guard.set_spec(WavSpec {
                channels: config.channels as u16,
                sample_rate: config.sample_rate.0,
                bits_per_sample: 16,
                sample_format: HoundSampleFormat::Int,
            });
        }

        self.writer = Some(writer);
        self.stream = Some(stream);
        Ok(())
    }

    /// Stop recording and finalize the WAV file. This is safe to call multiple
    /// times – subsequent invocations after successful completion are ignored.
    pub fn stop(&mut self) -> Result<()> {
        // Drop the CPAL stream first so that no more callbacks can occur while
        // we finalise the writer.
        self.stream.take();

        if let Some(writer) = self.writer.take() {
            let mut guard = writer.lock().unwrap();
            guard.flush().map_err(|e| {
                WorkflowRecorderError::AudioError(format!("Failed to flush WAV writer: {e}"))
            })?;
            guard.finalize().map_err(|e| {
                WorkflowRecorderError::AudioError(format!("Failed to finalize WAV file: {e}"))
            })?;
        }
        Ok(())
    }

    /// Internal helper for building a CPAL stream with a specific sample type.
    fn build_stream<T>(
        &self,
        device: &cpal::Device,
        config: &cpal::StreamConfig,
        writer: Arc<Mutex<WavWriter<BufWriter<File>>>>,
    ) -> Result<Stream>
    where
        T: cpal::Sample,
        T: cpal::FromSample<i16>,
    {
        let channels = config.channels as usize;

        let stream = device
            .build_input_stream(
                config,
                move |data: &[T], _| {
                    // Convert samples and write them.
                    let mut guard = writer.lock().unwrap();
                    for frame in data.chunks(channels) {
                        // For now we mix down to mono by taking the first channel.
                        let sample: i16 = frame[0].to_sample();
                        let _ = guard.write_sample(sample);
                    }
                },
                move |err| {
                    eprintln!("Audio stream error: {err}");
                },
                None,
            )
            .map_err(|e| {
                WorkflowRecorderError::AudioError(format!("Failed to build input stream: {e}"))
            })?;

        Ok(stream)
    }

    /// Returns the file path that this recorder writes to.
    pub fn file_path(&self) -> &Path {
        &self.file_path
    }
}