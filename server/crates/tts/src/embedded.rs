//! VOICEVOX CORE inside the app: no VOICEVOX engine to install or keep open.
//!
//! Needs three downloaded pieces: VOICEVOX's ONNX Runtime build (the official
//! voice models only run on it), the Open JTalk dictionary, and one `.vvm`
//! pack per group of voices (see [`crate::PACKS`]).

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use typed_floats::PositiveFinite;
use voicevox_core::blocking::{Onnxruntime, OpenJtalk, Synthesizer, VoiceModelFile};
use voicevox_core::StyleId;

use crate::{Error, Result};

/// A `.tgz`/`.tar.gz` download and the folder it unpacks to.
pub struct Archive {
    pub url: &'static str,
    pub size: u64,
    pub sha256: &'static str,
    /// Top-level folder inside the archive.
    pub dir: &'static str,
}

/// VOICEVOX ONNX Runtime 1.23.2 for this platform, and the library path
/// inside its folder. `None` where there's no build (embedded voices unavailable).
pub const ORT: Option<(Archive, &str)> = ort();

const fn ort() -> Option<(Archive, &'static str)> {
    if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
        Some((
            Archive {
                url: concat!(
                    "https://github.com/VOICEVOX/onnxruntime-builder/releases/download/voicevox_onnxruntime-1.23.2/",
                    "voicevox_onnxruntime-win-x64-1.23.2.tgz"
                ),
                size: 5_414_881,
                sha256: "1bf9635d1c0ffd4e7345e0f628cecbabde65782c73c5051d52a52e7edf7ce11c",
                dir: "voicevox_onnxruntime-win-x64-1.23.2",
            },
            "lib/voicevox_onnxruntime.dll",
        ))
    } else if cfg!(all(target_os = "android", target_arch = "aarch64")) {
        Some((
            Archive {
                url: concat!(
                    "https://github.com/VOICEVOX/onnxruntime-builder/releases/download/voicevox_onnxruntime-1.23.2/",
                    "voicevox_onnxruntime-android-arm64-1.23.2.tgz"
                ),
                size: 6_459_115,
                sha256: "43e6fc2a89ea6412d4cc20215042a3b34da22fdf05c1bb76256a686f82378d46",
                dir: "voicevox_onnxruntime-android-arm64-1.23.2",
            },
            "lib/libvoicevox_onnxruntime.so",
        ))
    } else if cfg!(all(target_os = "android", target_arch = "x86_64")) {
        Some((
            Archive {
                url: concat!(
                    "https://github.com/VOICEVOX/onnxruntime-builder/releases/download/voicevox_onnxruntime-1.23.2/",
                    "voicevox_onnxruntime-android-x64-1.23.2.tgz"
                ),
                size: 7_115_144,
                sha256: "807e2ea50dfb2a309416adb1cee6edc53d3d8eae1d702ad4e6f199a0f91f2d24",
                dir: "voicevox_onnxruntime-android-x64-1.23.2",
            },
            "lib/libvoicevox_onnxruntime.so",
        ))
    } else {
        None
    }
}

/// Open JTalk dictionary (text analysis). The release has no published hash;
/// this one was computed from the v1.11.1 asset.
pub const DICT: Archive = Archive {
    url: "https://github.com/r9y9/open_jtalk/releases/download/v1.11.1/open_jtalk_dic_utf_8-1.11.tar.gz",
    size: 23_646_843,
    sha256: "fe6ba0e43542cef98339abdffd903e062008ea170b04e7e2a35da805902f382a",
    dir: "open_jtalk_dic_utf_8-1.11",
};

fn core_err(e: impl std::fmt::Display) -> Error {
    Error::Core(e.to_string())
}

pub struct EmbeddedTts {
    synth: Synthesizer<OpenJtalk>,
    packs_dir: PathBuf,
    /// Packs loaded into the synthesizer so far (each is ~60 MB in memory).
    loaded: Mutex<HashSet<&'static str>>,
}

impl EmbeddedTts {
    /// Loads ONNX Runtime and the dictionary (about a second). Voice packs are
    /// loaded from `packs_dir` on first use of one of their styles.
    pub fn load(ort_lib: &Path, dict_dir: &Path, packs_dir: &Path) -> Result<Self> {
        let ort = Onnxruntime::load_once()
            .filename(ort_lib)
            .perform()
            .map_err(core_err)?;
        let dict = dict_dir.to_str().ok_or_else(|| Error::Core("caminho do dicionário inválido".into()))?;
        let synth = Synthesizer::builder(ort)
            .text_analyzer(OpenJtalk::new(dict).map_err(core_err)?)
            .build()
            .map_err(core_err)?;
        Ok(Self { synth, packs_dir: packs_dir.to_path_buf(), loaded: Mutex::new(HashSet::new()) })
    }

    /// WAV bytes for `text` in voice style `style` at `speed` (1.0 = normal).
    pub fn synthesize(&self, text: &str, style: u32, speed: f32) -> Result<Vec<u8>> {
        let info = crate::style(style).ok_or_else(|| Error::Core(format!("voz {style} desconhecida")))?;
        self.ensure_pack(info.pack)?;
        let id = StyleId(style);
        let mut query = self.synth.create_audio_query(text, id).map_err(core_err)?;
        query.speed_scale = PositiveFinite::<f32>::new(speed).map_err(|_| Error::Core(format!("velocidade inválida: {speed}")))?;
        self.synth.synthesis(&query, id).perform().map_err(core_err)
    }

    fn ensure_pack(&self, pack: &'static str) -> Result<()> {
        let mut loaded = self.loaded.lock().unwrap();
        if loaded.contains(pack) {
            return Ok(());
        }
        let path = self.packs_dir.join(pack);
        if !path.exists() {
            return Err(Error::Core(format!("baixe o pacote de vozes {pack} nas configurações")));
        }
        let model = VoiceModelFile::open(path.as_path()).map_err(core_err)?;
        self.synth.load_voice_model(&model).perform().map_err(core_err)?;
        loaded.insert(pack);
        Ok(())
    }
}
