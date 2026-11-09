// Safe areas (status bar, notch, gesture bar). Android's WebView reports 0 for
// env(safe-area-inset-*), so MainActivity hands the real ones over: read once
// at start-up (window.AndroidInsets.get()) and pushed again when they change
// (window.__setInsets). Elsewhere the CSS falls back to env().

const SIDES = ['top', 'right', 'bottom', 'left'];

/** `{"top":48,...}` (CSS px) → `{'--sa-top': '48px', ...}`; bad values are left out. */
export function insetVars(json) {
  let insets;
  try {
    insets = JSON.parse(json);
  } catch {
    return {};
  }
  const vars = {};
  for (const side of SIDES) {
    const v = insets?.[side];
    if (typeof v === 'number' && Number.isFinite(v) && v >= 0) vars[`--sa-${side}`] = `${v}px`;
  }
  return vars;
}

function apply(json) {
  for (const [name, value] of Object.entries(insetVars(json))) document.documentElement.style.setProperty(name, value);
}

/** Wires the Android bridge; does nothing on other platforms. */
export function init() {
  window.__setInsets = apply;
  if (window.AndroidInsets) apply(window.AndroidInsets.get());
}
