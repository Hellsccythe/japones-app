//! Ties the crates together: settings, example lookup/generation, cards,
//! audio and exports. Used by both the CLI and the desktop app.
//!
//! Locks are never held across an `.await`, so the async methods are `Send`
//! and can run as Tauri commands.

mod settings;
mod setup;
mod vocab;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};

use anyhow::{bail, Context, Result};
use local_llm::LocalLlm;
use tts::embedded::EmbeddedTts;
use jp_core::{Analyzer, Segment, Token};
use serde::{Deserialize, Serialize};

pub use settings::{PosColors, Settings};
pub use setup::{RecommendedSetup, SetupItem};
pub use vocab::VocabEntry;
pub use sentences::{Example, Source};

/// An example sentence plus everything the UI needs to show it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExampleView {
    #[serde(flatten)]
    pub example: Example,
    pub segments: Vec<Segment>,
    pub html: String,
    /// Morphemes with word class, so the UI can color the sentence.
    pub tokens: Vec<Token>,
    /// False when an LLM sentence doesn't actually contain the word
    /// (per morphological analysis). Shown as a warning, not dropped.
    pub contains_word: bool,
}

/// A card as chosen in the UI; furigana and audio are filled in on export.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CardInput {
    pub word: String,
    pub sentence: String,
    #[serde(default)]
    pub translation: String,
    #[serde(default)]
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WordInfo {
    pub word: String,
    pub reading: String,
    pub entries: Vec<dict::Entry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Status {
    pub tatoeba_sentences: i64,
    pub tatoeba_indexed: i64,
    pub dictionary_loaded: bool,
    /// The selected sentence generator can be used right now.
    pub llm_ready: bool,
    /// Its name, e.g. "Gemma 4 E4B" or "Claude".
    pub llm_name: String,
    /// Embedded voice installed with the selected style's pack (always true
    /// for the HTTP backend, which can only be checked by calling it).
    pub voice_ready: bool,
    /// Selected voice, e.g. "ずんだもん (ノーマル)".
    pub voice_name: String,
}

/// A catalog model plus its state on this device.
#[derive(Debug, Clone, Serialize)]
pub struct ModelView {
    #[serde(flatten)]
    pub info: local_llm::ModelInfo,
    pub downloaded: bool,
    /// Bytes of an unfinished download (it resumes from here).
    pub partial_bytes: u64,
}

/// A voice pack plus its state on this device.
#[derive(Debug, Clone, Serialize)]
pub struct VoicePackView {
    #[serde(flatten)]
    pub pack: tts::VoicePack,
    pub downloaded: bool,
    pub partial_bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct VoiceStatus {
    /// There's a VOICEVOX ONNX Runtime build for this platform.
    pub available: bool,
    /// ONNX Runtime and the dictionary are installed.
    pub core_installed: bool,
    pub terms_accepted: bool,
    pub packs: Vec<VoicePackView>,
    /// Credits for every character in the downloaded packs.
    pub credits: Vec<tts::Character>,
}

pub struct Engine {
    analyzer: Analyzer,
    store: Mutex<sentences::Store>,
    dict: Mutex<dict::Dict>,
    settings: RwLock<Settings>,
    settings_path: PathBuf,
    models_dir: PathBuf,
    /// Local model kept in memory after first use, with its catalog id.
    local: Mutex<Option<(String, Arc<LocalLlm>)>>,
    /// ONNX Runtime, dictionary and voice packs for the embedded voice.
    voice_dir: PathBuf,
    /// Synthesized WAVs by text + voice + speed, shared by the player and the cards.
    audio_cache_dir: PathBuf,
    embedded_tts: Mutex<Option<Arc<EmbeddedTts>>>,
    /// Decks and reviews (`study.db`).
    srs: Mutex<srs::Srs>,
}

/// The card being studied, with the furigana for its back side.
#[derive(Debug, Clone, Serialize)]
pub struct StudyView {
    #[serde(flatten)]
    pub card: srs::StudyCard,
    pub furigana_html: String,
    /// The sentence's morphemes with word class, to color it.
    pub tokens: Vec<Token>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImportReport {
    pub found: usize,
    pub added: usize,
}

impl Engine {
    /// Opens (or creates) the data directory: `settings.json` and `japones.db`.
    pub fn open(data_dir: &Path) -> Result<Self> {
        std::fs::create_dir_all(data_dir)?;
        let settings_path = data_dir.join("settings.json");
        let settings = Settings::load(&settings_path)?;
        let db = data_dir.join("japones.db");
        Ok(Self {
            analyzer: Analyzer::new()?,
            store: Mutex::new(sentences::Store::open(&db)?),
            dict: Mutex::new(dict::Dict::open(&db)?),
            settings: RwLock::new(settings),
            settings_path,
            models_dir: data_dir.join("models"),
            local: Mutex::new(None),
            voice_dir: data_dir.join("voice"),
            audio_cache_dir: data_dir.join("audio-cache"),
            embedded_tts: Mutex::new(None),
            srs: Mutex::new(srs::Srs::open(&data_dir.join("study.db"))?),
        })
    }

    pub fn analyzer(&self) -> &Analyzer {
        &self.analyzer
    }

    pub fn settings(&self) -> Settings {
        self.settings.read().unwrap().clone()
    }

    pub fn save_settings(&self, s: Settings) -> Result<()> {
        s.save(&self.settings_path)?;
        *self.settings.write().unwrap() = s;
        Ok(())
    }

    pub fn status(&self) -> Result<Status> {
        let store = self.store.lock().unwrap();
        Ok(Status {
            tatoeba_sentences: store.japanese_count()?,
            tatoeba_indexed: store.indexed_count()?,
            dictionary_loaded: !self.dict.lock().unwrap().is_empty()?,
            llm_ready: self.llm_ready(),
            llm_name: self.llm_name(),
            voice_ready: self.voice_ready(),
            voice_name: self.voice_name(),
        })
    }

    fn voice_ready(&self) -> bool {
        let s = self.settings();
        s.uses_voicevox_http()
            || (self.voice_core_installed()
                && tts::style(s.speaker).is_some_and(|st| self.packs_dir().join(st.pack).exists()))
    }

    fn voice_name(&self) -> String {
        let id = self.settings().speaker;
        tts::style(id).map_or_else(|| format!("voz {id}"), |s| format!("{} ({})", s.character, s.name))
    }

    fn llm_ready(&self) -> bool {
        let s = self.settings();
        if s.llm_off() {
            false
        } else if s.uses_claude() {
            s.api_key().is_some()
        } else {
            local_llm::find(&s.local_model).is_some_and(|m| self.model_path(m).exists())
        }
    }

    fn llm_name(&self) -> String {
        let s = self.settings();
        if s.llm_off() {
            "desligada".into()
        } else if s.uses_claude() {
            "Claude".into()
        } else {
            local_llm::find(&s.local_model).map_or("IA local".into(), |m| m.name.to_string())
        }
    }

    // ---- data import ----

    pub async fn import_tatoeba(&self, log: impl Fn(String)) -> Result<()> {
        let langs = self.settings().translation_langs;
        sentences::tatoeba::download_and_import(&self.store, &self.analyzer, &langs, log).await?;
        Ok(())
    }

    pub async fn import_dictionary(&self) -> Result<usize> {
        // Download outside the lock, then import.
        let dir = self.settings_path.parent().unwrap().to_path_buf();
        let mut tmp = dict::Dict::open(&dir.join("japones.db"))?;
        Ok(tmp.download_and_import().await?)
    }

    pub fn import_dictionary_file(&self, path: &Path) -> Result<usize> {
        Ok(self.dict.lock().unwrap().import_file(path)?)
    }

    // ---- lookup ----

    pub fn furigana(&self, text: &str) -> Result<Vec<Segment>> {
        Ok(self.analyzer.furigana(text)?)
    }

    pub fn word_info(&self, word: &str) -> Result<WordInfo> {
        let entries = self.dict.lock().unwrap().lookup(word)?;
        Ok(WordInfo {
            reading: entries
                .first()
                .and_then(|e| e.kana.first().cloned())
                .unwrap_or(self.analyzer.to_kana(word)?),
            word: word.to_string(),
            entries,
        })
    }

    /// Morphemes of `text` with furigana and word class.
    pub fn analyze(&self, text: &str) -> Result<Vec<Token>> {
        Ok(self.analyzer.analyze(text)?)
    }

    /// Word-by-word vocabulary of a sentence, with JMdict meanings in English
    /// (up to 3 glosses of the first sense; empty when the word isn't there).
    pub fn vocabulary(&self, text: &str) -> Result<Vec<VocabEntry>> {
        let tokens = self.analyzer.analyze(text)?;
        let dict = self.dict.lock().unwrap();
        let lookup = |base: &str, class| vocab::pick_gloss(&dict.lookup(base).ok()?, class);
        Ok(vocab::vocabulary(&tokens, lookup, |s| self.analyzer.to_kana(s).ok()))
    }

    /// Translates the vocabulary meanings into the configured language with the
    /// selected AI. Cached meanings are reused; only the rest go to the AI, and
    /// an entry it leaves untranslated keeps its original gloss.
    /// Errors `ia:off` (AI turned off) and `ia:not_ready` (no model / no key)
    /// let the UI open the quick AI setup.
    pub async fn translate_vocabulary(&self, entries: Vec<VocabEntry>) -> Result<Vec<VocabEntry>> {
        let s = self.settings();
        if s.llm_off() {
            bail!("ia:off");
        }
        if !self.llm_ready() {
            bail!("ia:not_ready");
        }
        let lang = s.translation_language_name.clone();
        let keys: Vec<_> = entries.iter().map(vocab::cache_key).collect();
        let cached = self.store.lock().unwrap().cached_glosses(&keys, &lang)?;
        let (entries, missing) = vocab::apply_cache(entries, &cached);
        if missing.is_empty() {
            return Ok(entries);
        }
        let words: Vec<(String, String)> =
            missing.iter().map(|&i| (entries[i].base.clone(), entries[i].gloss.clone())).collect();
        let translations = if s.uses_claude() {
            let key = s.api_key().context("ia:not_ready")?;
            sentences::LlmClient::new(key, Some(s.model.clone())).translate_vocabulary(&words, &lang).await?
        } else {
            let llm = self.local_llm().await?;
            let lang = lang.clone();
            tokio::task::spawn_blocking(move || llm.translate_vocabulary(&words, &lang)).await??
        };
        let (entries, rows) = vocab::merge_translations(entries, &missing, &translations);
        self.store.lock().unwrap().save_glosses(&rows, &lang)?;
        Ok(entries)
    }

    fn view(&self, example: Example, lemmas: &[String]) -> Result<ExampleView> {
        let segments = self.analyzer.furigana(&example.japanese)?;
        Ok(ExampleView {
            contains_word: sentences::contains_word(&self.analyzer, &example.japanese, lemmas)?,
            html: jp_core::render_html(&segments),
            tokens: self.analyzer.analyze(&example.japanese)?,
            segments,
            example,
        })
    }

    /// Corpus sentences only (fast, offline).
    pub fn corpus_examples(&self, word: &str, count: usize) -> Result<Vec<ExampleView>> {
        let langs = self.settings().translation_langs;
        let lemmas = sentences::word_lemmas(&self.analyzer, word)?;
        let found = self.store.lock().unwrap().find(&self.analyzer, word, &langs, count)?;
        found.into_iter().map(|e| self.view(e, &lemmas)).collect()
    }

    /// LLM-generated sentences, from the provider chosen in the settings.
    /// Sentences that are too short or don't contain the word are generated
    /// again once; if still missing, the flagged ones fill the gap (the UI
    /// shows a warning on them).
    pub async fn generated_examples(&self, word: &str, count: usize) -> Result<Vec<ExampleView>> {
        let lemmas = sentences::word_lemmas(&self.analyzer, word)?;
        let (mut good, mut flagged) = (Vec::new(), Vec::new());
        for _attempt in 0..2 {
            for e in self.generate(word, count - good.len()).await? {
                let v = self.view(e, &lemmas)?;
                let long_enough = v.example.japanese.chars().count() >= sentences::prompt::MIN_SENTENCE_CHARS;
                if v.contains_word && long_enough {
                    good.push(v);
                } else {
                    flagged.push(v);
                }
            }
            if good.len() >= count {
                break;
            }
        }
        good.truncate(count);
        let missing = count - good.len();
        good.extend(flagged.into_iter().take(missing));
        Ok(good)
    }

    async fn generate(&self, word: &str, count: usize) -> Result<Vec<Example>> {
        let s = self.settings();
        let req = sentences::GenerateRequest {
            word: word.to_string(),
            count,
            level: s.level.clone(),
            translation_language: s.translation_language_name.clone(),
        };
        if s.llm_off() {
            bail!("A IA está desligada nas configurações");
        }
        if s.uses_claude() {
            let key = s
                .api_key()
                .context("Configure a chave da API do Claude (ANTHROPIC_API_KEY) nas configurações")?;
            let client = sentences::LlmClient::new(key, Some(s.model.clone()));
            return Ok(client.generate(&req).await?);
        }
        let llm = self.local_llm().await?;
        tokio::task::spawn_blocking(move || llm.generate(&req)).await?
    }

    /// The selected local model, loaded on first use and kept in memory.
    async fn local_llm(&self) -> Result<Arc<LocalLlm>> {
        let s = self.settings();
        let info = local_llm::find(&s.local_model)
            .context("modelo local desconhecido; escolha outro nas configurações")?;
        if let Some((id, llm)) = self.local.lock().unwrap().as_ref() {
            if id == info.id {
                return Ok(llm.clone());
            }
        }
        let path = self.model_path(info);
        if !path.exists() {
            bail!("Baixe o modelo {} em Configurações → IA para gerar frases", info.name);
        }
        // Free the previous model before loading another (they're gigabytes).
        *self.local.lock().unwrap() = None;
        let threads = s.llm_threads;
        let llm = tokio::task::spawn_blocking(move || LocalLlm::load(&path, info.name, threads)).await??;
        let llm = Arc::new(llm);
        *self.local.lock().unwrap() = Some((info.id.to_string(), llm.clone()));
        Ok(llm)
    }

    /// Checks a Claude API key (the one typed, or the saved one when empty).
    pub async fn test_claude(&self, key: &str) -> Result<()> {
        let key = Some(key.trim().to_string())
            .filter(|k| !k.is_empty())
            .or_else(|| self.settings().api_key())
            .context("Digite a chave da API do Claude")?;
        Ok(sentences::LlmClient::new(key, None).check().await?)
    }

    /// What "Instalar o recomendado" downloads here: sentences, dictionary,
    /// a voice (0.vvm, plus the voice runtime the first time) and the local
    /// model that fits this device's memory.
    pub fn recommended_setup(&self) -> Result<RecommendedSetup> {
        let status = self.status()?;
        let model = local_llm::find(setup::pick_model(setup::total_ram(), cfg!(target_os = "android")))
            .context("modelo recomendado fora do catálogo")?;
        let mut items = vec![
            SetupItem { id: "tatoeba".into(), label: "Frases".into(), bytes: setup::TATOEBA_BYTES, done: status.tatoeba_indexed > 0 },
            SetupItem { id: "dict".into(), label: "Dicionário".into(), bytes: setup::DICT_BYTES, done: status.dictionary_loaded },
        ];
        if let (Some((ort, _)), Some(pack)) = (tts::embedded::ORT, tts::PACKS.iter().find(|p| p.file == "0.vvm")) {
            let core = if self.voice_core_installed() { 0 } else { ort.size + tts::embedded::DICT.size };
            let done = self.packs_dir().join(pack.file).exists();
            items.push(SetupItem { id: "voice".into(), label: "Voz".into(), bytes: pack.size + core, done });
        }
        items.push(SetupItem {
            id: "model".into(),
            label: format!("IA ({})", model.name),
            bytes: model.size,
            done: self.model_path(model).exists(),
        });
        Ok(RecommendedSetup {
            model_id: model.id.into(),
            model_name: model.name.into(),
            total_bytes: items.iter().filter(|i| !i.done).map(|i| i.bytes).sum(),
            items,
        })
    }

    // ---- local models ----

    fn model_path(&self, m: &local_llm::ModelInfo) -> PathBuf {
        self.models_dir.join(m.file)
    }

    pub fn models(&self) -> Vec<ModelView> {
        local_llm::MODELS
            .iter()
            .map(|m| {
                let path = self.model_path(m);
                let downloaded = path.exists();
                ModelView {
                    info: m.clone(),
                    downloaded,
                    partial_bytes: if downloaded { 0 } else { assets::downloaded_bytes(&path) },
                }
            })
            .collect()
    }

    /// Downloads (or resumes) a catalog model. `progress(done, total)` in bytes.
    pub async fn download_model(&self, id: &str, progress: impl Fn(u64, u64)) -> Result<()> {
        let m = local_llm::find(id).context("modelo desconhecido")?;
        let asset = assets::Asset { url: m.url, size: m.size, sha256: m.sha256 };
        assets::download(&asset, &self.model_path(m), progress).await
    }

    /// Pauses the model and voice downloads in progress; they fail with
    /// `download:paused` and resume from the partial file when started again.
    pub fn cancel_downloads(&self) {
        assets::cancel_downloads();
    }

    pub fn delete_model(&self, id: &str) -> Result<()> {
        let m = local_llm::find(id).context("modelo desconhecido")?;
        let mut local = self.local.lock().unwrap();
        if local.as_ref().is_some_and(|(loaded, _)| loaded == id) {
            *local = None;
        }
        assets::remove(&self.model_path(m))
    }

    /// Corpus first; tops up with the LLM when there aren't enough and it's ready.
    pub async fn examples(&self, word: &str, count: usize, use_llm: bool) -> Result<Vec<ExampleView>> {
        let mut out = self.corpus_examples(word, count)?;
        if use_llm && out.len() < count && self.llm_ready() {
            out.extend(self.generated_examples(word, count - out.len()).await?);
        }
        Ok(out)
    }

    // ---- audio ----

    /// WAV for `text` with the selected voice. Results are cached on disk.
    pub async fn speak(&self, text: &str) -> Result<Vec<u8>> {
        let s = self.settings();
        let key = assets::sha256_hex(format!("{}\0{}\0{}\0{text}", s.tts_backend, s.speaker, s.speed).as_bytes());
        let cached = self.audio_cache_dir.join(format!("{key}.wav"));
        if let Ok(wav) = std::fs::read(&cached) {
            return Ok(wav);
        }

        let wav = if s.uses_voicevox_http() {
            let vv = tts::Voicevox::new(Some(s.voicevox_url.clone()));
            vv.synthesize(text, tts::VoiceOptions { speaker: s.speaker, speed: s.speed }).await?
        } else {
            let synth = self.embedded_tts().await?;
            let text = text.to_string();
            tokio::task::spawn_blocking(move || synth.synthesize(&text, s.speaker, s.speed)).await??
        };

        std::fs::create_dir_all(&self.audio_cache_dir)?;
        std::fs::write(&cached, &wav)?;
        Ok(wav)
    }

    /// Voices to choose from: the downloaded packs, or the VOICEVOX engine's list.
    pub async fn speakers(&self) -> Result<Vec<tts::Speaker>> {
        let s = self.settings();
        if s.uses_voicevox_http() {
            return Ok(tts::Voicevox::new(Some(s.voicevox_url)).speakers().await?);
        }
        let downloaded: Vec<&str> =
            tts::PACKS.iter().filter(|p| self.packs_dir().join(p.file).exists()).map(|p| p.file).collect();
        Ok(tts::speakers_in(&downloaded))
    }

    /// The embedded synthesizer, loaded on first use and kept in memory.
    async fn embedded_tts(&self) -> Result<Arc<EmbeddedTts>> {
        if let Some(t) = self.embedded_tts.lock().unwrap().as_ref() {
            return Ok(t.clone());
        }
        let ort = self.ort_lib().context("Voz embutida não disponível nesta plataforma; use o VOICEVOX (HTTP)")?;
        if !self.voice_core_installed() {
            bail!("Instale uma voz em Configurações → Áudio");
        }
        let (dict, packs) = (self.dict_dir(), self.packs_dir());
        let t = Arc::new(tokio::task::spawn_blocking(move || EmbeddedTts::load(&ort, &dict, &packs)).await??);
        *self.embedded_tts.lock().unwrap() = Some(t.clone());
        Ok(t)
    }

    // ---- embedded voice assets ----

    fn ort_lib(&self) -> Option<PathBuf> {
        tts::embedded::ORT.map(|(a, lib)| self.voice_dir.join("ort").join(a.dir).join(lib))
    }

    fn dict_dir(&self) -> PathBuf {
        self.voice_dir.join("dict").join(tts::embedded::DICT.dir)
    }

    fn packs_dir(&self) -> PathBuf {
        self.voice_dir.join("vvms")
    }

    fn voice_core_installed(&self) -> bool {
        self.ort_lib().is_some_and(|p| p.exists()) && self.dict_dir().join("sys.dic").exists()
    }

    pub fn voice_status(&self) -> VoiceStatus {
        let packs: Vec<VoicePackView> = tts::PACKS
            .iter()
            .map(|p| {
                let path = self.packs_dir().join(p.file);
                let downloaded = path.exists();
                VoicePackView {
                    pack: p.clone(),
                    downloaded,
                    partial_bytes: if downloaded { 0 } else { assets::downloaded_bytes(&path) },
                }
            })
            .collect();
        let mut credits: Vec<tts::Character> = Vec::new();
        for name in packs.iter().filter(|p| p.downloaded).flat_map(|p| p.pack.characters) {
            if let Some(c) = tts::character(name).filter(|c| !credits.iter().any(|x| x.name == c.name)) {
                credits.push(c.clone());
            }
        }
        VoiceStatus {
            available: tts::embedded::ORT.is_some(),
            core_installed: self.voice_core_installed(),
            terms_accepted: self.settings().voice_terms_accepted,
            packs,
            credits,
        }
    }

    /// Downloads a voice pack, installing ONNX Runtime and the dictionary
    /// first if needed. `progress(item, done, total)` in bytes, where item is
    /// "onnxruntime", "dicionário" or the pack file.
    pub async fn download_voice_pack(&self, file: &str, progress: impl Fn(&str, u64, u64)) -> Result<()> {
        if !self.settings().voice_terms_accepted {
            bail!("Aceite os termos do VOICEVOX antes de baixar vozes");
        }
        let pack = tts::PACKS.iter().find(|p| p.file == file).context("pacote de vozes desconhecido")?;
        let (ort, lib) = tts::embedded::ORT.context("Voz embutida não disponível nesta plataforma")?;
        let ort_dir = self.voice_dir.join("ort");
        if !ort_dir.join(ort.dir).join(lib).exists() {
            self.install_archive(&ort, &ort_dir, |d, t| progress("onnxruntime", d, t)).await?;
        }
        if !self.dict_dir().join("sys.dic").exists() {
            let dict = tts::embedded::DICT;
            self.install_archive(&dict, &self.voice_dir.join("dict"), |d, t| progress("dicionário", d, t)).await?;
        }
        let asset = assets::Asset { url: pack.url, size: pack.size, sha256: pack.sha256 };
        assets::download(&asset, &self.packs_dir().join(pack.file), |d, t| progress(pack.file, d, t)).await
    }

    async fn install_archive(
        &self,
        a: &tts::embedded::Archive,
        into: &Path,
        progress: impl Fn(u64, u64),
    ) -> Result<()> {
        let tgz = into.join(format!("{}.tgz", a.dir));
        assets::download(&assets::Asset { url: a.url, size: a.size, sha256: a.sha256 }, &tgz, progress).await?;
        let (archive, dest) = (tgz.clone(), into.to_path_buf());
        tokio::task::spawn_blocking(move || assets::extract_tgz(&archive, &dest)).await??;
        std::fs::remove_file(&tgz)?;
        Ok(())
    }

    pub fn delete_voice_pack(&self, file: &str) -> Result<()> {
        let pack = tts::PACKS.iter().find(|p| p.file == file).context("pacote de vozes desconhecido")?;
        // The synthesizer may hold it; drop it and reload on next use.
        *self.embedded_tts.lock().unwrap() = None;
        assets::remove(&self.packs_dir().join(pack.file))
    }

    // ---- export ----

    async fn build_cards(&self, inputs: &[CardInput], with_audio: bool) -> Result<Vec<anki::Card>> {
        let mut cards = Vec::with_capacity(inputs.len());
        let credit = with_audio.then(|| tts::credit(self.settings().speaker));
        for c in inputs {
            let audio = if with_audio { Some(self.speak(&c.sentence).await?) } else { None };
            cards.push(anki::Card {
                word: c.word.clone(),
                sentence_furigana: self.analyzer.to_anki(&c.sentence)?,
                sentence: c.sentence.clone(),
                translation: c.translation.clone(),
                note: c.note.clone(),
                audio,
                audio_credit: credit.clone(),
                schedule: None,
            });
        }
        Ok(cards)
    }

    /// Sends cards to a running Anki. Returns how many were added.
    pub async fn export_anki_connect(&self, inputs: &[CardInput], with_audio: bool) -> Result<usize> {
        let s = self.settings();
        let client = anki::AnkiConnect::new(Some(s.anki_connect_url.clone()));
        // Fail before generating audio, which is the slow part.
        client.check().await?;
        let cards = self.build_cards(inputs, with_audio).await?;
        Ok(client.add_cards(&s.deck, &cards, &tags()).await?)
    }

    /// Checks the AnkiConnect connection. Returns the add-on version.
    pub async fn anki_test(&self) -> Result<u32> {
        let client = anki::AnkiConnect::new(Some(self.settings().anki_connect_url.clone()));
        Ok(client.check().await?)
    }

    // ---- study (decks inside the app) ----

    pub fn decks(&self) -> Result<Vec<srs::DeckView>> {
        Ok(self.srs.lock().unwrap().decks()?)
    }

    /// Creates the deck if needed and returns its id.
    pub fn create_deck(&self, name: &str) -> Result<i64> {
        Ok(self.srs.lock().unwrap().deck_id(name)?)
    }

    pub fn update_deck(&self, deck: &srs::Deck) -> Result<()> {
        Ok(self.srs.lock().unwrap().update_deck(deck)?)
    }

    pub fn delete_deck(&self, id: i64) -> Result<()> {
        Ok(self.srs.lock().unwrap().delete_deck(id)?)
    }

    /// Adds the cards to a deck (by name, created if missing). Returns how many were new.
    pub fn add_to_deck(&self, deck: &str, inputs: &[CardInput]) -> Result<usize> {
        let notes: Vec<srs::NoteInput> = inputs
            .iter()
            .map(|c| srs::NoteInput {
                word: c.word.clone(),
                sentence: c.sentence.clone(),
                translation: c.translation.clone(),
                note: c.note.clone(),
            })
            .collect();
        let mut srs = self.srs.lock().unwrap();
        let id = srs.deck_id(deck)?;
        Ok(srs.add_notes(id, &notes)?)
    }

    pub fn next_card(&self, deck_id: i64) -> Result<Option<StudyView>> {
        let card = self.srs.lock().unwrap().next_card(deck_id)?;
        card.map(|card| {
            Ok(StudyView {
                furigana_html: self.analyzer.to_html(&card.note.sentence)?,
                tokens: self.analyzer.analyze(&card.note.sentence)?,
                card,
            })
        })
        .transpose()
    }

    pub fn answer_card(&self, card_id: i64, rating: u8, took_ms: i64) -> Result<()> {
        let rating = srs::Rating::from_u8(rating).context("resposta inválida")?;
        Ok(self.srs.lock().unwrap().answer(card_id, rating, took_ms)?)
    }

    /// Copies all decks and review history to one file (for backup or another device).
    pub fn backup_study(&self, path: &Path) -> Result<()> {
        Ok(self.srs.lock().unwrap().backup_to(path)?)
    }

    /// Replaces all decks and review history with a backup.
    pub fn restore_study(&self, path: &Path) -> Result<()> {
        Ok(self.srs.lock().unwrap().restore_from(path)?)
    }

    /// Imports the notes of an Anki `.apkg` into a deck (created if missing).
    pub fn import_apkg(&self, path: &Path, deck: &str) -> Result<ImportReport> {
        let notes = srs::import_apkg(path)?;
        let mut srs = self.srs.lock().unwrap();
        let id = srs.deck_id(deck)?;
        Ok(ImportReport { found: notes.len(), added: srs.add_notes(id, &notes)? })
    }

    /// Exports a deck's notes as an Anki `.apkg` (without review history).
    /// Exports a study deck with each card's progress and review history, so
    /// studying continues in Anki where it stopped here.
    pub async fn export_deck_apkg(&self, deck_id: i64, path: &Path, with_audio: bool) -> Result<usize> {
        let (name, notes) = {
            let srs = self.srs.lock().unwrap();
            (srs.deck(deck_id)?.name, srs.export(deck_id)?)
        };
        let inputs: Vec<CardInput> = notes
            .iter()
            .map(|n| CardInput {
                word: n.note.word.clone(),
                sentence: n.note.sentence.clone(),
                translation: n.note.translation.clone(),
                note: n.note.note.clone(),
            })
            .collect();
        let mut cards = self.build_cards(&inputs, with_audio).await?;
        for (card, n) in cards.iter_mut().zip(&notes) {
            card.schedule = Some(anki_schedule(n));
        }
        anki::apkg::write(path, &name, &cards, &tags(), srs::day_start(now_secs()))?;
        Ok(cards.len())
    }

    pub async fn export_apkg(&self, path: &Path, inputs: &[CardInput], with_audio: bool) -> Result<()> {
        let cards = self.build_cards(inputs, with_audio).await?;
        anki::apkg::write(path, &self.settings().deck, &cards, &tags(), srs::day_start(now_secs()))?;
        Ok(())
    }

    /// PDF grouped by word, in the order the cards were given.
    pub fn export_pdf(&self, path: &Path, title: &str, inputs: &[CardInput], furigana: bool) -> Result<()> {
        let mut sections: Vec<pdf::Section> = Vec::new();
        for c in inputs {
            let entry = pdf::Entry {
                segments: self.analyzer.furigana(&c.sentence)?,
                translation: Some(c.translation.clone()).filter(|s| !s.is_empty()),
                note: Some(c.note.clone()).filter(|s| !s.is_empty()),
            };
            match sections.iter_mut().find(|s| s.heading == c.word) {
                Some(s) => s.entries.push(entry),
                None => sections.push(pdf::Section { heading: c.word.clone(), entries: vec![entry] }),
            }
        }
        let bytes = pdf::render(&pdf::Document { title: title.to_string(), sections, furigana })?;
        std::fs::write(path, bytes)?;
        Ok(())
    }

    /// PDF of free text (one entry per non-empty line).
    pub fn export_text_pdf(&self, path: &Path, title: &str, text: &str, furigana: bool) -> Result<()> {
        let entries = text
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| {
                Ok(pdf::Entry { segments: self.analyzer.furigana(l.trim())?, translation: None, note: None })
            })
            .collect::<Result<Vec<_>>>()?;
        let doc = pdf::Document {
            title: title.to_string(),
            sections: vec![pdf::Section { heading: title.to_string(), entries }],
            furigana,
        };
        std::fs::write(path, pdf::render(&doc)?)?;
        Ok(())
    }
}

fn tags() -> Vec<String> {
    vec!["japones-app".into()]
}

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or_default()
}

/// Error text for the UI: AnkiConnect failures become a code the UI turns
/// into a friendly message (`anki:not_running`, `anki:no_addon`,
/// `anki:incompatible:<v>`); anything else is its plain message.
pub fn error_text(e: &anyhow::Error) -> String {
    match e.downcast_ref::<anki::Error>() {
        Some(anki::Error::NotRunning) => "anki:not_running".into(),
        Some(anki::Error::NoAddon) => "anki:no_addon".into(),
        Some(anki::Error::Incompatible(v)) => format!("anki:incompatible:{v}"),
        _ => e.to_string(),
    }
}

fn anki_kind(state: srs::State) -> anki::CardKind {
    match state {
        srs::State::New => anki::CardKind::New,
        srs::State::Learning => anki::CardKind::Learning,
        srs::State::Review => anki::CardKind::Review,
        srs::State::Relearning => anki::CardKind::Relearning,
    }
}

fn anki_schedule(n: &srs::ExportedNote) -> anki::Schedule {
    anki::Schedule {
        kind: anki_kind(n.state),
        due: n.due,
        last_review: n.last_review,
        stability: n.stability,
        difficulty: n.difficulty,
        reps: n.reps,
        lapses: n.lapses,
        reviews: n
            .reviews
            .iter()
            .map(|r| anki::Review {
                time_ms: r.time_ms,
                ease: r.rating,
                kind: anki_kind(r.state_before),
                interval_secs: r.interval_secs,
                took_ms: r.took_ms,
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anki_errors_become_codes() {
        assert_eq!(error_text(&anki::Error::NotRunning.into()), "anki:not_running");
        assert_eq!(error_text(&anki::Error::NoAddon.into()), "anki:no_addon");
        assert_eq!(error_text(&anki::Error::Incompatible(5).into()), "anki:incompatible:5");
        assert_eq!(error_text(&anyhow::anyhow!("outro")), "outro");
    }
}
