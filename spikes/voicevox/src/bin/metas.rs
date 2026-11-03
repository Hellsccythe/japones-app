//! Prints the characters and styles inside VVM files, one line per style:
//! `<file>\t<character>\t<style id>\t<style name>\t<style type>`.
//! Usage: metas <a.vvm> [b.vvm ...]
use voicevox_core::blocking::VoiceModelFile;

fn main() -> anyhow::Result<()> {
    for path in std::env::args().skip(1) {
        let model = VoiceModelFile::open(path.as_str())?;
        let file = std::path::Path::new(&path).file_name().unwrap().to_string_lossy().into_owned();
        for c in model.metas().iter() {
            for s in &c.styles {
                println!("{file}\t{}\t{}\t{}\t{:?}", c.name, s.id.0, s.name, s.r#type);
            }
        }
    }
    Ok(())
}
