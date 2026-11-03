//! Command-line front end, handy for testing and batch work.

use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};
use engine::{CardInput, Engine, ExampleView};

#[derive(Parser)]
#[command(name = "jp", about = "Frases de exemplo, furigana, Anki, PDF e áudio para japonês")]
struct Cli {
    /// Data directory (settings.json + japones.db).
    #[arg(long, env = "JAPONES_APP_DATA", default_value = "data")]
    data: PathBuf,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Show the text with furigana (Anki syntax and HTML).
    Furigana { text: String },
    /// Download Tatoeba sentences and build the lemma index.
    ImportTatoeba,
    /// Import JMdict (downloads the latest release when no file is given).
    ImportDict { file: Option<PathBuf> },
    /// Look up a word in the dictionary.
    Lookup { word: String },
    /// List example sentences for a word.
    Examples {
        word: String,
        #[arg(short, default_value_t = 5)]
        n: usize,
        /// Top up with AI-generated sentences when the corpus has too few.
        #[arg(long)]
        llm: bool,
    },
    /// Generate example sentences with the AI chosen in the settings (local or Claude).
    Generate {
        word: String,
        #[arg(short, default_value_t = 3)]
        n: usize,
    },
    /// List the local AI models and whether they're downloaded.
    Models,
    /// Download (or resume) a local AI model by id.
    DownloadModel { id: String },
    /// List the study decks with today's counts (new / learning / review).
    Decks,
    /// Add example sentences for words to a study deck.
    AddDeck {
        deck: String,
        words: Vec<String>,
        #[arg(short, default_value_t = 5)]
        n: usize,
        #[arg(long)]
        llm: bool,
    },
    /// Import the notes of an Anki .apkg into a study deck.
    ImportApkg { file: PathBuf, deck: String },
    /// Export a study deck, with its review history, as an Anki .apkg.
    ExportApkg {
        deck: String,
        file: PathBuf,
        #[arg(long)]
        audio: bool,
    },
    /// Save all decks and review history to one file.
    Backup { file: PathBuf },
    /// Replace all decks and review history with a backup.
    Restore { file: PathBuf },
    /// List the embedded voice packs and whether they're downloaded.
    Voices,
    /// Download an embedded voice pack (e.g. 0.vvm), plus ONNX Runtime and the dictionary.
    DownloadVoice {
        file: String,
        /// Accept the VOICEVOX ONNX Runtime and voice model terms
        /// (https://github.com/VOICEVOX/voicevox_vvm).
        #[arg(long)]
        accept_terms: bool,
    },
    /// Synthesize speech with VOICEVOX into a WAV file.
    Speak {
        text: String,
        #[arg(short, default_value = "out.wav")]
        o: PathBuf,
    },
    /// Make Anki cards for words (AnkiConnect, or a .apkg with --apkg).
    Anki {
        words: Vec<String>,
        #[arg(short, default_value_t = 5)]
        n: usize,
        #[arg(long)]
        llm: bool,
        #[arg(long)]
        audio: bool,
        #[arg(long)]
        apkg: Option<PathBuf>,
    },
    /// Make a PDF with furigana of example sentences for words.
    Pdf {
        words: Vec<String>,
        #[arg(short, default_value_t = 5)]
        n: usize,
        #[arg(long)]
        llm: bool,
        #[arg(short, default_value = "frases.pdf")]
        o: PathBuf,
        #[arg(long)]
        no_furigana: bool,
    },
    /// Make a PDF with furigana from a text file (one paragraph per line).
    TextPdf {
        file: PathBuf,
        #[arg(short, default_value = "texto.pdf")]
        o: PathBuf,
        #[arg(long)]
        no_furigana: bool,
    },
}

fn print_example(i: usize, e: &ExampleView) {
    let src = match &e.example.source {
        engine::Source::Tatoeba { id } => format!("tatoeba #{id}"),
        engine::Source::Llm { model } => model.clone(),
    };
    let warn = if e.contains_word { "" } else { "  ⚠ não contém a palavra" };
    println!("{}. {}  [{}]{}", i + 1, e.example.japanese, src, warn);
    println!("   {}", jp_core::render_anki(&e.segments).trim());
    if let Some(t) = &e.example.translation {
        println!("   {t}");
    }
    if let Some(n) = &e.example.note {
        println!("   ({n})");
    }
}

