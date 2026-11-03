# Interface v2: visual moderno, detalhamento da frase e configuração fácil

Data: 2026-11-03 · Status: aprovado no chat, aguardando revisão da spec

## Objetivo

Deixar o app mais organizado, bonito e fácil para quem baixa do GitHub sem conhecer o projeto:

- visual moderno (referência: o projeto *aplicativo Roleplay IA*), com tema claro e escuro;
- menos informação de uma vez nas abas Frases e Configurações;
- um **detalhamento** por frase, com o vocabulário palavra a palavra;
- cores por **classe de palavra** dentro da frase japonesa, personalizáveis;
- configuração inicial em poucos cliques, incluindo baixar a IA;
- mensagens claras quando o envio ao Anki (AnkiConnect) falha.

Fora do escopo: mudar o funcionamento do estudo (FSRS), da exportação ou dos formatos de arquivo.

## Decisões tomadas

| Tema | Decisão |
|---|---|
| Stack | HTML/CSS/JS **sem bundler**, como hoje. `app.js` (900 linhas) dividido em módulos ES por aba. |
| Paleta | **Sumi**: neutros de papel e tinta, destaque índigo. |
| Tema | Sistema (padrão), Claro ou Escuro. |
| Contraste na frase | Cor por classe de palavra (substantivo, verbo, partícula, adjetivo/advérbio, auxiliar). |
| Significados | Inglês do JMdict na hora; botão **Traduzir com IA** passa para o idioma configurado e guarda em cache. |
| Ícones | Ícones de linha no estilo Lucide (ISC), em SVG inline, sem biblioteca. |
| Anki | Erro do AnkiConnect vira guia (abrir o Anki, instalar o add-on `2055492159`), com **Testar conexão** e alternativa **Salvar .apkg**. |

## 1. Sistema visual

### Tokens

Todos os estilos usam variáveis CSS definidas em `:root` (escuro) e `:root[data-theme='light']`. O tema "Sistema" troca o atributo pela `prefers-color-scheme`.

| Token | Escuro | Claro | Uso |
|---|---|---|---|
| `--bg` | `#111214` | `#F7F6F3` | fundo da página |
| `--sidebar` | `#16171A` | `#EFEEEA` | navegação |
| `--surface` | `#1B1C20` | `#FFFFFF` | cartões |
| `--surface-2` | `#23252A` | `#F2F1ED` | campos, blocos internos |
| `--surface-3` | `#2E3036` | `#E6E4DE` | hover, segmentos ativos |
| `--text` | `#ECECEF` | `#1F1F1F` | texto |
| `--muted` | `#8D8F98` | `#6B6A66` | texto secundário |
| `--faint` | `#5F616A` | `#A3A19B` | placeholders |
| `--accent` | `#8B9CF7` | `#3B5BA9` | botão principal, item ativo |
| `--accent-soft` | `rgba(139,156,247,.14)` | `#E8ECF8` | fundo de item ativo/pílula |
| `--ring` | `rgba(139,156,247,.45)` | `rgba(59,91,169,.35)` | foco e cartão selecionado |
| `--border` | `#26282D` | `#E6E4DE` | bordas |
| `--ok` / `--warn` / `--bad` | `#6FCF97` / `#E0B050` / `#F08080` | `#2E7D5B` / `#A86F0C` / `#C0392B` | estados |

Raios: `--radius` 12–14px (cartões), `--radius-sm` 8px (botões, campos). Sombras só em elementos flutuantes (menus, modal). Transições de 150 ms em cor, fundo e borda.

### Cores das classes de palavra (padrão)

| Classe | IPADIC | Escuro | Claro |
|---|---|---|---|
| Substantivo | 名詞 (inclui pronome, número) | `#7CC4E8` | `#2F5FB3` |
| Verbo | 動詞 | `#F0937D` | `#B2452E` |
| Partícula | 助詞 | `#9A9CA5` | `#8A8780` |
| Adjetivo/advérbio | 形容詞, 形容動詞 (名詞-形容動詞語幹), 副詞, 連体詞 | `#7FCBA4` | `#2E7D5B` |
| Auxiliar | 助動詞 | `#C8A2E0` | `#8E5BA8` |
| Outro | 記号, 感動詞, 接続詞, 接頭詞, フィラー | cor do texto | cor do texto |

O usuário muda cada cor clicando na bolinha da legenda do detalhamento (ou em *Configurações › Aparência*). O seletor tem 8 cores sugeridas, um campo de cor livre e **Restaurar padrão**. Claro e escuro guardam cores separadas. Um interruptor desliga as cores (a frase fica na cor do texto).

### Tipografia

- Interface: `'Segoe UI Variable Text', 'Segoe UI', system-ui, Roboto, sans-serif`.
- Japonês (frases, vocabulário, dicionário): `'Noto Serif JP', 'Yu Mincho', serif`, em tamanho ajustável (*Aparência › Tamanho do japonês*: P/M/G = 18/21/24px).
- Furigana a 0,5em, na cor `--muted`.

### Componentes

