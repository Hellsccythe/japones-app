# Plano de ação: app standalone (Windows .exe + Android .apk)

Objetivo: um único app que gera frases, faz furigana/PDF, fala as frases e
**revisa os cards sozinho** — sem precisar abrir Anki, VOICEVOX, Ollama nem ter
chave de API. Distribuído pelo GitHub Releases.

---

## 1. Decisões de licença

| Componente | Licença | Pode embutir? | Decisão |
|---|---|---|---|
| **Anki** (desktop e backend Rust `rslib`) | AGPL-3.0+ | Sim, mas o app inteiro vira AGPL-3.0 e o código precisa ser público. Não pode usar o nome/logo "Anki". | **Não embutir.** Fazer revisão própria com FSRS (abaixo). Manter export `.apkg` e AnkiConnect como opcionais. |
| **AnkiDroid** | GPL-3.0 | Mesmo raciocínio (viraria GPL). | Não usar. |
| **fsrs-rs** (algoritmo de repetição espaçada que o Anki usa hoje) | BSD-3-Clause | Sim, sem restrição. | **Usar** para o agendamento dos cards. |
| **VOICEVOX CORE** ≥ 0.16 (biblioteca Rust) | MIT | Sim. | **Usar direto** (sem o VOICEVOX ENGINE/editor). Builds < 0.16 têm outra licença: fixar ≥ 0.16. |
| **Modelos de voz VOICEVOX** (VVM) | Termos próprios | Sim: o termo permite embutir em aplicativo e redistribuir. Exige crédito ("VOICEVOX:ずんだもん") e alguns personagens têm regras extras (uso só pessoal/não comercial, consulta para empresas). | Baixar no primeiro uso, mostrar tela de créditos, oferecer só vozes cujos termos permitam o uso do app. |
| ONNX Runtime usado pelo VOICEVOX CORE | a confirmar | Provável (MIT upstream), mas o VOICEVOX distribui um build próprio. | **Verificar no spike** (fase 0). |
| **llama.cpp** / crate `llama-cpp-2` | MIT / Apache-2.0 | Sim. | **Usar** para a IA local. |
| Modelo de IA (GGUF) | depende do modelo | Ver seção 2. | Baixar no primeiro uso, não colocar no instalador. |
| Tatoeba / JMdict | CC BY 2.0 FR / CC BY-SA 4.0 | Sim, com atribuição. | Já baixados em tempo de execução. Créditos na tela "Sobre". |
| Fonte japonesa para PDF (Noto Sans/Serif JP) | OFL 1.1 | Sim. | **Embutir** (Android não tem Yu Gothic). |

**Licença do nosso app:** sem código do Anki fica livre escolher. Sugestão: MIT ou Apache-2.0.
Se um dia quiser sincronizar com o AnkiWeb usando `rslib`, a licença muda para AGPL-3.0. Isso dá
para fazer porque o projeto vai ser público. Os termos do AnkiWeb para clientes de terceiros
precisam ser verificados antes.

---

## 2. IA local

**Por que não Ollama:** é um programa separado que o usuário teria que instalar e deixar rodando,
o que contraria o "standalone". Ele também não roda no Android. O caminho é embutir o **llama.cpp**
(o mesmo motor que o Ollama usa por dentro) e baixar o modelo **GGUF do Hugging Face**.

**Candidatos** (avaliar na fase 0; tamanhos aproximados em Q4):

| Modelo | Tamanho | Licença | Onde |
|---|---|---|---|
| Gemma 4 E4B | 5,2 GB (QAT q4_0) | Apache-2.0 | PC padrão (escolhido na fase 0) |
| Gemma 4 E2B | 3,3 GB (QAT q4_0) | Apache-2.0 | celular (escolhido na fase 0) |
| Qwen3.5-4B | ~2,7 GB | Apache-2.0 | descartado na fase 0 |
| LLM-jp-4 (focado em japonês) | MoE grande | Apache-2.0 | só PC forte, opcional |
| Nemotron Nano 9B v2 Japanese | ~5,5 GB | confirmar | PC com 16 GB, opcional |

