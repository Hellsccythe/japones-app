// Friendly text for the AnkiConnect error codes the backend sends.

const MESSAGES = {
  'anki:not_running': 'O Anki não está aberto. Abra o Anki e tente de novo.',
  'anki:no_addon': 'Falta o add-on AnkiConnect no Anki.',
};

/** `help` says whether "Ver como instalar" is worth offering. */
export function ankiMessage(err) {
  const code = String(err);
  if (MESSAGES[code]) return { text: MESSAGES[code], help: true };
  const old = code.match(/^anki:incompatible:(\d+)$/);
  if (old) return { text: `O AnkiConnect instalado (versão ${old[1]}) é antigo. Atualize o add-on.`, help: true };
  return { text: code, help: false };
}
