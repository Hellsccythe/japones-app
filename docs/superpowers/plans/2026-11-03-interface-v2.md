# Interface v2 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Novo visual (Sumi, claro/escuro, ícones), aba Frases com cores por classe e detalhamento, Configurações em seções, teste do AnkiConnect e configuração fácil.

**Architecture:** O Rust ganha a classe de cada palavra (`jp-core`), o vocabulário e a tradução com cache (`engine`/`sentences`), o teste do AnkiConnect (`anki`) e a recomendação de modelo (`engine`). O front-end continua sem bundler: `app/ui/app.js` vira módulos ES em `app/ui/js/`, e a lógica pura (tema, cores, texto de cópia, render de tokens) é testada com `node --test`.

**Tech Stack:** Rust (lindera/IPADIC, rusqlite, reqwest, llama.cpp), Tauri 2, HTML/CSS/JS (módulos ES), `node --test`.

**Spec:** `docs/superpowers/specs/2026-11-03-interface-v2-design.md`

## Global Constraints

- Só dependências com licença permissiva (MIT, Apache-2.0, ISC, BSD…); `cargo about generate app/licenses.hbs -o app/ui/licenses.html` tem que continuar passando (o `about.toml` não aceita GPL).
- Nenhuma dependência nova de front-end (sem npm em runtime, sem CDN): o app funciona offline.
- Textos da interface em português do Brasil, frases curtas, verbo primeiro nos botões.
- `settings.json` antigo continua carregando: todo campo novo tem padrão via `#[serde(default)]`.
- Tokens de cor exatamente como na tabela da seção 1 da spec; cores das classes exatamente como na tabela "Cores das classes de palavra".
- Desktop e Android usam a mesma UI; layout de celular abaixo de 600px.
- Ícones: paths no estilo Lucide (ISC), inline; aviso ISC em `THIRD_PARTY_NOTICES.md` e `app/licenses.hbs`.
- Build: `LIBCLANG_PATH=/e/dev/cmake-venv/Lib/site-packages/clang/native` e `/e/dev/cmake-venv/Scripts` no PATH para `cargo test --workspace`.
- Para rodar o app no desktop: `cd app && npm run tauri -- dev` (dados em `%APPDATA%\com.japonesapp.desktop`, ou defina `JAPONES_APP_DATA=E:/projetos/japones-app/data` para usar os dados de desenvolvimento).

## Review Focus

- Frase sem kanji, só pontuação ou vazia (ex.: `。`, `ねえ`): `analyze`/`vocabulary` devolvem lista sem pânico e o cartão renderiza sem linhas vazias no vocabulário. → testes na Task 3.
- Palavra fora do JMdict (gíria, nome próprio): a linha do vocabulário aparece com significado vazio ("—") e "Traduzir com IA" ainda funciona. → teste na Task 3 (`gloss` vazio) e Task 5 (tradução de entrada sem gloss).
- IA responde com número de traduções diferente do pedido: não quebra nem desalinha; entradas sem tradução ficam com o gloss em inglês e nada errado vai para o cache. → teste na Task 5.
- `settings.json` com cor inválida (`"red"`, `"#12"`) ou classe desconhecida: volta ao padrão daquela classe. → teste na Task 1 (`normalizeColors`).
- AnkiConnect devolvendo HTML/porta ocupada por outro programa (não-JSON): mensagem "Falta o add-on AnkiConnect" e não "erro de JSON". → teste na Task 2.

---

### Task 1: Base visual — tokens, tema, ícones e módulos

**Files:**
- Modify: `crates/engine/src/settings.rs`
- Modify: `app/package.json` (adicionar `"type": "module"`)
- Create: `app/ui/js/api.js`, `app/ui/js/icons.js`, `app/ui/js/theme.js`, `app/ui/js/colors.js`, `app/ui/js/main.js`, `app/ui/js/frases.js`, `app/ui/js/estudar.js`, `app/ui/js/texto.js`, `app/ui/js/config.js`, `app/ui/js/setup.js`
- Delete: `app/ui/app.js` (conteúdo movido para os módulos)
- Modify: `app/ui/index.html` (`<script type="module" src="js/main.js">`), `app/ui/style.css` (reescrito com tokens)
- Test: `app/ui/js/theme.test.js`, `app/ui/js/colors.test.js`, `crates/engine/src/settings.rs` (mod tests)
- Modify: `.github/workflows/ci.yml` (trocar `node --check app/ui/app.js` por `node --test app/ui/js/`)

**Interfaces:**
- Produces (Rust, `engine::Settings`, todos com `#[serde(default)]`):
  - `theme: String` — `"system" | "light" | "dark"`, padrão `"system"`
  - `pos_colors: PosColors` — `struct PosColors { light: BTreeMap<String, String>, dark: BTreeMap<String, String> }`, padrão vazio (vazio = cor padrão da classe)
  - `pos_colors_on: bool` — padrão `true`
  - `jp_font_size: String` — `"small" | "medium" | "large"`, padrão `"medium"`
  - `examples_count: usize` — padrão `5`
  - `fill_with_llm: bool` — padrão `true`
  - `llm_provider` aceita também `"off"`; `Settings::uses_claude()` continua igual, e `generate` com `"off"` falha com "A IA está desligada nas configurações".
