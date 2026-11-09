//! Usage: spike-voicevox <onnxruntime.dll> <dict_dir> <vvm> <out.wav> [text]
use std::time::Instant;

use anyhow::Context;
use voicevox_core::blocking::{Onnxruntime, OpenJtalk, Synthesizer, VoiceModelFile};

fn main() -> anyhow::Result<()> {
    let a: Vec<String> = std::env::args().collect();
    let text = a.get(5).map(String::as_str).unwrap_or("料理がおいしくてつい食べ過ぎた。");

    let t = Instant::now();
    let ort = Onnxruntime::load_once().filename(&a[1]).perform().context("load onnxruntime")?;
    let synth = Synthesizer::builder(ort).text_analyzer(OpenJtalk::new(a[2].as_str())?).build()?;
    let model = VoiceModelFile::open(a[3].as_str())?;
    synth.load_voice_model(&model).perform()?;
    println!("init: {:?}", t.elapsed());

    for m in synth.metas().iter() {
        println!("speaker: {} -> {:?}", m.name, m.styles.iter().map(|s| (&s.name, s.id)).collect::<Vec<_>>());
    }
    let style = synth.metas().iter().flat_map(|m| m.styles.iter()).next().context("no style")?.id;

    let t = Instant::now();
    let wav = synth.tts(text, style).perform()?;
    println!("tts: {:?}, {} bytes", t.elapsed(), wav.len());
    std::fs::write(&a[4], wav)?;
    Ok(())
}
