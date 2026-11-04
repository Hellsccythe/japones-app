# Changelog

As notas de cada versão saem daqui para a página de releases do GitHub
(a seção `## [x.y.z]` que bate com a tag `vx.y.z`).

## [Não lançado]

Interface nova (v2).

- **Visual novo** com tema claro e escuro (ou o do sistema), ícones de linha e navegação lateral no PC / barra inferior no celular. Um indicador mostra se falta configurar algo e leva à seção certa.
- **Configurações em seções** (Aparência, IA, Frases e dicionário, Voz, Anki, Backup e Sobre), salvas automaticamente.
- **Aba Frases**: busca em uma linha, dicionário resumido com `+N` sentidos, cartões com **cores por classe de palavra** e **Detalhes** com o vocabulário da frase (forma de dicionário, leitura e significado). **Copiar** gera texto simples, sem colchetes.
- **Traduzir com IA** os significados do vocabulário para o idioma das traduções, com cache no aparelho.
- **Cores personalizáveis**: clique numa bolinha da legenda (ou em Aparência) para trocar a cor, com 8 sugestões, cor livre e **Restaurar padrão**; claro e escuro guardam cores separadas.
- **Barra de seleção** no rodapé: deck, **Exportar ▾** (Anki, `.apkg`, PDF) e **Adicionar ao deck**.
- **Anki**: **Testar conexão** e mensagens claras ("O Anki não está aberto", "Falta o add-on AnkiConnect") com o passo a passo e a opção de salvar `.apkg`.
- **Primeiros passos**: **Instalar o recomendado** (frases, dicionário, voz e o modelo de IA certo para a memória do aparelho) com uma barra só, pausar e continuar; ou **Usar o Claude** com teste da chave. Quando falta algo (IA, voz, frases), abre uma **configuração rápida** daquele item.
- **Instalador do Windows** agora leva o runtime do Visual C++: o app abre em um Windows que não tem o Redistributable.
- **Downloads** continuam sozinhos quando a conexão cai; se ela não voltar, uma mensagem clara explica que o que já baixou está salvo.
- **Estudar** e **Texto** com as cores por classe e a fonte do japonês; botões de resposta nas cores de estado.

## [0.1.0]

Primeira versão, para Windows (instalador `.exe`) e Android (`.apk`).

- **Frases de exemplo** para qualquer palavra: frases reais do Tatoeba (acha todas as conjugações) e frases geradas por IA local (Gemma 4, offline) ou pelo Claude (online, opcional).
- **Furigana** automática, com PDF das frases ou de um texto livre (fonte Noto Serif JP embutida).
- **Áudio** com vozes VOICEVOX embutidas, offline, com o crédito de cada voz nos cards.
- **Estudo dentro do app**: decks com revisão espaçada FSRS, limites diários, atalhos de teclado e layout de celular.
- **Anki**: envio pelo AnkiConnect (PC), exportação `.apkg` com progresso e histórico de revisões, e importação de `.apkg`.
- **Backup** dos decks num arquivo, para levar o estudo entre PC e celular.
- **Primeiros passos** guiados, mostrando o tamanho de cada download.
- Dicionário JMdict com significados e tipo do verbo.