- Produces (JS):
  - `api.js`: `invoke`, `listen`, `dialog`, `$`, `toast(msg, isError)`, `busy(btn, fn)`, `escapeHtml(s)`, `isAndroid` (movidos de `app.js` sem mudar comportamento).
  - `icons.js`: `icon(name: string, size = 18): string` → `<svg class="ic" …>` com `stroke="currentColor"`; nomes: `volume, chevron-down, chevron-up, check, plus, copy, sparkles, search, download, upload, trash, sliders, book, file, message, moon, sun, info, x, refresh, alert, play`.
  - `theme.js`: `resolveTheme(setting: string, prefersDark: boolean): 'light'|'dark'`; `applyTheme(setting)` põe `data-theme` em `<html>` e escuta `prefers-color-scheme` quando `setting === 'system'`.
  - `colors.js`: `CLASSES = ['noun','verb','particle','adjective','auxiliary']`; `DEFAULT_COLORS = { light: {...}, dark: {...} }` (valores da spec); `normalizeColors(saved): {light, dark}` (completa e troca inválidos pelo padrão; válido = `/^#[0-9a-fA-F]{6}$/`); `applyColors(colors, on)` define `--pos-<classe>` em `:root[data-theme=…]` via `<style id="pos-colors">`.

- [ ] **Step 1: Teste Rust das configurações novas**

```rust
#[test]
fn old_settings_file_gets_new_defaults() {
    let s: Settings = serde_json::from_str(r#"{"llm_provider":"claude","deck":"X"}"#).unwrap();
    assert_eq!(s.theme, "system");
    assert!(s.pos_colors_on);
    assert_eq!(s.jp_font_size, "medium");
    assert_eq!(s.examples_count, 5);
    assert!(s.fill_with_llm);
    assert_eq!(s.llm_provider, "claude");
}
```

- [ ] **Step 2:** `cargo test -p engine settings` → FAIL (campos não existem).
- [ ] **Step 3:** Adicionar os campos e `PosColors` em `settings.rs`; `"off"` no `generate` do `engine/src/lib.rs`.
- [ ] **Step 4:** `cargo test -p engine settings` → PASS.
- [ ] **Step 5: Testes JS**

```js
// theme.test.js
import { test } from 'node:test'; import assert from 'node:assert/strict';
import { resolveTheme } from './theme.js';
test('system follows the OS', () => { assert.equal(resolveTheme('system', true), 'dark'); assert.equal(resolveTheme('system', false), 'light'); });
test('explicit choice wins', () => { assert.equal(resolveTheme('light', true), 'light'); assert.equal(resolveTheme('dark', false), 'dark'); });
test('unknown value means system', () => assert.equal(resolveTheme('blue', true), 'dark'));

// colors.test.js
import { normalizeColors, DEFAULT_COLORS } from './colors.js';
test('empty uses defaults', () => assert.deepEqual(normalizeColors({}), DEFAULT_COLORS));
test('invalid and unknown entries fall back', () => {
  const c = normalizeColors({ dark: { noun: 'red', verb: '#12', particle: '#AABBCC', foo: '#000000' } });
  assert.equal(c.dark.noun, DEFAULT_COLORS.dark.noun);
  assert.equal(c.dark.verb, DEFAULT_COLORS.dark.verb);
  assert.equal(c.dark.particle, '#AABBCC');
  assert.equal(c.dark.foo, undefined);
});
```

- [ ] **Step 6:** `node --test app/ui/js/` → FAIL (módulos não existem).
- [ ] **Step 7:** Criar os módulos e mover o código de `app.js` por seção (os blocos `// ---------- x ----------` de hoje): Frases/export → `frases.js`, estudar → `estudar.js`, texto → `texto.js`, configurações/IA/voz/import → `config.js`, primeiros passos → `setup.js`, tabs/status/init/links externos/sobre → `main.js`. Cada módulo exporta `init()`, chamado por `main.js` na ordem atual. Módulos que precisam de estado de outro recebem por import (ex.: `config.js` exporta `loadSettings`, `getLoaded()`, `saveSettingsNow`).
- [ ] **Step 8:** Reescrever `style.css`: variáveis da tabela da spec em `:root` (escuro) e `:root[data-theme='light']`; `--pos-*` padrão; componentes `.btn` (principal), `.btn.ghost`, `.btn-icon` (32×32, `.on`), `.pill`, `.tag`, `.switch`, `.seg`, `.card`; fontes `--font-ui` e `--font-jp` (`'Noto Serif JP','Yu Mincho',serif`); `--jp-size` 18/21/24px por `[data-jp-size]`; transições de 150 ms; `select option` com fundo `--surface`. Manter as regras de layout existentes (`.android`, safe-area, 600px) adaptadas às variáveis.
- [ ] **Step 9:** `node --test app/ui/js/` → PASS. `cargo test --workspace` → PASS.
- [ ] **Step 10: Verificar no app** — `npm run tauri -- dev` em `app/`: as quatro abas funcionam como antes (buscar 見学, tocar áudio, abrir deck, mostrar furigana, salvar configuração); trocar `theme` para `light`/`dark` em Configurações (controle provisório no topo da página, substituído na Task 2) muda as cores na hora; nada de erro no console do DevTools.
- [ ] **Step 11: Commit** — `git add -A app/ui app/package.json crates/engine .github/workflows/ci.yml && git commit -m "Interface: tokens de tema, claro/escuro, ícones e app.js em módulos"`