**Como garantir frases boas com modelo pequeno:**
- Saída restrita a JSON por gramática: o llama.cpp converte JSON Schema em gramática GBNF, então o modelo não consegue produzir JSON inválido.
- A validação que já existe continua valendo: o lindera confere se a palavra aparece na frase, e as que falham são descartadas e geradas de novo.
- O Tatoeba continua sendo a fonte principal. A IA só completa o que falta.
- O Claude continua como opção "online" nas configurações, para quem quiser mais qualidade.

**Avaliação objetiva:** uma lista de 30 palavras (N5 a N2). Para cada modelo, medir:
1. % de frases que contêm a palavra;
2. % de JSON válido;
3. tempo por frase no PC e no celular;
4. revisão manual de naturalidade em 10 frases.

Escolher o padrão de cada plataforma com base nisso.

---

## 3. Arquitetura alvo

```
engine
 ├─ SentenceSource:   Tatoeba | LocalLlm (llama.cpp) | Claude (opcional)
 ├─ TtsBackend:       VoicevoxCore (embutido) | VoicevoxHttp (opcional)
 ├─ Srs:              decks, notas, cards, revisões (SQLite) + fsrs-rs
 ├─ AssetManager:     download com progresso/retomada + SHA-256 de modelos (LLM, VVM, ONNX), dicionário e corpus
 └─ Export:           .apkg | AnkiConnect (opcional) | PDF
app (Tauri 2) → mesmo código Rust para Windows e Android, UI responsiva
```

O que já existe e não muda: `jp-core`, `sentences::store`, `dict`, `pdf` (só ganha fonte embutida),
`anki::apkg`.

---

## 4. Fases

### Fase 0: spikes de risco (antes de construir)
Cada item gera um "funciona / não funciona" com números.

- [x] Compilar o app atual para Android (`tauri android init` / `build`), com lindera, SQLite e Typst. Medir o tamanho do APK.
- [x] `voicevox_core` ≥ 0.16 em Rust no Windows **e** em Android (testado no emulador x86_64; arm64 compila igual, falta medir em aparelho real): sintetizar uma frase. Confirmar a licença do ONNX Runtime distribuído e qual downloader/URL usar para os VVM.
- [x] `llama-cpp-2` no Windows (CPU) e no Android (NDK, CPU, emulador): gerar frases em JSON com gramática. Medir tokens/s. Vulkan ficou de fora (precisa do Vulkan SDK para compilar).
- [x] Rodar a avaliação da seção 2 com 4 modelos (resultados abaixo).
- [ ] Verificar o salvamento de arquivos no Android (PDF, `.apkg`): plugin dialog/fs ou compartilhamento. **Não feito**, vai para a fase 4.

#### Resultados da fase 0 (2026-11-02)

Código dos testes em `spikes/` (`voicevox/`, `llm/`, scripts de build do APK).

**Android / APK**
- O app atual compila e abre no Android (emulador API 35, x86_64). A UI carrega, mas o cabeçalho fica espremido em tela estreita, então a fase 4 (UI responsiva) é necessária.
- Tamanho: `.so` release com 119 MB, 96 MB após `strip` (57 MB são dados: dicionário IPADIC do lindera + fontes do Typst; 27 MB de código), ~29 MB comprimido. APK debug ~127 MB por arquitetura; release com strip deve ficar em ~30 MB.
- Para reduzir: carregar o IPADIC de arquivo baixado em vez de `embed-ipadic` e não embutir as fontes padrão do Typst (só Noto JP em subset).
- Pegadinhas de build no Windows:
  - `tauri android build` cria um *symlink* da `.so` e falha sem o **Modo de Desenvolvedor do Windows**. Alternativa: `cargo build --target ...`, copiar para `jniLibs/` e rodar `gradlew assemble<Arch>Debug -x rustBuild<Arch>Debug` (`spikes/gradle-apk.sh`). No CI (Linux) isso não acontece.
  - A `.so` precisa da feature `tauri/custom-protocol` (agora é default no `app/Cargo.toml`); sem ela o app tenta abrir o servidor de desenvolvimento.
  - Gradle/Java 17 com "Unable to establish loopback connection": apontar `TEMP`/`TMP` e `-Djdk.net.unixdomain.tmpdir` para uma pasta sem espaços/caracteres especiais.
  - NDK 28 + CMake: usar o gerador Ninja e não definir `CMAKE_TOOLCHAIN_FILE` (o crate `cmake` detecta o NDK). O bindgen do llama.cpp precisa de `libclang` (`pip install libclang` + `LIBCLANG_PATH`).

