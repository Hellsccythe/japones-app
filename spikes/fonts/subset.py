"""Subsets Noto Serif JP for embedding in the PDF export.

Keeps JIS X 0208 (kana + 6,355 kanji, which covers the jōyō list), the 2010
jōyō kanji outside it, Latin for Portuguese/English translations, and common
punctuation. Characters outside the subset fall back to system fonts.

    pip install fonttools
    curl -LO https://github.com/notofonts/noto-cjk/raw/main/Serif/SubsetOTF/JP/NotoSerifJP-Regular.otf
    curl -LO https://github.com/notofonts/noto-cjk/raw/main/Serif/SubsetOTF/JP/NotoSerifJP-Bold.otf
    python subset.py <dir with the .otf files> crates/pdf/fonts
"""

import sys
from pathlib import Path

from fontTools import subset


def jis0208() -> set[int]:
    chars = set()
    for hi in range(0xA1, 0xFF):
        for lo in range(0xA1, 0xFF):
            try:
                chars.add(ord(bytes([hi, lo]).decode("euc_jp")))
            except UnicodeDecodeError:
                pass
    return chars


RANGES = [
    (0x0020, 0x007E),  # ASCII
    (0x00A0, 0x024F),  # Latin-1, Latin Extended-A/B
    (0x2000, 0x206F),  # general punctuation
    (0x2190, 0x21FF),  # arrows
    (0x2460, 0x24FF),  # circled numbers
    (0x25A0, 0x25FF),  # shapes
    (0x3000, 0x30FF),  # CJK punctuation, hiragana, katakana
    (0x31F0, 0x31FF),  # small katakana
    (0xFF00, 0xFFEF),  # full-width forms
]

EXTRA = "𠮟剝塡頰々〆ヶ〜・"


def main(src: Path, dst: Path) -> None:
    chars = jis0208() | {ord(c) for c in EXTRA}
    for a, b in RANGES:
        chars.update(range(a, b + 1))
    dst.mkdir(parents=True, exist_ok=True)
    for weight in ("Regular", "Bold"):
        opts = subset.Options()
        opts.layout_features = ["*"]  # keep vert, ruby, kern, palt
        opts.name_IDs = ["*"]
        opts.name_languages = ["*"]
        opts.notdef_outline = True
        opts.hinting = False
        font = subset.load_font(str(src / f"NotoSerifJP-{weight}.otf"), opts)
        sub = subset.Subsetter(opts)
        sub.populate(unicodes=chars)
        sub.subset(font)
        out = dst / f"NotoSerifJP-{weight}.otf"
        subset.save_font(font, str(out), opts)
        print(out, out.stat().st_size // 1024, "KB")


if __name__ == "__main__":
    main(Path(sys.argv[1]), Path(sys.argv[2]))