---

### Task 2: Navegação, Configurações em seções e teste do Anki

**Files:**
- Modify: `crates/anki/src/connect.rs`, `crates/anki/src/lib.rs` (variantes de erro), `crates/anki/Cargo.toml` (dev-dep `tokio = { workspace = true }`)
- Modify: `crates/engine/src/lib.rs` (`anki_test`), `app/src/lib.rs` (comando `anki_test`)
- Modify: `app/ui/index.html`, `app/ui/style.css`, `app/ui/js/main.js`, `app/ui/js/config.js`, `app/ui/js/frases.js` (mensagem do erro do Anki)
- Test: `crates/anki/src/connect.rs` (mod tests)

**Interfaces:**
- Produces (Rust):
  - `anki::Error::{NotRunning, NoAddon, Incompatible(u32)}` (novas variantes; `Unreachable` passa a ser mapeado para `NotRunning` em conexão recusada).
  - `AnkiConnect::check(&self) -> Result<u32>` — chama `version`; conexão recusada/timeout (2 s) → `NotRunning`; resposta não-JSON ou sem `result` numérico → `NoAddon`; versão < 6 → `Incompatible(v)`.
  - `add_cards` chama `check()` antes, para o erro do envio ser o mesmo do teste.
  - `Engine::anki_test(&self) -> Result<u32>`; comando Tauri `anki_test() -> CmdResult<u32>`; o erro chega ao JS como texto com prefixo de código: `"anki:not_running"`, `"anki:no_addon"`, `"anki:incompatible:<v>"`.
- Produces (JS): `config.js` exporta `openSection(id)` com ids `aparencia, ia, frases, voz, anki, backup, sobre`; `main.js` exporta `setStatus(status)` que pinta o indicador (verde/amarelo/vermelho) e os pontos das seções.

- [ ] **Step 1: Testes do `check`** com um servidor TCP de teste (`tokio::net::TcpListener` em `127.0.0.1:0` que responde uma resposta HTTP fixa):

```rust
#[tokio::test] async fn check_reads_version() { /* responde {"result":6,"error":null} */ assert_eq!(client.check().await.unwrap(), 6); }
#[tokio::test] async fn check_not_running() { /* porta sem ninguém escutando */ assert!(matches!(client.check().await, Err(Error::NotRunning))); }
#[tokio::test] async fn check_no_addon_on_html() { /* responde 200 text/html "<html>" */ assert!(matches!(client.check().await, Err(Error::NoAddon))); }
#[tokio::test] async fn check_incompatible() { /* {"result":5,"error":null} */ assert!(matches!(client.check().await, Err(Error::Incompatible(5)))); }
```

- [ ] **Step 2:** `cargo test -p anki connect` → FAIL.
- [ ] **Step 3:** Implementar `check`, as variantes e o mapeamento; `Engine::anki_test`; comando `anki_test` registrado no `invoke_handler`; prefixos de erro no `CmdResult` do comando (e no `export_anki_connect`).
- [ ] **Step 4:** `cargo test -p anki` → PASS.
- [ ] **Step 5: Navegação** — `index.html`: `<nav class="sidebar">` com logo 日本, botões de ícone Frases/Estudar/Texto (`data-tab`), espaçador, indicador de status e Configurações; abaixo de 600px vira barra inferior fixa (`env(safe-area-inset-bottom)`). Sai o texto longo de status do topo; `setStatus` usa `status()`: vermelho se a aba atual não tem o que precisa (Frases sem Tatoeba e sem IA), amarelo se falta algo opcional (dicionário, voz, IA), verde caso contrário; clicar abre a seção que falta.
- [ ] **Step 6: Configurações** — lista de seções à esquerda (no celular: lista como primeira tela, seção em tela própria com botão voltar). Conteúdo por seção conforme a tabela da seção 4 da spec, reaproveitando os controles atuais: interruptores no lugar dos checkboxes, controle segmentado para provedor (Local/Claude/Desligada) e frases por busca (3/5/10), "Avançado" recolhido (threads, JLPT, idioma), log de importação recolhido em "ver detalhes" com barra de progresso, termos da voz resumidos com "ler termos"; o **Testar** da chave do Claude entra na Task 6. **Aparência**: tema (Sistema/Claro/Escuro), interruptor das cores, tamanho do japonês (P/M/G). **Anki** (só desktop): URL, **Testar conexão** mostrando "Conectado (AnkiConnect v6)" ou a mensagem do erro, e o passo a passo com o código `2055492159` e botão copiar.
- [ ] **Step 7: Erro do envio** — em `frases.js`, falha de `export_anki_connect` mostra a mensagem amigável do código (`anki:not_running` → "O Anki não está aberto. Abra o Anki e tente de novo."; `anki:no_addon` → "Falta o add-on AnkiConnect no Anki."), com botões **Ver como instalar** (abre `openSection('anki')`) e **Salvar .apkg**.
- [ ] **Step 8:** `node --test app/ui/js/` e `cargo test --workspace` → PASS.
- [ ] **Step 9: Verificar no app** — desktop: navegação lateral, cada seção abre, o tema troca pela Aparência, **Testar conexão** com o Anki fechado mostra "O Anki não está aberto"; viewport de 375px (DevTools) mostra barra inferior e Configurações em telas.
- [ ] **Step 10: Commit** — `git commit -m "Interface: navegação lateral, Configurações em seções e teste do AnkiConnect"`