**VOICEVOX CORE 0.17.1**
- Funciona em Rust no Windows e no Android. Windows: init 1,4 s, 0,44 s por frase. Emulador (CPU disputada): init 2,5 s, 2,5 s por frase; WAV idêntico ao do PC.
- Os VVM oficiais estão no formato "vv-bin" e **só funcionam com o ONNX Runtime da VOICEVOX** (`voicevox_onnxruntime`). O ORT da Microsoft (MIT) recusa esses modelos.
- Termos do ORT da VOICEVOX: uso comercial, embutir e redistribuir permitidos; crédito obrigatório; proibida engenharia reversa. Termos dos VVM: crédito por voz. Os dois ficam fora do APK/instalador e são baixados no primeiro uso (fase 2).
- Tamanhos: ORT ~14 MB (Windows), `0.vvm` 59 MB, dicionário Open JTalk 103 MB.

**llama.cpp (`llama-cpp-2` 0.1.160)**
- Funciona no Windows e no emulador Android. No emulador deu ~1 tok/s, mas com CPU disputada; o número real só sai de um celular de verdade.
- Gramática JSON (`json_schema_to_grammar`): aplicada a todo o vocabulário custa caro (8 tok/s). Otimização usada: amostrar sem gramática, checar só o token escolhido e, se for rejeitado, amostrar de novo com a gramática completa. Resultado: 26 tok/s no 2B em CPU, com JSON 100% válido.

**Avaliação dos modelos** (30 palavras N5–N2, 3 frases cada, gramática JSON, CPU Ryzen 7 5700X com 8 threads; as velocidades dos Qwen foram medidas com a CPU disputada, então só a ordem relativa é confiável)

| Modelo | Arquivo | JSON válido | Frase contém a palavra | Velocidade | Qualidade (revisão manual) |
|---|---|---|---|---|---|
| Qwen3.5-2B Q4_K_M | 1,3 GB | 30/30 | 84% | 8,4 tok/s | Ruim: frases curtas demais (「行く」 sozinho), notas erradas |
| Qwen3.5-4B Q4_K_M | 2,7 GB | 30/30 | 87% | 3,8 tok/s | Média: japonês às vezes artificial, traduções erradas |
| **Gemma 4 E2B** QAT q4_0 | 3,3 GB | 30/30 | 93% | **22,6 tok/s** (~8 s/palavra) | Boa: japonês natural; notas às vezes em inglês |
| **Gemma 4 E4B** QAT q4_0 | 5,2 GB | 30/30 | **94%** | 12,1 tok/s (~14 s/palavra) | Boa: japonês natural, traduções corretas; notas gramaticais às vezes imprecisas |

**Decisão:** Gemma 4 E4B como padrão no PC; Gemma 4 E2B como padrão no celular e opção "rápida" no PC. Os Qwen saem da lista.

Achados para a fase 1:
- O llama.cpp não reconhece o template Jinja do Gemma 4 (`apply_chat_template` falha). Montar o prompt manualmente: `<bos><|turn>system\n…<turn|>\n<|turn>user\n…<turn|>\n<|turn>model\n`.
- Um download retomado com `curl -C -` corrompeu o GGUF do E2B (4,3 GB em vez de 3,3 GB) e o modelo gerou só tokens `<unused…>`. O `AssetManager` **precisa** conferir tamanho e SHA-256 (a API do Hugging Face informa os dois) antes de usar o arquivo.
- Validações a adicionar: tamanho mínimo da frase, nota obrigatoriamente em português (rejeitar ou gerar de novo se vier em inglês), e checar se a forma descrita na nota bate com a análise do lindera.
- Falta medir a velocidade num celular real (o emulador não serve como referência).