- **Botão principal** (`--accent`), **ghost** (borda, fundo `--surface-2` no hover), **ícone** (32×32, cor `--muted`; ativo em `--accent` sobre `--accent-soft`) e **pílula** (ícone + texto, mesmo estado ativo). No máximo um botão principal por área.
- **Interruptor** no lugar de checkbox, **controle segmentado** no lugar de `<select>` quando há até 4 opções.
- **Tag** (pílula 11px) para origem (Tatoeba/IA), classe gramatical e estados.
- Todo botão de ícone tem `aria-label` e `title`.
- Módulo `ui/icons.js`: `icon(name)` devolve o `<svg>` com os paths (volume, chevron, check, plus, copy, sparkles, search, download, upload, trash, settings/sliders, book, file, message, moon, sun, info, x, refresh, alert).

## 2. Navegação

- **Desktop:** barra lateral estreita (64px) com o logo 日本 e ícones de Frases, Estudar e Texto; Configurações embaixo. Tooltip com o nome.
- **Celular (< 600px):** a mesma navegação vira uma barra de ícones fixa embaixo, respeitando a área segura.
- O texto longo de status do topo ("248,943 frases · dicionário ok · IA…") sai. No lugar, um indicador na barra lateral: verde quando tudo está pronto, amarelo quando falta algo opcional, vermelho quando falta algo necessário para a aba atual. Clicar leva à seção das Configurações que falta.

## 3. Aba Frases

- **Busca** em uma linha: campo com ícone de lupa, **Gerar com IA** (ghost) e **Buscar** (principal). Quantidade de frases e "completar com IA" saem da tela e viram configurações (*IA*).
- **Dicionário** em uma linha: palavra, leitura, tag da classe (ex.: `vs`) e o primeiro significado; `+N` expande os outros sentidos.
- **Cartão da frase:**
  - frase com furigana e cores por classe;
  - tradução em `--muted`;
  - ações: **Ouvir** (ícone), **Detalhes** (pílula, abre/fecha), **Selecionar** (ícone check/plus); tag de origem à direita;
  - selecionado: borda `--ring` e anel de 3px.
- **Detalhamento** (dentro do cartão, fundo `--surface-2`):
  - cabeçalho "Vocabulário" com **Traduzir com IA** e **Copiar**;
  - linhas: forma de dicionário (na cor da classe), leitura (só se tiver kanji), significado;
  - legenda clicável das cores;
  - **Copiar** gera texto simples, sem colchetes: a frase, a tradução, uma linha em branco e `palavra (leitura) = significado` por linha.
- **Barra de seleção** fixa no rodapé, só quando há frase marcada: contador, deck de destino, **Exportar ▾** (Enviar ao Anki, Salvar .apkg, Salvar PDF, com as opções "incluir áudio" e "furigana no PDF" dentro do menu) e **Adicionar ao deck** (principal). Ela substitui o painel lateral "Selecionadas".

## 4. Configurações

Lista de seções à esquerda (no celular, a lista é a primeira tela e cada seção abre em tela própria, com voltar). Cada item tem um ponto de estado (verde/amarelo).

| Seção | Conteúdo |
|---|---|
| **Aparência** | Tema (Sistema/Claro/Escuro), cores por classe (liga/desliga + cor de cada uma), tamanho do japonês. |
| **IA** | Provedor (segmentado: Local / Claude / Desligada); modelos com estado e **Baixar**/**Apagar** e progresso; chave do Claude com link para criar e **Testar**; "completar com IA" e frases por busca (3/5/10). *Avançado* recolhido: threads, nível JLPT, idioma das traduções. |
| **Frases e dicionário** | Tatoeba e JMdict com estado ("248.943 frases") e **Atualizar**; idiomas de tradução. Progresso numa barra; o log detalhado fica num "ver detalhes" recolhido. |
| **Voz** | Voz embutida ou VOICEVOX aberto; termos resumidos em uma linha + "ler termos" e aceite; pacotes de voz com **Baixar**/**Apagar**; voz padrão. |
| **Anki** | Endereço do AnkiConnect, **Testar conexão** com o resultado; quando falha, o passo a passo (abrir o Anki; *Ferramentas › Complementos › Obter complementos*; código `2055492159` com botão copiar; reiniciar o Anki). Só no desktop. |
| **Backup** | Salvar e restaurar o estudo. |
| **Sobre** | Versão, créditos, licenças. |

## 5. Primeiros passos e configuração fácil

Para quem baixa do GitHub:

- **Primeiros passos** (primeira abertura) com um botão **Instalar o recomendado**: baixa em sequência frases, dicionário, uma voz (0.vvm) e um modelo de IA escolhido pela memória do aparelho (**E4B** com 16 GB de RAM ou mais, **E2B** abaixo disso; no Android sempre E2B). Mostra o total antes de começar e uma barra de progresso única, com pausar e retomar (os downloads já retomam de onde pararam).
- Opção **Escolher eu mesmo**, que abre as mesmas seções das Configurações em sequência (como hoje).
- **Usar o Claude em vez de baixar a IA**: campo para a chave, link para o console, **Testar**.
- Qualquer aviso de "falta configurar" (ex.: clicar em Gerar com IA sem modelo) abre um **modal de configuração rápida** daquele item, com o tamanho e um botão de ação, em vez de mandar o usuário procurar nas Configurações.
- Termos das vozes continuam exigindo aceite explícito antes do download.

