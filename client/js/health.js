// What the app is missing, as a traffic light for the navigation.

/**
 * Red when the Frases tab can't work (no sentences and no AI), yellow when an
 * optional piece is missing (dictionary, voice, AI), green otherwise.
 * `section` is the settings section that fixes the first missing piece.
 */
export function health(s) {
  const missing = {
    frases: !s.tatoeba_indexed || !s.dictionary_loaded,
    voz: !s.voice_ready,
    ia: !s.llm_ready,
  };
  if (!s.tatoeba_indexed && !s.llm_ready) return { level: 'bad', section: 'frases', missing };
  const section = Object.keys(missing).find((k) => missing[k]) ?? null;
  return { level: section ? 'warn' : 'ok', section, missing };
}