### Fase 1: IA local
- [x] Prompt e schema compartilhados (`sentences::prompt`) pelos dois provedores (Claude e local). Em vez de um trait, o `engine` escolhe o provedor pela configuração `llm_provider` (são só dois).
- [x] Crate `local-llm` (llama.cpp + gramática JSON). Carregar o modelo uma vez e mantê-lo em memória.
- [x] Crate `assets`: baixar o GGUF do Hugging Face com barra de progresso, retomada (`Range`, recomeça do zero se o servidor ignorar) e checagem de tamanho + SHA-256.
- [x] Tela de configurações para escolher o modelo (tamanho, licença, recomendado para este aparelho), baixar/continuar/apagar.
- [x] Validação: frase curta demais ou sem a palavra é gerada de novo uma vez; prompt exige tradução e nota no idioma escolhido.
- [ ] Pendências: medir velocidade num celular real; cancelar download em andamento; notas ainda escapam palavras de outro idioma de vez em quando (ex.: "bentuk"), avaliar checagem automática.

### Fase 2: voz embutida
- [x] Crate `tts` com backend embutido (`tts::embedded`, VOICEVOX CORE 0.17.1); o HTTP continua como opção ("Programa VOICEVOX aberto").
- [x] Baixar ONNX Runtime da VOICEVOX + dicionário Open JTalk + VVM no primeiro uso, só depois de o usuário aceitar os termos na tela. Tudo conferido por SHA-256.
- [x] Catálogo de vozes gerado dos próprios VVMs (`spikes/voicevox/gen_catalog.py`): 26 pacotes, 128 estilos, 44 personagens com crédito, link dos termos e restrições extras (não comercial, consulta prévia para empresas). Ficam de fora `s0.vvm` (canto) e `n0.vvm` (vozes Nemo, termos próprios).
- [x] Créditos das vozes baixadas na tela de Áudio; crédito automático no campo Audio do card (`.apkg` e AnkiConnect). O PDF não tem áudio, então não leva crédito.
- [x] Cache dos WAVs (texto + voz + velocidade), usado pelo player e pelos cards. Medido no PC: 3,0 s na primeira frase (inclui carregar), 0,03 s do cache.
- [x] Testado no Android (emulador x86_64, app real): baixa e extrai tudo em ~35 s, sintetiza em 4,4 s (0,007 s do cache), WAV idêntico ao do PC.
- [ ] Pendências: limpar o cache pela tela (hoje cresce sem limite, ~130 KB por frase); cancelar download.
- [ ] Risco Android: carregar a `.so` do ONNX Runtime baixada para a pasta de dados funciona, mas o Android registra `avc: granted { execute }` (W^X, prática que versões futuras podem bloquear). Plano B: empacotar o ONNX Runtime da VOICEVOX dentro do APK (`jniLibs`, ~7 MB), o que os termos permitem.

**Achado de build (vale para o CI):** desde a fase 1 o app inclui C++ (llama.cpp, VOICEVOX) e o APK precisa levar a `libc++_shared.so` do NDK em `jniLibs/<abi>/`; sem ela o app fecha ao abrir (`dlopen failed: library "libc++_shared.so" not found`). `spikes/rebuild-x86.sh` já copia; o script de arm64 e o CI precisam fazer o mesmo. Para inspecionar a UI no Android: compilar com `--features devtools` e usar `chrome://inspect`.

### Fase 3: revisão de cards dentro do app (substitui abrir o Anki)
- [x] Tabelas `decks`, `notes`, `cards`, `revlog` num banco separado (`study.db`, crate `srs`), para o backup levar só o estudo.
- [x] "Adicionar ao deck" na tela de frases, ao lado do export para o Anki.
- [x] Tela de estudo (aba "Estudar"): frente (frase e palavra), verso (furigana, tradução, nota, áudio automático) e botões Errei / Difícil / Bom / Fácil com o próximo intervalo, agendados pelo `fsrs` 6 (o mesmo do Anki, retenção 0,9). Atalhos: Espaço/Enter, 1–4, R (ouvir), Esc.
  - Passos de aprendizado como os do Anki: 1min / 6min / 10min e depois o intervalo do FSRS; reaprendizado de 10min.
  - O dia vira às 4h (horário local), como no Anki.
