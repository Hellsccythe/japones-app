use std::path::{Path, PathBuf};

use base64::Engine as _;
use engine::{CardInput, Engine, ExampleView, ImportReport, ModelView, Settings, Status, StudyView, VoiceStatus, WordInfo};
use jp_core::Segment;
use serde::Serialize;
use tauri::{Emitter, Manager, State};
use tauri_plugin_fs::{FilePath, FsExt, OpenOptions};

type CmdResult<T> = Result<T, String>;

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

#[derive(Serialize)]
struct Furigana {
    segments: Vec<Segment>,
    html: String,
}

#[tauri::command]
fn status(engine: State<'_, Engine>) -> CmdResult<Status> {
    engine.status().map_err(err)
}

#[tauri::command]
fn get_settings(engine: State<'_, Engine>) -> Settings {
    engine.settings()
}

#[tauri::command]
fn save_settings(engine: State<'_, Engine>, settings: Settings) -> CmdResult<()> {
    engine.save_settings(settings).map_err(err)
}

#[tauri::command]
fn furigana(engine: State<'_, Engine>, text: String) -> CmdResult<Furigana> {
    let segments = engine.furigana(&text).map_err(err)?;
    Ok(Furigana { html: jp_core::render_html(&segments), segments })
}

#[tauri::command]
fn word_info(engine: State<'_, Engine>, word: String) -> CmdResult<WordInfo> {
    engine.word_info(&word).map_err(err)
}

#[tauri::command]
fn corpus_examples(engine: State<'_, Engine>, word: String, count: usize) -> CmdResult<Vec<ExampleView>> {
    engine.corpus_examples(&word, count).map_err(err)
}

#[tauri::command]
async fn generated_examples(engine: State<'_, Engine>, word: String, count: usize) -> CmdResult<Vec<ExampleView>> {
    engine.generated_examples(&word, count).await.map_err(err)
}

/// WAV audio as base64, played by the frontend through a data URL.
#[tauri::command]
async fn speak(engine: State<'_, Engine>, text: String) -> CmdResult<String> {
    let wav = engine.speak(&text).await.map_err(err)?;
    Ok(base64::engine::general_purpose::STANDARD.encode(wav))
}

#[tauri::command]
async fn speakers(engine: State<'_, Engine>) -> CmdResult<Vec<tts::Speaker>> {
    engine.speakers().await.map_err(err)
}

// ---- files picked in the dialog ----
//
// On Android the file picker returns `content://` URIs, which `std::fs` can't
// open. The engine works on real paths, so those files go through a temporary
// copy in the cache dir.

/// A real path for a picked file; the temporary copy is removed on drop.
struct Local {
    path: PathBuf,
    temp: bool,
}