## 6. Estudar e Texto

Mesmo sistema visual, sem mudar o funcionamento. No Estudar, a frase do card usa as cores por classe e a fonte do japonês; os botões de resposta mantêm as cores de estado. No Texto, o resultado com furigana também usa as cores.

## 7. Backend (Rust)

| Item | Onde | O quê |
|---|---|---|
| Classe da palavra | `jp-core` | `enum WordClass { Noun, Verb, Particle, Adjective, Auxiliary, Other }` a partir do `pos` do IPADIC (tabela da seção 1). `Analyzer::analyze(text) -> Vec<Token>` com `surface`, `base`, `reading`, `segments` (furigana) e `class`. |
| Vocabulário | `engine` | `vocabulary(text) -> Vec<VocabEntry { base, reading, class, gloss }>`: agrupa pela forma de dicionário, ignora `Other` de pontuação, mantém a ordem da frase; `gloss` = primeiros significados do primeiro sentido do JMdict (ou vazio). |
| Exemplos analisados | `engine` | `ExampleView` passa a trazer `tokens: Vec<Token>` além do HTML de furigana, para a tela colorir. |
| Tradução do vocabulário | `engine` + `sentences` | `translate_vocabulary(entries, lang) -> Vec<String>`: um pedido ao provedor configurado com JSON Schema (lista de traduções curtas). Cache em tabela `vocab_gloss(base, reading, lang, gloss)` no `japones.db`. |
| Teste do Anki | `anki` | `AnkiConnect::check() -> Result<u32 /* versão */, AnkiError>`, com `AnkiError::{NotRunning, NoAddon, Incompatible}` distinguidos pela resposta. |
| Memória do aparelho | `engine` | `recommended_model()` com `sysinfo` (MIT) para a RAM total. |
| Configurações | `engine::settings` | `theme: Theme { System, Light, Dark }`, `pos_colors: { light, dark }` (mapa classe → `#rrggbb`), `pos_colors_on: bool`, `jp_font_size: Small/Medium/Large`, `examples_count` (padrão 5) e `fill_with_llm` (padrão sim), que hoje só existem na tela; `llm_provider` ganha o valor `"off"`. Tudo com `#[serde(default)]`, então o `settings.json` antigo continua válido. |
| Comandos Tauri | `app` | `analyze`, `vocabulary`, `translate_vocabulary`, `anki_test`, `recommended_setup`. |

## 8. Estrutura do front-end

```
app/ui/
  index.html        casca: navegação, áreas das abas, modais
  style.css         tokens, componentes, layout
  js/
    main.js         navegação, tema, status, inicialização
    api.js          wrappers de invoke e eventos
    icons.js        ícones SVG
    frases.js       busca, cartões, detalhamento, barra de seleção
    estudar.js
    texto.js
    config.js       seções das Configurações
    setup.js        primeiros passos e modal de configuração rápida
    colors.js       cores por classe, legenda e seletor
```

`index.html` carrega `js/main.js` com `type="module"`. Sem dependências novas de front-end.

## 9. Erros e estados

- AnkiConnect: as mensagens viram texto do app ("O Anki não está aberto", "Falta o add-on AnkiConnect") com o botão do passo a passo; o texto técnico fica num "detalhes".
- Tradução com IA sem provedor: abre o modal de configuração rápida da IA.
- Downloads: progresso, erro com **Tentar de novo**, e o arquivo parcial é mantido para retomar.
- Cores inválidas no `settings.json`: voltam ao padrão sem quebrar a tela.

## 10. Testes

- `jp-core`: classe de cada palavra em frases de exemplo (助詞 → partícula, 名詞-形容動詞語幹 → adjetivo, 助動詞 → auxiliar).
- `engine`: `vocabulary` agrupa conjugações (して/いって → する/行く), ignora pontuação e mantém a ordem; cache da tradução (segunda chamada não consulta o provedor).
- `anki`: `check()` diferencia os três erros (servidor HTTP de teste).
- `settings`: arquivo antigo carrega com os padrões novos; cor inválida volta ao padrão.
- Manual: as quatro abas nos dois temas no Windows e no emulador Android (layout de celular, barra de navegação inferior, seções das Configurações em telas).
- `cargo about` continua sem GPL com o `sysinfo`; ícones Lucide (ISC) nos avisos de terceiros.

## 11. Ordem de entrega

Cada etapa termina em um commit funcionando:

1. Tokens de tema, claro/escuro, ícones e divisão do `app.js` em módulos (visual novo, mesmas telas).
2. Navegação lateral/inferior e Configurações em seções, com o teste do Anki.
3. Aba Frases nova: cartões, cores por classe, detalhamento com o JMdict, barra de seleção.
4. Tradução do vocabulário com IA (com cache) e cores personalizáveis.
5. Instalar o recomendado e modal de configuração rápida.
6. Estudar e Texto no novo estilo; revisão no Android.