---

### Task 3: Classe das palavras e vocabulário (Rust)

**Files:**
- Modify: `crates/jp-core/src/lib.rs`
- Create: `crates/engine/src/vocab.rs`
- Modify: `crates/engine/src/lib.rs` (`ExampleView.tokens`, `Engine::analyze`, `Engine::vocabulary`), `app/src/lib.rs` (comandos `analyze`, `vocabulary`)
- Test: `crates/jp-core/src/lib.rs` (mod tests), `crates/engine/src/vocab.rs` (mod tests)

**Interfaces:**
- Produces (`jp-core`):
  - `#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)] #[serde(rename_all = "lowercase")] pub enum WordClass { Noun, Verb, Particle, Adjective, Auxiliary, Other }`
  - `pub fn word_class(pos: &str, pos_detail: &str) -> WordClass` — 名詞 → Noun, exceto detalhe `形容動詞語幹` → Adjective; 動詞 → Verb; 助詞 → Particle; 形容詞/副詞/連体詞 → Adjective; 助動詞 → Auxiliary; resto → Other.
  - `Morph` ganha `pos_detail: String` (coluna 1 do IPADIC).
  - `pub struct Token { pub surface: String, pub base: String, pub reading: Option<String>, pub segments: Vec<Segment>, pub class: WordClass }`
  - `Analyzer::analyze(&self, text: &str) -> Result<Vec<Token>>` — um token por morfema, `segments` = furigana daquele morfema (mesma regra de `furigana()`).
- Produces (`engine`):
  - `pub struct VocabEntry { pub base: String, pub reading: Option<String>, pub class: WordClass, pub gloss: String }` (Serialize/Deserialize)
  - `pub fn vocabulary(tokens: &[Token], lookup: impl Fn(&str) -> Option<String>, kana: impl Fn(&str) -> Option<String>) -> Vec<VocabEntry>` em `vocab.rs` — pula tokens `Other` que são só pontuação/símbolo ou espaço; agrupa por `base` mantendo a primeira ocorrência; `reading` = `kana(base)` só quando `base` tem kanji (senão `None`); `gloss` = `lookup(base)` ou `""`. O `Engine` passa `|s| analyzer.to_kana(s).ok()` como `kana`.
  - `ExampleView` ganha `tokens: Vec<Token>`.
  - `Engine::analyze(&self, text) -> Result<Vec<Token>>`, `Engine::vocabulary(&self, text) -> Result<Vec<VocabEntry>>` (gloss = até 3 glosses do primeiro sentido do `dict.first(base)`, unidos por `"; "`).
  - Comandos Tauri `analyze(text) -> Vec<Token>` e `vocabulary(text) -> Vec<VocabEntry>`.

- [ ] **Step 1: Testes do `jp-core`**

```rust
#[test] fn classes_of_a_sentence() {
    let a = Analyzer::new().unwrap();
    let t = a.analyze("日本には見学するところが色々あります。").unwrap();
    let class = |s: &str| t.iter().find(|x| x.surface == s).unwrap().class;
    assert_eq!(class("日本"), WordClass::Noun);
    assert_eq!(class("に"), WordClass::Particle);
    assert_eq!(class("する"), WordClass::Verb);
    assert_eq!(class("ます"), WordClass::Auxiliary);
    assert_eq!(class("色々"), WordClass::Adjective);
    assert_eq!(class("。"), WordClass::Other);
}
#[test] fn na_adjective_stem_is_adjective() { assert_eq!(word_class("名詞", "形容動詞語幹"), WordClass::Adjective); }
#[test] fn token_segments_carry_furigana() {
    let t = Analyzer::new().unwrap().analyze("見学").unwrap();
    assert_eq!(t[0].segments[0].reading.as_deref(), Some("けんがく"));
}
#[test] fn empty_and_punctuation_only() {
    let a = Analyzer::new().unwrap();
    assert!(a.analyze("").unwrap().is_empty());
    assert!(a.analyze("。").unwrap().iter().all(|t| t.class == WordClass::Other));
}
```

