// AI errors that the quick setup can fix (pure: no Tauri here).

/** The backend says the AI is off (`ia:off`) or not usable yet (`ia:not_ready`). */
export function needsAiSetup(err) {
  return /^(Error: )?ia:(off|not_ready)\b/.test(String(err ?? ''));
}