impl Drop for Local {
    fn drop(&mut self) {
        if self.temp {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

fn local_path(path: &FilePath) -> Option<PathBuf> {
    match path {
        FilePath::Path(p) => Some(p.clone()),
        FilePath::Url(u) if u.scheme() == "file" => u.to_file_path().ok(),
        FilePath::Url(_) => None,
    }
}

fn temp_file(app: &tauri::AppHandle) -> anyhow::Result<Local> {
    let dir = app.path().app_cache_dir()?.join("picked");
    std::fs::create_dir_all(&dir)?;
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_nanos();
    Ok(Local { path: dir.join(format!("{nanos}.tmp")), temp: true })
}

/// A file to read from.
fn input(app: &tauri::AppHandle, path: &FilePath) -> anyhow::Result<Local> {
    if let Some(p) = local_path(path) {
        return Ok(Local { path: p, temp: false });
    }
    let local = temp_file(app)?;
    let mut opts = OpenOptions::new();
    opts.read(true);
    let mut src = app.fs().open(path.clone(), opts)?;
    std::io::copy(&mut src, &mut std::fs::File::create(&local.path)?)?;
    Ok(local)
}

/// Runs `write` on a real path, then copies the result to `path` if it's a URI.
async fn output<T, F>(app: &tauri::AppHandle, path: &FilePath, write: impl FnOnce(PathBuf) -> F) -> CmdResult<T>
where
    F: std::future::Future<Output = CmdResult<T>>,
{
    let local = match local_path(path) {
        Some(p) => Local { path: p, temp: false },
        None => temp_file(app).map_err(err)?,
    };
    let out = write(local.path.clone()).await?;
    if local.temp {
        copy_to(app, &local.path, path).map_err(err)?;
    }
    Ok(out)
}

fn copy_to(app: &tauri::AppHandle, from: &Path, to: &FilePath) -> anyhow::Result<()> {
    let mut opts = OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    let mut dst = app.fs().open(to.clone(), opts)?;
    std::io::copy(&mut std::fs::File::open(from)?, &mut dst)?;
    Ok(())
}

#[tauri::command]
async fn export_anki_connect(engine: State<'_, Engine>, cards: Vec<CardInput>, with_audio: bool) -> CmdResult<usize> {
    engine.export_anki_connect(&cards, with_audio).await.map_err(err)
}

#[tauri::command]
async fn export_apkg(app: tauri::AppHandle, engine: State<'_, Engine>, path: FilePath, cards: Vec<CardInput>, with_audio: bool) -> CmdResult<()> {
    output(&app, &path, |p| async move { engine.export_apkg(&p, &cards, with_audio).await.map_err(err) }).await
}

/// Typst compilation is CPU-bound (and scans system fonts), so it runs on a
/// blocking thread instead of the UI or async runtime threads.
async fn blocking<T: Send + 'static>(
    app: tauri::AppHandle,
    f: impl FnOnce(&Engine) -> anyhow::Result<T> + Send + 'static,
) -> CmdResult<T> {
    tauri::async_runtime::spawn_blocking(move || f(&app.state::<Engine>()))
        .await
        .map_err(err)?
        .map_err(err)
}

#[tauri::command]
async fn export_pdf(app: tauri::AppHandle, path: FilePath, title: String, cards: Vec<CardInput>, furigana: bool) -> CmdResult<()> {
    let handle = app.clone();
    output(&app, &path, |p| blocking(handle, move |e| e.export_pdf(&p, &title, &cards, furigana))).await
}

#[tauri::command]
async fn export_text_pdf(app: tauri::AppHandle, path: FilePath, title: String, text: String, furigana: bool) -> CmdResult<()> {
    let handle = app.clone();
    output(&app, &path, |p| blocking(handle, move |e| e.export_text_pdf(&p, &title, &text, furigana))).await
}

#[tauri::command]
async fn import_tatoeba(app: tauri::AppHandle, engine: State<'_, Engine>) -> CmdResult<()> {
    engine
        .import_tatoeba(move |msg| {
            let _ = app.emit("import-log", msg);
        })
        .await
        .map_err(err)
}

#[tauri::command]
async fn import_dictionary(engine: State<'_, Engine>) -> CmdResult<usize> {
    engine.import_dictionary().await.map_err(err)
}

#[tauri::command]
fn models(engine: State<'_, Engine>) -> Vec<ModelView> {
    engine.models()
}

#[derive(Clone, Serialize)]
struct DownloadProgress {
    id: String,
    done: u64,
    total: u64,
}

/// Progress arrives as `model-progress` events.
#[tauri::command]
async fn download_model(app: tauri::AppHandle, engine: State<'_, Engine>, id: String) -> CmdResult<()> {
    let event_id = id.clone();
    engine
        .download_model(&id, move |done, total| {
            let _ = app.emit("model-progress", DownloadProgress { id: event_id.clone(), done, total });
        })
        .await
        .map_err(err)
}

#[tauri::command]
fn delete_model(engine: State<'_, Engine>, id: String) -> CmdResult<()> {
    engine.delete_model(&id).map_err(err)
}

#[tauri::command]
fn voice_status(engine: State<'_, Engine>) -> VoiceStatus {
    engine.voice_status()
}

#[derive(Clone, Serialize)]
struct VoiceProgress {
    pack: String,
    /// What's downloading: "onnxruntime", "dicionário" or the pack file.
    item: String,
    done: u64,
    total: u64,
}

/// Progress arrives as `voice-progress` events.
#[tauri::command]
async fn download_voice_pack(app: tauri::AppHandle, engine: State<'_, Engine>, file: String) -> CmdResult<()> {
    let pack = file.clone();
    engine
        .download_voice_pack(&file, move |item, done, total| {
            let _ = app.emit("voice-progress", VoiceProgress { pack: pack.clone(), item: item.to_string(), done, total });
        })
        .await
        .map_err(err)
}

#[tauri::command]
fn delete_voice_pack(engine: State<'_, Engine>, file: String) -> CmdResult<()> {
    engine.delete_voice_pack(&file).map_err(err)
}

// ---- study ----

#[tauri::command]
fn decks(engine: State<'_, Engine>) -> CmdResult<Vec<srs::DeckView>> {
    engine.decks().map_err(err)
}

#[tauri::command]
fn create_deck(engine: State<'_, Engine>, name: String) -> CmdResult<i64> {
    engine.create_deck(&name).map_err(err)
}

#[tauri::command]
fn update_deck(engine: State<'_, Engine>, deck: srs::Deck) -> CmdResult<()> {
    engine.update_deck(&deck).map_err(err)
}

#[tauri::command]
fn delete_deck(engine: State<'_, Engine>, id: i64) -> CmdResult<()> {
    engine.delete_deck(id).map_err(err)
}

#[tauri::command]
fn add_to_deck(engine: State<'_, Engine>, deck: String, cards: Vec<CardInput>) -> CmdResult<usize> {
    engine.add_to_deck(&deck, &cards).map_err(err)
}

#[tauri::command]
fn next_card(engine: State<'_, Engine>, deck_id: i64) -> CmdResult<Option<StudyView>> {
    engine.next_card(deck_id).map_err(err)
}

#[tauri::command]
fn answer_card(engine: State<'_, Engine>, card_id: i64, rating: u8, took_ms: i64) -> CmdResult<()> {
    engine.answer_card(card_id, rating, took_ms).map_err(err)
}

#[tauri::command]
async fn backup_study(app: tauri::AppHandle, engine: State<'_, Engine>, path: FilePath) -> CmdResult<()> {
    output(&app, &path, |p| async move { engine.backup_study(&p).map_err(err) }).await
}

#[tauri::command]
fn restore_study(app: tauri::AppHandle, engine: State<'_, Engine>, path: FilePath) -> CmdResult<()> {
    let local = input(&app, &path).map_err(err)?;
    engine.restore_study(&local.path).map_err(err)
}

#[tauri::command]
fn import_apkg(app: tauri::AppHandle, engine: State<'_, Engine>, path: FilePath, deck: String) -> CmdResult<ImportReport> {
    let local = input(&app, &path).map_err(err)?;
    engine.import_apkg(&local.path, &deck).map_err(err)
}

#[tauri::command]
async fn export_deck_apkg(app: tauri::AppHandle, engine: State<'_, Engine>, deck_id: i64, path: FilePath, with_audio: bool) -> CmdResult<usize> {
    output(&app, &path, |p| async move { engine.export_deck_apkg(deck_id, &p, with_audio).await.map_err(err) }).await
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let data_dir = std::env::var_os("JAPONES_APP_DATA")
                .map(PathBuf::from)
                .unwrap_or(app.path().app_data_dir()?);
            app.manage(Engine::open(&data_dir)?);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            status,
            get_settings,
            save_settings,
            furigana,
            word_info,
            corpus_examples,
            generated_examples,
            speak,
            speakers,
            export_anki_connect,
            export_apkg,
            export_pdf,
            export_text_pdf,
            import_tatoeba,
            import_dictionary,
            models,
            download_model,
            delete_model,
            voice_status,
            download_voice_pack,
            delete_voice_pack,
            decks,
            create_deck,
            update_deck,
            delete_deck,
            add_to_deck,
            next_card,
            answer_card,
            backup_study,
            restore_study,
            import_apkg,
            export_deck_apkg,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