- [ ] **Step 2:** `cargo test -p jp-core` → FAIL.
- [ ] **Step 3:** Implementar `pos_detail`, `word_class`, `Token`, `analyze`; `cargo test -p jp-core` → PASS.
- [ ] **Step 4: Testes do vocabulário** (`vocab.rs`, com `Analyzer::new()` e lookups de teste):

```rust
#[test] fn groups_conjugations_and_keeps_order() {
    let a = Analyzer::new().unwrap();
    let t = a.analyze("見学していってください。").unwrap();
    let v = vocabulary(&t, |_| None, |s| a.to_kana(s).ok());
    let bases: Vec<_> = v.iter().map(|e| e.base.as_str()).collect();
    assert_eq!(bases.first(), Some(&"見学"));
    assert!(bases.contains(&"する") && bases.contains(&"いく"));
    assert_eq!(bases.iter().filter(|b| **b == "て").count(), 1);   // repetida, aparece uma vez
    assert!(!bases.contains(&"。"));
}
#[test] fn unknown_word_has_empty_gloss() {
    let a = Analyzer::new().unwrap();
    let v = vocabulary(&a.analyze("ねえ").unwrap(), |_| None, |s| a.to_kana(s).ok());
    assert!(v.iter().all(|e| e.gloss.is_empty()));
}
#[test] fn reading_only_with_kanji() {
    let a = Analyzer::new().unwrap();
    let v = vocabulary(&a.analyze("見学する").unwrap(), |_| Some("x".into()), |s| a.to_kana(s).ok());
    assert_eq!(v[0].reading.as_deref(), Some("けんがく"));
    assert_eq!(v[1].reading, None);
    assert_eq!(v[0].gloss, "x");
}
```

(`いく`: o IPADIC devolve a forma de dicionário de いっ como `いく`; se o teste mostrar outra forma, use a que o IPADIC devolve e ajuste a asserção.)

- [ ] **Step 5:** `cargo test -p engine vocab` → FAIL; implementar; → PASS.
- [ ] **Step 6:** `ExampleView.tokens` preenchido em `Engine::view`; `Engine::analyze`/`vocabulary`; comandos registrados. `cargo test --workspace` → PASS.
- [ ] **Step 7: Commit** — `git commit -m "Classe gramatical das palavras e vocabulário da frase"`

---

### Task 4: Aba Frases nova

**Files:**
- Create: `app/ui/js/render.js` (funções puras de HTML/texto)
- Modify: `app/ui/js/frases.js`, `app/ui/index.html` (seção Frases), `app/ui/style.css`
- Test: `app/ui/js/render.test.js`

**Interfaces:**
- Consumes: `ExampleView.tokens` e o comando `vocabulary(text)` (Task 3); `icon()` (Task 1); `applyColors` (Task 1).
- Produces (`render.js`):
  - `tokensHtml(tokens): string` — cada token vira `<span class="w w-<class>">` com `<ruby>…<rt>…</rt></ruby>` nos segmentos com leitura; texto escapado.
  - `copyText(sentence: string, translation: string, vocab: VocabEntry[]): string` — `"${sentence}\n${translation}\n\n"` + uma linha por entrada: `"${base} (${reading}) = ${gloss}"`, sem `(…)` quando `reading` é nulo e com `"—"` quando `gloss` é vazio. Nunca contém `[` nem `]`.
  - `vocabHtml(vocab): string` — linhas da grade (forma na cor da classe, leitura, significado ou `—`).
  - `legendHtml(): string` — bolinhas clicáveis `data-class="<classe>"` com os nomes "subst.", "verbo", "partícula", "adj./adv.", "aux.".

- [ ] **Step 1: Testes**

```js
test('tokensHtml colors by class and adds ruby', () => {
  const html = tokensHtml([{ surface: '見学', class: 'noun', segments: [{ text: '見学', reading: 'けんがく' }] },
                           { surface: 'に', class: 'particle', segments: [{ text: 'に', reading: null }] }]);
  assert.match(html, /class="w w-noun"><ruby>見学<rt>けんがく<\/rt><\/ruby>/);
  assert.match(html, /class="w w-particle">に</);
});
test('tokensHtml escapes', () => assert.doesNotMatch(tokensHtml([{ surface: '<b>', class: 'other', segments: [{ text: '<b>', reading: null }] }]), /<b>/));
test('copyText has no brackets', () => {
  const t = copyText('日本には見学する。', 'No Japão…', [
    { base: '日本', reading: 'にほん', class: 'noun', gloss: 'Japan' },
    { base: 'に', reading: null, class: 'particle', gloss: '' }]);
  assert.equal(t, '日本には見学する。\nNo Japão…\n\n日本 (にほん) = Japan\nに = —');
  assert.doesNotMatch(t, /[\[\]]/);
});
```