- [x] Contadores (novos / aprendendo / revisão) e limite diário por deck (padrão 20 novos e 200 revisões), editáveis na engrenagem do deck.
- [x] Importar `.apkg` (`collection.anki21`/`anki2`). O nosso tipo de nota é lido pelos campos; nos outros, o 1º campo vira a frase e o 2º a tradução, sem HTML e `[sound:]`.
  - O formato novo `anki21b` (Anki 2.1.50+ sem "Suporte a versões antigas") ainda não é lido: o app pede para exportar de novo com essa opção marcada.
  - O histórico de revisões do arquivo não é importado (os cards entram como novos).
- [x] Exportar um deck como `.apkg`, com áudio opcional, **com o progresso e o histórico de revisões**: cada card leva o estado (novo / aprendendo / revisão), a data de vencimento, o intervalo, o estado de memória do FSRS e o log de respostas, então o estudo continua no Anki de onde parou. O `.apkg` é gravado pelo próprio app (crate `anki`, esquema 11 do Anki), sem o `genanki-rs`. Conferido com o importador oficial do Anki 26.09: cards, agendamento, FSRS, histórico, áudio e reimportação sem duplicar.
- [x] Backup e restauração do estudo num arquivo `.db` (API de backup do SQLite, seguro com o app aberto). Serve para passar os decks do PC para o celular e vice-versa.
- CLI: `decks`, `add-deck`, `import-apkg`, `backup`, `restore`.

### Fase 4: mobile e polimento
- [x] UI responsiva: abaixo de 600px o cabeçalho quebra em duas linhas, as abas viram uma faixa rolável e os botões têm 40px de altura. Na tela de estudo, os botões de resposta ficam presos embaixo (60px), ao alcance do polegar. Respeita a área segura (`viewport-fit=cover`).
  - No Android ficam escondidos o "Enviar ao Anki" (AnkiConnect) e a voz por HTTP, que só existem no PC.
- [x] Arquivos escolhidos no Android: o seletor devolve URIs `content://`, que o Rust não abre direto. Os comandos recebem um `FilePath` e passam por uma cópia temporária com o `tauri-plugin-fs` (importar `.apkg`, restaurar backup, salvar `.apkg`/PDF/backup).
- [x] Fonte Noto Serif JP embutida no Typst, em subset (JIS X 0208 + kana + latim, ~2,5 MB por peso, regular e negrito; script em `spikes/fonts/subset.py`). O PDF sai igual no PC e no celular; caracteres fora do subset usam fontes do sistema.
- [x] Primeiro uso guiado ("Primeiros passos"): Tatoeba (~36 MB, necessário), JMdict (~12 MB, recomendado), voz (~90 MB) e IA local (3,3–5,2 GB), com ✓ no que já está instalado. Abre na primeira vez e depois fica em Configurações.
- [x] Seção "Sobre" em Configurações com os créditos de dados, vozes, modelos e fontes, e a página "Licenças de software" gerada pelo `cargo about` (`about.toml` + `app/licenses.hbs` → `app/ui/licenses.html`).
- [x] Links externos abrem no navegador do sistema (`tauri-plugin-opener`).
- Testado no emulador Android (x86_64): primeiros passos, tela de estudo, backup salvo em Downloads e restaurado pelo seletor do sistema (`content://`) e PDF com a fonte embutida. O `spikes/rebuild-x86.sh` agora define `TAURI_ANDROID_PROJECT_PATH`, para o Gradle incluir os plugins com código Android (fs, opener).

### Fase 5: distribuição pelo GitHub
- [x] Licença **MIT OR Apache-2.0** (`LICENSE-MIT`, `LICENSE-APACHE`, `license` em todos os crates) e `THIRD_PARTY_NOTICES.md` com os avisos do mecab-ipadic e da Noto Serif JP, que também aparecem no app.
  - Todas as dependências têm licenças permissivas. O `genanki-rs` foi trocado por um gravador próprio porque puxava o `ramhorns` (GPL-3.0). O `about.toml` não aceita GPL/AGPL, então o `cargo about` falha se uma dependência assim entrar. Vozes, modelos de IA, Tatoeba e JMdict não vão no repositório: o app baixa na hora, com os termos de cada um.
