# Japonês App

App em Rust (Tauri 2), para Windows e Android, para estudar japonês a partir de palavras:

- **Frases de exemplo**: busca frases reais no corpus [Tatoeba](https://tatoeba.org) (encontra qualquer conjugação: buscar `食べる` acha `食べました`, `食べたい`...) e completa com frases geradas por IA quando faltar: uma **IA local** (Gemma 4 via llama.cpp, offline) ou, online e opcional, o Claude ou qualquer modelo do [OpenRouter](https://openrouter.ai).
- **Estudo dentro do app**: decks com revisão espaçada (FSRS, o mesmo algoritmo do Anki), frente e verso com furigana e áudio, limites diários, importação de `.apkg` e backup para levar os decks entre PC e celular. Não precisa do Anki.
- **Anki**: envia direto para o Anki aberto (AnkiConnect) ou salva um `.apkg`. Os cards têm furigana (`{{furigana:}}`), tradução, nota e áudio opcional.
- **PDF com furigana**: das frases selecionadas ou de um texto livre, com Typst.
- **Áudio**: vozes VOICEVOX embutidas no app (VOICEVOX CORE, offline) ou o programa VOICEVOX aberto, para ouvir frases e textos e gravar o áudio nos cards.
- **Detalhamento da frase**: cada frase mostra o vocabulário palavra a palavra (forma de dicionário, leitura e significado do JMdict). **Traduzir com IA** passa os significados para o seu idioma (com cache) e **Copiar** gera um texto simples com a frase, a tradução e o vocabulário. Na aba Texto, cada linha tem o mesmo detalhamento, e a IA traduz também a frase.
- **Cores por classe de palavra**: substantivos, verbos, partículas, adjetivos/advérbios e auxiliares com cores diferentes na frase (Frases, Estudar e Texto). As cores mudam clicando na legenda, separadas para o tema claro e o escuro, e podem ser desligadas.
- **Temas claro e escuro** (ou o do sistema), tamanho ajustável do japonês e navegação lateral no PC / barra inferior no celular.
- **Dicionário**: JMdict (significados e tipo do verbo).
- **Anki com teste de conexão**: em Configurações › Anki, **Testar conexão** diz se o Anki está aberto e se falta o add-on AnkiConnect (código `2055492159`), com o passo a passo; sempre dá para salvar um `.apkg`.
- **PC e Android**: a mesma interface se ajusta à tela do celular. Na primeira vez, **Instalar o recomendado** baixa frases, dicionário, uma voz e o modelo de IA certo para a memória do aparelho, com uma barra só e pausa; ou use o Claude em vez de baixar a IA. Se faltar algo depois, o app abre uma configuração rápida daquele item.

## Baixar

Na página de [Releases](../../releases) do GitHub:

- **Windows**: `Japones-App_<versão>_windows-x64-setup.exe`. O instalador não tem assinatura de código paga, então o Windows pode avisar "O Windows protegeu o computador": clique em *Mais informações › Executar assim mesmo*.
- **Android**: a partir da versão 0.2.0, ainda sem APK nas releases; para usar agora, compile seguindo as instruções abaixo.

Na primeira vez, o app mostra o que baixar (frases, dicionário, voz e IA) e o tamanho de cada item.

## Compilar

### Requisitos

| O quê | Para quê | Como |
|---|---|---|
| Rust (stable) | compilar | <https://rustup.rs> |
| CMake + libclang | compilar o llama.cpp (IA local) | `pip install cmake libclang`; aponte `LIBCLANG_PATH` para a pasta `clang/native` do pacote `libclang` se o build não achar |
| WebView2 | janela do app (já vem no Windows 11) | — |
| Vozes VOICEVOX | áudio | Baixadas pelo app (Configurações → Áudio), depois de aceitar os termos. Alternativa: abrir o [VOICEVOX](https://voicevox.hiroshiba.jp) e escolher "Programa VOICEVOX" |
| Anki + [AnkiConnect](https://ankiweb.net/shared/info/2055492159) | enviar cards direto | No Anki: Ferramentas › Complementos › Obter → código `2055492159` |
| Modelo de IA local | gerar frases offline | Baixado pelo app (Configurações → IA): Gemma 4 E4B (5,2 GB) ou E2B (3,3 GB) |
| Chave da API do Claude | gerar frases online (opcional) | Variável `ANTHROPIC_API_KEY` ou campo nas Configurações |
| Chave do OpenRouter | gerar frases online com outros modelos (opcional) | Variável `OPENROUTER_API_KEY` ou campo nas Configurações |

Todos são opcionais: sem vozes não há áudio, sem Anki use o `.apkg`, sem modelo de IA use só o corpus.

### Rodando

```bash
cargo run -p japones-app
```

Na primeira vez, abra **Configurações** e clique em:

1. **Baixar e indexar Tatoeba**: baixa ~35 MB e indexa ~250 mil frases (cerca de 2 min).
2. **Baixar dicionário (JMdict)**: baixa ~12 MB.
3. Em **IA**, baixe um modelo (o download continua de onde parou e o arquivo é conferido por SHA-256).

Os dados ficam na pasta de dados do app (`%APPDATA%\com.japonesapp.desktop`). Para usar outra pasta, defina `JAPONES_APP_DATA`.

Para gerar o instalador (`target/release/bundle/nsis/`), com o Tauri CLI fixado em `app/package.json`:

```bash
cd app && npm ci && npm run tauri -- build
```

O APK de Android sai de `npm run tauri -- android build --apk --target aarch64` e precisa do JDK 17, do Android SDK e do NDK 28.2 (`NDK_HOME`). O CMake das dependências em C++ não acha o compilador do NDK sozinho: defina `CC_aarch64_linux_android`, `CXX_aarch64_linux_android` e `AR_aarch64_linux_android` com os executáveis de `toolchains/llvm/prebuilt/<sistema>/bin` (veja o passo *Build APK* do workflow de release). No Windows, a CLI do Tauri cria links simbólicos e pede o Modo de Desenvolvedor; sem ele, [`spikes/rebuild-arm64-release.sh`](spikes/rebuild-arm64-release.sh) faz o mesmo build copiando a biblioteca (ajuste os caminhos no começo do script). Sem `app/gen/android/keystore.properties` o APK sai sem assinatura.

## Publicar uma versão

O workflow [`.github/workflows/release.yml`](.github/workflows/release.yml) gera o instalador do Windows e cria a release no GitHub. O APK assinado só entra quando a variável do repositório `BUILD_ANDROID` vale `true` (Settings › Secrets and variables › Actions › Variables).

1. **Uma vez só, a chave de assinatura do Android.** Guarde o arquivo e a senha fora do repositório: sem eles não dá para publicar atualizações que instalem por cima.

   ```bash
   keytool -genkeypair -keystore japones-release.jks -storetype PKCS12 -alias japones -keyalg RSA -keysize 2048 -validity 10000
   ```

   No GitHub, em *Settings › Secrets and variables › Actions*, crie:
   - `ANDROID_KEYSTORE_BASE64`: o resultado de `base64 -w0 japones-release.jks`;
   - `ANDROID_KEYSTORE_PASSWORD`: a senha;
   - `ANDROID_KEY_ALIAS`: `japones`.
2. Suba a versão em `Cargo.toml` (`[workspace.package]`) e em `app/tauri.conf.json`, e escreva a seção `## [x.y.z]` no [CHANGELOG.md](CHANGELOG.md).
3. Crie e envie a tag:

   ```bash
   git tag v0.1.0 && git push origin v0.1.0
   ```

O workflow confere se a tag, as duas versões e o CHANGELOG batem, compila e publica. Para testar o build sem publicar, rode o workflow à mão em *Actions › Release › Run workflow*. O [`ci.yml`](.github/workflows/ci.yml) roda os testes a cada push.


## CLI

A mesma funcionalidade existe em linha de comando, o que ajuda em testes e em lotes. A CLI usa `./data` por padrão.

```bash
cargo run -p jp-cli -- import-tatoeba
cargo run -p jp-cli -- import-dict
cargo run -p jp-cli -- models                              # modelos de IA local
cargo run -p jp-cli -- download-model gemma-4-e4b
cargo run -p jp-cli -- generate 食べる -n 3                # só IA
cargo run -p jp-cli -- examples 食べる -n 5 --llm
cargo run -p jp-cli -- furigana "日本語を勉強しています。"
cargo run -p jp-cli -- anki 食べる 行く --audio            # via AnkiConnect
cargo run -p jp-cli -- anki 食べる --apkg frases.apkg      # arquivo
cargo run -p jp-cli -- pdf 食べる 行く -o frases.pdf
cargo run -p jp-cli -- text-pdf texto.txt -o texto.pdf
cargo run -p jp-cli -- decks                               # decks de estudo e contadores de hoje
cargo run -p jp-cli -- add-deck "Japonês::Verbos" 食べる 行く --llm
cargo run -p jp-cli -- import-apkg deck.apkg "Core 2k"
cargo run -p jp-cli -- export-apkg "Core 2k" deck.apkg    # com progresso e histórico; --audio inclui áudio
cargo run -p jp-cli -- backup estudo.db                    # restore estudo.db faz o caminho inverso
cargo run -p jp-cli -- voices                              # pacotes de vozes
cargo run -p jp-cli -- download-voice 0.vvm --accept-terms
cargo run -p jp-cli -- speak "こんにちは" -o oi.wav
```

## Estrutura

```
crates/
  jp-core/    análise morfológica (lindera + IPADIC), lemas, alinhamento de furigana
  sentences/  corpus Tatoeba em SQLite com índice por lema, prompt/schema das frases geradas, clientes do Claude e do OpenRouter
  local-llm/  IA local: llama.cpp + gramática JSON, catálogo de modelos
  assets/     downloads grandes com retomada e SHA-256
  dict/       JMdict (jmdict-simplified) em SQLite
  srs/        decks, cards e revisões (FSRS) em study.db, importação de .apkg, backup
  anki/       AnkiConnect e .apkg (gravador próprio, com agendamento e histórico), mesmo tipo de nota "Japones App"
  pdf/        PDF com ruby/furigana via Typst, com a fonte Noto Serif JP embutida (subset)
  tts/        VOICEVOX CORE embutido + catálogo de vozes (gerado) + cliente HTTP do VOICEVOX
  engine/     orquestração + configurações (usado pela CLI e pelo app)
cli/          binário `jp`
app/          Tauri 2: src/main.rs (comandos) + ui/ (HTML/CSS/JS sem bundler)
```

Testes: `cargo test --workspace`. Página de licenças das dependências: `cargo about generate app/licenses.hbs -o app/ui/licenses.html` (precisa de `cargo install cargo-about --features cli`). Prévia do layout do PDF: `cargo run -p pdf --example preview -- prev.png`.

## Como funciona

- **Busca por lema**: na importação, cada frase do Tatoeba passa pelo lindera e os lemas (forma de dicionário) são indexados. A busca analisa a palavra digitada da mesma forma. Palavras compostas (`勉強する`) precisam aparecer em sequência.
- **Ordenação**: primeiro as frases com tradução nos idiomas preferidos (padrão `por`, depois `eng`), depois as de tamanho próximo de 16 caracteres.
- **Frases geradas**: o mesmo prompt e o mesmo JSON Schema servem à IA local e ao Claude. Na IA local, o schema vira uma gramática do llama.cpp, então a resposta é sempre JSON válido. Cada frase é conferida pelo analisador: se a palavra não aparece ou a frase é curta demais, ela é gerada de novo uma vez; se ainda faltar, a frase aparece com um aviso ⚠. O modelo local fica na memória depois do primeiro uso (o primeiro pedido demora alguns segundos a mais).
- **Claude**: saída em JSON estruturado (`output_config.format`). Modelo padrão: `claude-opus-5-5` com `effort: low`. Nos modelos com classificadores de segurança, o fallback do servidor (`fallbacks: "default"`) fica ligado, para que uma recusa seja refeita em outro modelo.
- **Estudo**: cada frase vira uma nota com um card (frase → furigana, tradução e áudio). Cards novos passam pelos passos de 1min / 6min / 10min e depois seguem os intervalos do FSRS com retenção desejada de 90%. Ao errar uma revisão, o card volta por 10min. O dia vira às 4h. A fila mostra primeiro os cards em aprendizado que já venceram, depois as revisões do dia e por fim os novos, respeitando os limites do deck. Tudo fica em `study.db`, separado do corpus.
- **Furigana**: a leitura do IPADIC é alinhada só sobre os kanji (`食[た]べる`, `お 茶[ちゃ]`).

## Limitações conhecidas

- O IPADIC escolhe uma leitura por contexto e às vezes erra palavras ambíguas (`今日`/こんにち, `何`/なに·なん, `方`/かた·ほう). Ainda não há edição manual da leitura.
- O Tatoeba tem poucas traduções diretas para português, então a maioria vem em inglês. As traduções podem ser editadas antes de exportar.
- O áudio é gerado na exportação (ou vem do cache), então exportar com áudio exige uma voz instalada ou o VOICEVOX aberto.
- Importar `.apkg` começa do zero (o histórico do arquivo não é lido) e não lê o formato novo `anki21b` (no Anki, exporte com "Suporte a versões antigas do Anki"). Exportar um deck leva o progresso e o histórico.

## Licença

O código é distribuído sob **MIT OR Apache-2.0**, à sua escolha ([LICENSE-MIT](LICENSE-MIT), [LICENSE-APACHE](LICENSE-APACHE)). Os componentes de terceiros e os seus avisos estão em [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md) e, dentro do app, em Configurações › Sobre.

## Licenças dos dados

- Tatoeba: CC BY 2.0 FR. As frases exportadas mantêm o ID `tatoeba #N`.
- JMdict: © EDRDG, CC BY-SA 4.0.
- Noto Serif JP (embutida nos PDFs, em subset): SIL Open Font License 1.1, ver `crates/pdf/fonts/OFL.txt`.
- VOICEVOX: o ONNX Runtime da VOICEVOX e os modelos de voz permitem uso comercial e redistribuição embutida, proíbem engenharia reversa e exigem crédito. Cada voz tem seus termos (ex.: `VOICEVOX:ずんだもん`); algumas são só não comerciais. A tela de Áudio lista o crédito e os termos de cada voz baixada, e o app põe o crédito nos cards.