- [ ] **Step 2:** `node --test app/ui/js/` → FAIL; implementar `render.js`; → PASS.
- [ ] **Step 3: Tela** conforme a seção 3 da spec:
  - busca em uma linha (lupa, **Gerar com IA** ghost, **Buscar**), quantidade e "completar com IA" lidos de `examples_count`/`fill_with_llm`;
  - dicionário em uma linha com `+N` para os outros sentidos;
  - cartões com `tokensHtml` (classe `.jp`, fonte e tamanho do japonês), tradução editável como hoje, ações Ouvir/Detalhes/Selecionar com `icon()` e `aria-label`, tag de origem; selecionado com `--ring`;
  - Detalhes busca `vocabulary(sentence)` na primeira abertura (guarda no cartão), mostra `vocabHtml` + `legendHtml` + **Copiar** (`navigator.clipboard.writeText(copyText(...))`, toast "Copiado") — o botão **Traduzir com IA** entra na Task 5;
  - barra de seleção fixa no rodapé só com seleção: contador, deck (campo), **Exportar ▾** (menu com Enviar ao Anki — só desktop —, Salvar .apkg, Salvar PDF e os interruptores "Incluir áudio" e "Furigana no PDF"), **Adicionar ao deck**; o painel lateral "Selecionadas" sai; "Limpar seleção" fica no menu.
  - `pos_colors_on = false` adiciona `.no-pos` no `<html>`, que tira as cores (`.w { color: inherit }`).
- [ ] **Step 4: Verificar no app** — buscar 見学: cores por classe nos dois temas; Detalhes abre/fecha e lista どうぞ/見学/する/いく/ください; Copiar cola o texto sem `[]`; selecionar mostra a barra e Exportar ▾ salva `.apkg` e PDF como antes; em 375px a barra ocupa a largura e não cobre a navegação inferior.
- [ ] **Step 5: Commit** — `git commit -m "Aba Frases: cores por classe, detalhamento e barra de seleção"`

---

### Task 5: Tradução do vocabulário com IA e cores personalizáveis

**Files:**
- Create: `crates/sentences/src/vocab_prompt.rs`
- Modify: `crates/sentences/src/lib.rs` (`pub mod vocab_prompt`), `crates/sentences/src/llm.rs` (`translate_vocabulary`), `crates/sentences/src/store.rs` (tabela e métodos de cache)
- Modify: `crates/local-llm/src/lib.rs` (`translate_vocabulary`)
- Modify: `crates/engine/src/lib.rs` (`Engine::translate_vocabulary`), `app/src/lib.rs` (comando)
- Modify: `app/ui/js/frases.js`, `app/ui/js/colors.js`, `app/ui/js/config.js`, `app/ui/style.css`
- Test: `crates/sentences/src/vocab_prompt.rs`, `crates/sentences/src/store.rs` (mod tests)

**Interfaces:**
- Consumes: `VocabEntry` (Task 3), `Settings.pos_colors` (Task 1), `normalizeColors`/`applyColors` (Task 1), `legendHtml` (Task 4).
- Produces:
  - `sentences::vocab_prompt::{SYSTEM, user_prompt(words: &[(String, String)], language: &str) -> String, schema(count: usize) -> Value, parse(text: &str, count: usize) -> Result<Vec<String>>}` — `words` = `(base, gloss_en)`; schema `{ "translations": [string; count] }` (`minItems`/`maxItems` = `count`); `parse` devolve exatamente `count` itens, completando com `""` quando vier menos e cortando o excesso.
  - `LlmClient::translate_vocabulary(&self, words: &[(String, String)], language: &str) -> Result<Vec<String>>` (mesma chamada HTTP do `generate`, `effort: "low"`); `LocalLlm::translate_vocabulary(&self, words, language) -> Result<Vec<String>>` (gramática do schema, `max_tokens` = `50 + 30 * words.len()`).
  - `Store`: tabela `vocab_gloss (base TEXT, reading TEXT, lang TEXT, gloss TEXT, PRIMARY KEY (base, reading, lang))`; `cached_glosses(&self, keys: &[(String, String)], lang: &str) -> Result<HashMap<(String, String), String>>` (`reading` vazio quando `None`); `save_glosses(&mut self, rows: &[(String, String, String)], lang: &str) -> Result<()>` com `(base, reading, gloss)`, ignorando gloss vazio.
  - `Engine::translate_vocabulary(&self, entries: Vec<VocabEntry>) -> Result<Vec<VocabEntry>>` — idioma = `translation_language_name`; consulta o cache, pede à IA só os que faltam, salva, devolve as entradas com `gloss` trocado (entrada sem tradução mantém o gloss original); provedor `"off"` → erro "ia:off".
  - Comando `translate_vocabulary(entries) -> Vec<VocabEntry>`.

- [ ] **Step 1: Testes**