- [x] GitHub Actions (`release.yml`), sem actions de terceiros além das oficiais e do `Swatinem/rust-cache`:
  - Windows: instalador NSIS `.exe` (português ou inglês, instala só para o usuário, sem pedir administrador). O `.msi` ficou de fora: um instalador só é mais simples para quem baixa.
  - Android: `.apk` arm64 assinado com o keystore dos *secrets*. O Gradle copia o `libc++_shared.so` do NDK sozinho (antes era à mão). A biblioteca sai sem símbolos e comprimida no APK: 40 MB em vez de 146 MB.
  - Testado localmente: o instalador do Windows (26 MB) e o APK arm64 assinado (v2) saem certos, e o mesmo empacotamento abre normalmente no emulador x86_64.
  - O Tauri CLI fica fixado em `app/package.json` (2.12.4), usado pelo build local, pelo Gradle e pelo CI.
- [x] Release disparada por tag `v*`: confere tag × versões × CHANGELOG e usa a seção do `CHANGELOG.md` como notas. `ci.yml` roda os testes a cada push.
- [x] Modelos **fora** do instalador e do APK, baixados pelo app.
- [ ] (Opcional) Assinatura de código no Windows para evitar o aviso do SmartScreen. É paga; sem ela o app funciona, mas mostra o aviso (as notas da release explicam como passar).
- [ ] Criar o repositório no GitHub, cadastrar os secrets e enviar a primeira tag (passo a passo no README, "Publicar uma versão").

### Fase 6: interface v2
Spec: `docs/superpowers/specs/2026-11-03-interface-v2-design.md`; plano: `docs/superpowers/plans/2026-11-03-interface-v2.md`.
- [x] Tokens de tema Sumi (claro/escuro/sistema), ícones Lucide inline (ISC) e `app.js` dividido em módulos ES em `app/ui/js/`.
- [x] Navegação lateral (barra inferior no celular), indicador de estado e Configurações em seções com salvamento automático; **Testar conexão** do AnkiConnect com os erros distinguidos (`anki:not_running`, `anki:no_addon`, `anki:incompatible`).
- [x] Classe de cada palavra (`jp-core::WordClass`, a partir do IPADIC) e vocabulário da frase (`engine::vocabulary`); aba Frases com cores por classe, detalhamento, Copiar e barra de seleção.
- [x] Tradução do vocabulário com a IA configurada (JSON Schema, cache em `vocab_gloss` no `japones.db`) e cores personalizáveis por tema.
- [x] Instalar o recomendado (modelo escolhido pela RAM com `sysinfo`, MIT), pausar/continuar downloads e configuração rápida quando falta algo; teste da chave do Claude (`GET /v1/models`, sem custo).
- [x] Estudar e Texto com as cores por classe; licença dos ícones nos avisos de terceiros.
- [ ] Revisão manual no Windows e no emulador Android (layout de celular, dois temas).

---

## 5. Riscos e o que fazer se falharem

| Risco | Plano B |
|---|---|
| `voicevox_core` não roda no Android | No Android, usar o TTS do sistema (voz japonesa do Android) e manter o VOICEVOX no PC. |
| IA local lenta demais no celular | Modelo menor (E2B) só para completar frases, Tatoeba como principal, Claude opcional. Ou gerar no PC e sincronizar o deck por backup. |
| Frases ruins com modelo pequeno | Validação + nova tentativa já existentes, gramática JSON, few-shot com frases do Tatoeba no prompt. |
| APK grande (dicionário lindera + Typst + fontes) | Fonte em subset. Baixar o dicionário do lindera em vez de embutir (`load_dictionary` por caminho). |
| Termos de alguma voz VOICEVOX incompatíveis | Mostrar só vozes liberadas para o uso do app e documentar no README. |

---

## Fontes consultadas
- Anki LICENSE (AGPL-3.0+): https://github.com/ankitects/anki/blob/main/LICENSE
- VOICEVOX CORE (MIT a partir da 0.16): https://github.com/VOICEVOX/voicevox_core
- Termos dos modelos VVM: https://github.com/VOICEVOX/voicevox_vvm
- fsrs-rs (BSD-3): https://github.com/open-spaced-repetition/fsrs-rs
- llama-cpp-rs (Apache-2.0): https://github.com/utilityai/llama-cpp-rs
- Comparativo de LLMs locais para japonês (2026): https://dev.classmethod.jp/en/articles/local-llm-guide-2026-summer/ e https://lilting.ch/en/articles/japanese-llm-options-compared