async fn cards_for(engine: &Engine, words: &[String], n: usize, llm: bool) -> Result<Vec<CardInput>> {
    let mut cards = Vec::new();
    for w in words {
        let examples = engine.examples(w, n, llm).await?;
        if examples.is_empty() {
            eprintln!("Nenhuma frase para {w} (importe o Tatoeba ou use --llm).");
        }
        cards.extend(examples.into_iter().map(|e| CardInput {
            word: w.clone(),
            sentence: e.example.japanese,
            translation: e.example.translation.unwrap_or_default(),
            note: e.example.note.unwrap_or_default(),
        }));
    }
    Ok(cards)
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let engine = Engine::open(&cli.data)?;

    match cli.cmd {
        Cmd::Furigana { text } => {
            println!("{}", engine.analyzer().to_anki(&text)?);
            println!("{}", engine.analyzer().to_html(&text)?);
        }
        Cmd::ImportTatoeba => engine.import_tatoeba(|m| println!("{m}")).await?,
        Cmd::ImportDict { file } => {
            let n = match file {
                Some(f) => engine.import_dictionary_file(&f)?,
                None => engine.import_dictionary().await?,
            };
            println!("{n} entradas importadas.");
        }
        Cmd::Lookup { word } => println!("{}", serde_json::to_string_pretty(&engine.word_info(&word)?)?),
        Cmd::Examples { word, n, llm } => {
            for (i, e) in engine.examples(&word, n, llm).await?.iter().enumerate() {
                print_example(i, e);
            }
        }
        Cmd::Generate { word, n } => {
            let t = std::time::Instant::now();
            for (i, e) in engine.generated_examples(&word, n).await?.iter().enumerate() {
                print_example(i, e);
            }
            eprintln!("({:.1}s)", t.elapsed().as_secs_f64());
        }
        Cmd::Models => {
            let active = engine.settings().local_model;
            for m in engine.models() {
                let state = if m.downloaded {
                    "baixado".to_string()
                } else if m.partial_bytes > 0 {
                    format!("{}% baixado", m.partial_bytes * 100 / m.info.size)
                } else {
                    "não baixado".to_string()
                };
                let mark = if m.info.id == active { "*" } else { " " };
                println!(
                    "{mark} {:<12} {:<12} {:>5.1} GB  {:<26} {state}",
                    m.info.id,
                    m.info.name,
                    m.info.size as f64 / 1e9,
                    m.info.recommended_for
                );
            }
        }
        Cmd::DownloadModel { id } => {
            engine
                .download_model(&id, |done, total| {
                    eprint!("\r{:.1} / {:.1} GB ({}%)   ", done as f64 / 1e9, total as f64 / 1e9, done * 100 / total)
                })
                .await?;
            eprintln!("\nModelo {id} pronto.");
        }
        Cmd::Decks => {
            for d in engine.decks()? {
                let c = d.counts;
                println!(
                    "{:<24} novos {:>3}  aprendendo {:>3}  revisão {:>3}  ({} cards)",
                    d.deck.name, c.new, c.learning, c.review, c.total
                );
            }
        }
        Cmd::AddDeck { deck, words, n, llm } => {
            let cards = cards_for(&engine, &words, n, llm).await?;
            let added = engine.add_to_deck(&deck, &cards)?;
            println!("{added} de {} cards adicionados a {deck}.", cards.len());
        }
        Cmd::ImportApkg { file, deck } => {
            let r = engine.import_apkg(&file, &deck)?;
            println!("{} notas no arquivo, {} novas em {deck}.", r.found, r.added);
        }
        Cmd::ExportApkg { deck, file, audio } => {
            let id = engine
                .decks()?
                .into_iter()
                .find(|d| d.deck.name == deck)
                .map(|d| d.deck.id)
                .ok_or_else(|| anyhow::anyhow!("deck {deck} não encontrado"))?;
            let n = engine.export_deck_apkg(id, &file, audio).await?;
            println!("{n} cards salvos em {}", file.display());
        }
        Cmd::Backup { file } => {
            engine.backup_study(&file)?;
            println!("Backup salvo em {}", file.display());
        }
        Cmd::Restore { file } => {
            engine.restore_study(&file)?;
            println!("Decks restaurados de {}", file.display());
        }
        Cmd::Voices => {
            let st = engine.voice_status();
            println!(
                "voz embutida: {}; onnxruntime + dicionário: {}; termos aceitos: {}",
                if st.available { "disponível" } else { "indisponível nesta plataforma" },
                if st.core_installed { "instalados" } else { "não instalados" },
                if st.terms_accepted { "sim" } else { "não" },
            );
            for p in st.packs {
                let state = if p.downloaded {
                    "baixado".to_string()
                } else if p.partial_bytes > 0 {
                    format!("{}%", p.partial_bytes * 100 / p.pack.size)
                } else {
                    String::new()
                };
                println!("{:<7} {:>3} MB  {:<9} {}", p.pack.file, p.pack.size / 1_000_000, state, p.pack.characters.join(", "));
            }
        }
        Cmd::DownloadVoice { file, accept_terms } => {
            if accept_terms {
                let mut s = engine.settings();
                s.voice_terms_accepted = true;
                engine.save_settings(s)?;
            }
            engine
                .download_voice_pack(&file, |item, done, total| {
                    eprint!("\r{item}: {:.1} / {:.1} MB   ", done as f64 / 1e6, total as f64 / 1e6)
                })
                .await?;
            eprintln!("\nVozes de {file} prontas.");
        }
        Cmd::Speak { text, o } => {
            std::fs::write(&o, engine.speak(&text).await?)?;
            println!("Salvo em {}", o.display());
        }
        Cmd::Anki { words, n, llm, audio, apkg } => {
            let cards = cards_for(&engine, &words, n, llm).await?;
            match apkg {
                Some(path) => {
                    engine.export_apkg(&path, &cards, audio).await?;
                    println!("{} cards salvos em {}", cards.len(), path.display());
                }
                None => {
                    let added = engine.export_anki_connect(&cards, audio).await?;
                    println!("{added} de {} cards adicionados ao Anki.", cards.len());
                }
            }
        }
        Cmd::Pdf { words, n, llm, o, no_furigana } => {
            let cards = cards_for(&engine, &words, n, llm).await?;
            engine.export_pdf(&o, &words.join("・"), &cards, !no_furigana)?;
            println!("PDF salvo em {}", o.display());
        }
        Cmd::TextPdf { file, o, no_furigana } => {
            let text = std::fs::read_to_string(&file)?;
            let title = file.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            engine.export_text_pdf(&o, &title, &text, !no_furigana)?;
            println!("PDF salvo em {}", o.display());
        }
    }
    Ok(())
}
