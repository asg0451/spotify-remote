use librespot::playback::{
    audio_backend::{Sink, SinkAsBytes, SinkResult},
    config::AudioFormat,
    convert,
    decoder::AudioPacket,
};

pub struct BufSink {
    format: AudioFormat,
    tx: Option<std::sync::mpsc::SyncSender<Vec<u8>>>,
    rx: Option<std::sync::mpsc::Receiver<Vec<u8>>>,
}

impl BufSink {
    pub fn rx(&mut self) -> std::sync::mpsc::Receiver<Vec<u8>> {
        self.rx.take().unwrap()
    }

    pub fn new(format: AudioFormat) -> Self {
        let (tx, rx) = std::sync::mpsc::sync_channel(100);
        Self {
            format,
            tx: Some(tx),
            rx: Some(rx),
        }
    }
}

impl Sink for BufSink {
    fn start(&mut self) -> SinkResult<()> {
        Ok(())
    }

    // the sink can be stopped and started for a variety of reasons, eg pausing
    // we don't want to close the channel in that case
    // BUT actually not doing anything on close leads to spotify hanging on resume. TODO: why
    // TODO: how can we legitimately close it when we're done?

    fn stop(&mut self) -> SinkResult<()> {
        tracing::info!("stopping sink");
        self.tx.take();
        Ok(())
    }

    fn write(
        &mut self,
        packet: librespot::playback::decoder::AudioPacket,
        converter: &mut librespot::playback::convert::Converter,
    ) -> SinkResult<()> {
        use convert::i24;
        use zerocopy::AsBytes;
        match packet {
            AudioPacket::Samples(samples) => match self.format {
                AudioFormat::F64 => self.write_bytes(samples.as_bytes()),
                AudioFormat::F32 => {
                    let samples_f32: &[f32] = &converter.f64_to_f32(&samples);
                    self.write_bytes(samples_f32.as_bytes())
                }
                AudioFormat::S32 => {
                    let samples_s32: &[i32] = &converter.f64_to_s32(&samples);
                    self.write_bytes(samples_s32.as_bytes())
                }
                AudioFormat::S24 => {
                    let samples_s24: &[i32] = &converter.f64_to_s24(&samples);
                    self.write_bytes(samples_s24.as_bytes())
                }
                AudioFormat::S24_3 => {
                    let samples_s24_3: &[i24] = &converter.f64_to_s24_3(&samples);
                    self.write_bytes(samples_s24_3.as_bytes())
                }
                AudioFormat::S16 => {
                    let samples_s16: &[i16] = &converter.f64_to_s16(&samples);
                    self.write_bytes(samples_s16.as_bytes())
                }
            },
            AudioPacket::OggData(samples) => self.write_bytes(&samples),
        }
    }
}

impl SinkAsBytes for BufSink {
    fn write_bytes(&mut self, data: &[u8]) -> SinkResult<()> {
        if let Some(tx) = &self.tx {
            tx.send(data.to_vec()).map_err(|e| {
                librespot::playback::audio_backend::SinkError::OnWrite(e.to_string())
            })?;
        }
        Ok(())
    }
}
