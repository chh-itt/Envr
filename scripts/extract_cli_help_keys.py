"""Extract tr("key", zh, en) from cli_help.rs for locale sync."""
import re
import sys
from pathlib import Path


def toml_escape(s: str) -> str:
    return s.replace("\\", "\\\\").replace('"', '\\"')


def main() -> None:
    root = Path(__file__).resolve().parents[1]
    path = root / "crates/envr-cli/src/cli_help.rs"
    t = path.read_text(encoding="utf-8")
    # Trailing comma before `)` is common: `tr( "a", "b", "c", )`
    pat = re.compile(
        r'tr\s*\(\s*"([^"]+)"\s*,\s*"((?:[^"\\]|\\.)*)"\s*,\s*"((?:[^"\\]|\\.)*)"\s*,?\s*\)',
        re.MULTILINE | re.DOTALL,
    )
    by_key: dict[str, tuple[str, str]] = {}
    for k, z, e in pat.findall(t):
        by_key[k] = (z, e)
    if "--emit-toml" in sys.argv:
        zh_lines = ["# auto from cli_help.rs (scripts/extract_cli_help_keys.py)"]
        en_lines = ["# auto from cli_help.rs (scripts/extract_cli_help_keys.py)"]
        for k in sorted(by_key):
            z, e = by_key[k]
            zh_lines.append(f'{k} = "{toml_escape(z)}"')
            en_lines.append(f'{k} = "{toml_escape(e)}"')
        (root / "locales/_cli_help_zh.toml").write_text(
            "\n".join(zh_lines) + "\n", encoding="utf-8"
        )
        (root / "locales/_cli_help_en.toml").write_text(
            "\n".join(en_lines) + "\n", encoding="utf-8"
        )
        print(f"wrote locales/_cli_help_zh.toml + _cli_help_en.toml ({len(by_key)} keys)")
        return
    print(f"{len(by_key)} keys from {path.relative_to(root)}")
    for k in sorted(by_key):
        z, e = by_key[k]
        print(f"{k}\n  zh: {z!r}\n  en: {e!r}")


if __name__ == "__main__":
    main()