```rust
// vocab_prompt.rs
#[test] fn parse_pads_and_truncates() {
    assert_eq!(parse(r#"{"translations":["visitar"]}"#, 2).unwrap(), vec!["visitar".to_string(), String::new()]);
    assert_eq!(parse(r#"{"translations":["a","b","c"]}"#, 2).unwrap().len(), 2);
}
#[test] fn schema_fixes_count() { let s = schema(3); assert_eq!(s["properties"]["translations"]["minItems"], 3); assert_eq!(s["properties"]["translations"]["maxItems"], 3); }
#[test] fn prompt_lists_words_and_language() { let p = user_prompt(&[("見学".into(), "field trip".into())], "português"); assert!(p.contains("見学") && p.contains("field trip") && p.contains("português")); }

// store.rs
#[test] fn gloss_cache_roundtrip_and_skips_empty() {
    let mut s = test_store(); // helper existente do módulo de testes (ou Store::open em arquivo temporário)
    s.save_glosses(&[("見学".into(), "けんがく".into(), "visitar".into()), ("に".into(), String::new(), String::new())], "português").unwrap();
    let got = s.cached_glosses(&[("見学".into(), "けんがく".into()), ("に".into(), String::new())], "português").unwrap();
    assert_eq!(got.get(&("見学".into(), "けんがく".into())).map(String::as_str), Some("visitar"));
    assert!(!got.contains_key(&("に".into(), String::new())));
    assert!(s.cached_glosses(&[("見学".into(), "けんがく".into())], "english").unwrap().is_empty());
}
```

- [ ] **Step 2:** `cargo test -p sentences` → FAIL; implementar `vocab_prompt`, cache, `LlmClient::translate_vocabulary`, `LocalLlm::translate_vocabulary`, `Engine::translate_vocabulary` e o comando; → PASS. `cargo test --workspace` → PASS.
- [ ] **Step 3: Botão Traduzir com IA** no cabeçalho do vocabulário: chama o comando, troca a grade, toast em erro; `ia:off` ou modelo não baixado abre o modal rápido da IA (Task 6; até lá, `openSection('ia')`).
- [ ] **Step 4: Cores** — clique numa bolinha da legenda (e nas linhas de *Aparência*) abre um popover com 8 cores sugeridas (as 5 padrão do tema atual + `#E0B050`, `#F08080`, `#A0A0FF` no escuro / `#A86F0C`, `#C0392B`, `#5B4BC4` no claro), `<input type="color">` e **Restaurar padrão**; salva em `pos_colors[<tema atual>][<classe>]` via `save_settings` e chama `applyColors` na hora.
- [ ] **Step 5: Verificar no app** — com o Gemma baixado (ou Claude), Traduzir com IA troca os significados para português; repetir na mesma frase responde na hora (cache); mudar a cor do verbo no escuro não muda a do claro; Restaurar padrão volta.
- [ ] **Step 6: Commit** — `git commit -m "Tradução do vocabulário com IA (com cache) e cores personalizáveis"`

---

### Task 6: Instalar o recomendado e configuração rápida

**Files:**
- Modify: `Cargo.toml` (workspace: `sysinfo = { version = "0.37", default-features = false, features = ["system"] }` — confirmar a versão estável mais recente no crates.io e a licença MIT), `crates/engine/Cargo.toml`
- Create: `crates/engine/src/setup.rs`
- Modify: `crates/engine/src/lib.rs`, `app/src/lib.rs` (comandos `recommended_setup`, `test_claude`), `crates/sentences/src/llm.rs` (`check`)
- Modify: `app/ui/js/setup.js`, `app/ui/index.html` (overlay de primeiros passos e modal), `app/ui/style.css`
- Test: `crates/engine/src/setup.rs` (mod tests)

**Interfaces:**
- Produces:
  - `pub fn pick_model(total_ram_bytes: u64, android: bool) -> &'static str` — `"gemma-4-e4b"` se `!android && total_ram_bytes >= 16 * 1024^3` (use o limiar em GiB menos 5% para tolerar a RAM reservada: `>= 15_200_000_000`), senão `"gemma-4-e2b"`. (Conferir os ids exatos em `local_llm::MODELS`.)
  - `pub struct RecommendedSetup { pub model_id: String, pub model_name: String, pub items: Vec<SetupItem>, pub total_bytes: u64 }`, `pub struct SetupItem { pub id: String /* "tatoeba" | "dict" | "voice" | "model" */, pub label: String, pub bytes: u64, pub done: bool }`
  - `Engine::recommended_setup(&self) -> RecommendedSetup` (tamanhos: os que o app já mostra hoje nos primeiros passos; voz = pacote `0.vvm` + núcleo).
  - Comando `recommended_setup() -> RecommendedSetup`.
  - `LlmClient::check(&self) -> Result<()>` — `GET https://api.anthropic.com/v1/models?limit=1` com os mesmos cabeçalhos (não gera tokens, não custa); 401 → `Error::Api("chave inválida")`. `Engine::test_claude(&self, key: &str) -> Result<()>` e comando `test_claude(key)`, usados pelo **Testar** da seção IA (Task 2 deixa o botão chamando este comando, adicionado aqui) e do modal.
  - JS `setup.js`: `openQuickSetup(item: 'ia' | 'voz' | 'frases' | 'dict')` — modal com uma linha de explicação, tamanho e o botão de ação daquele item (IA: baixar o modelo recomendado **ou** usar o Claude com campo de chave, link `https://console.anthropic.com/settings/keys` e **Testar**).

- [ ] **Step 1: Testes**

```rust
#[test] fn desktop_with_16gb_gets_e4b() { assert_eq!(pick_model(16 * (1 << 30), false), "gemma-4-e4b"); }
#[test] fn reserved_ram_still_counts_as_16gb() { assert_eq!(pick_model(15_500_000_000, false), "gemma-4-e4b"); }
#[test] fn small_pc_gets_e2b() { assert_eq!(pick_model(8 * (1 << 30), false), "gemma-4-e2b"); }
#[test] fn android_always_e2b() { assert_eq!(pick_model(24 * (1 << 30), true), "gemma-4-e2b"); }

// sentences/src/llm.rs — parse da resposta do /v1/models (função pura extraída do check)
#[test] fn models_check_status() { assert!(check_status(200).is_ok()); assert!(matches!(check_status(401), Err(Error::Api(m)) if m.contains("chave inválida"))); }
```

- [ ] **Step 2:** `cargo test -p engine setup` → FAIL; implementar; → PASS. `cargo about generate app/licenses.hbs -o app/ui/licenses.html` → sem erro (sysinfo MIT).
- [ ] **Step 3: Primeiros passos** — overlay com o total ("Frases, dicionário, voz e IA (Gemma 4 E2B): 3,6 GB"), **Instalar o recomendado** (aceite dos termos da voz antes, no próprio overlay), **Escolher eu mesmo** (fluxo atual por seções) e **Usar o Claude em vez de baixar a IA**; a instalação roda os downloads existentes em sequência (pula os `done`) com uma barra única somando os eventos de progresso atuais; **Pausar** cancela o download em curso (o arquivo parcial fica) e **Continuar** recomeça de onde parou; erro mostra **Tentar de novo**.
- [ ] **Step 4: Modal rápido** — Gerar com IA sem modelo/IA desligada, Ouvir sem voz e Buscar sem Tatoeba abrem `openQuickSetup` do item, em vez do toast atual.
- [ ] **Step 5: Verificar no app** — com dados vazios (`JAPONES_APP_DATA` apontando para uma pasta nova): primeiros passos mostram 4 itens e o total; instalar baixa tudo; pausar/continuar retoma; Gerar com IA sem modelo abre o modal.
- [ ] **Step 6: Commit** — `git commit -m "Primeiros passos: instalar o recomendado e configuração rápida"`

---

### Task 7: Estudar e Texto, Android, licenças e documentação

**Files:**
- Modify: `crates/engine/src/lib.rs` (`StudyView.tokens: Vec<Token>`), `app/ui/js/estudar.js`, `app/ui/js/texto.js`, `app/ui/index.html`, `app/ui/style.css`
- Modify: `THIRD_PARTY_NOTICES.md`, `app/licenses.hbs` (Lucide ISC), `app/ui/licenses.html` (regenerado)
- Modify: `README.md`, `CHANGELOG.md` (seção `## [Não lançado]`), `docs/PLANO.md` (Fase 6: interface v2)

**Interfaces:**
- Consumes: `tokensHtml` (Task 4), `Token` (Task 3).
- Produces: `StudyView.tokens` (tokens da frase do card) e comando de furigana do Texto devolvendo também `tokens`.

- [ ] **Step 1:** `StudyView.tokens` preenchido em `next_card`; Estudar usa `tokensHtml` na frase do card e a fonte do japonês; botões de resposta com as cores de estado (`--bad`, `--warn`, `--ok`, `--accent`); Texto usa `tokensHtml` no resultado. Visual dos decks, edição de deck e do Texto com os componentes novos (cartões, interruptores, botões de ícone).
- [ ] **Step 2:** `cargo test --workspace` e `node --test app/ui/js/` → PASS.
- [ ] **Step 3: Licenças** — Lucide: "ISC License, Copyright (c) for portions of Lucide are held by Cole Bemis 2013-2022 as part of Feather (MIT). All other copyright (c) for Lucide are held by Lucide Contributors 2022." + texto ISC, em `THIRD_PARTY_NOTICES.md` e como seção estática em `app/licenses.hbs`; regenerar `licenses.html`.
- [ ] **Step 4: Android** — `bash spikes/rebuild-x86.sh`, instalar no emulator-5554: navegação inferior, Configurações em telas com voltar, cartões e barra de seleção sem sobrepor a barra de gestos, tema escuro e claro (Aparência), Detalhes e Copiar.
- [ ] **Step 5: Docs** — README (seção de funcionalidades: detalhamento, cores, temas, Anki/teste de conexão), CHANGELOG `## [Não lançado]` com as mudanças, PLANO com a Fase 6 marcada.
- [ ] **Step 6: Commit** — `git commit -m "Estudar e Texto no visual novo, licença dos ícones e documentação"`
