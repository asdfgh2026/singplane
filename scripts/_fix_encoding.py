from pathlib import Path

p = Path(__file__).resolve().parents[1] / "lib" / "core" / "core_downloader.dart"
t = p.read_text(encoding="utf-8", errors="replace")
# Fix known corrupted Chinese error strings (replacement char / truncated quotes)
fixes = {
    "throw StateError('未找\ufffdAlpha 预发\ufffd),": "throw StateError('Alpha prerelease not found');",
}
# Broader: rewrite any line with replacement char in StateError/ArgumentError
out = []
for line in t.splitlines(keepends=True):
    if "\ufffd" in line:
        if "Alpha" in line or "prerelease" in line.lower() or "预发" in line:
            indent = line[: len(line) - len(line.lstrip())]
            line = f"{indent}orElse: () => throw StateError('Alpha prerelease not found'),\n"
            if "orElse" not in line and "throw" in "".join(out[-3:]):
                pass
        elif "未找到匹配" in line or "资源" in line or "Asset" in line:
            # leave for full rewrite below
            pass
        elif "下载地址" in line:
            line = line.split("throw")[0] + "throw StateError('download url empty');\n"
        elif "不支持" in line:
            line = line.split("throw")[0] + "throw StateError('unsupported archive format: ${info.assetName}');\n"
        elif "zip" in line and "未找到" in line:
            line = line.split("throw")[0] + "throw StateError('binary not found in zip');\n"
        elif "tar" in line and "未找到" in line:
            line = line.split("throw")[0] + "throw StateError('binary not found in tar.gz');\n"
        elif "解压" in line:
            line = line.split("throw")[0] + "throw StateError('binary missing after extract: \$targetPath');\n"
        else:
            # generic strip broken Chinese throw
            if "throw StateError(" in line and line.count("'") % 2 == 1:
                indent = line[: len(line) - len(line.lstrip())]
                line = f"{indent}throw StateError('error');\n"
    out.append(line)

text = "".join(out)
# Explicit fix for the orElse line which may be broken differently
import re

text = re.sub(
    r"orElse:\s*\(\)\s*=>\s*throw StateError\([^)]*\n?",
    "orElse: () => throw StateError('Alpha prerelease not found'),\n",
    text,
    count=1,
)
# Fix asset not found multiline
text = re.sub(
    r"throw StateError\(\s*'未找到匹配资源:.*?\);",
    """throw StateError(
        'Asset not found: \$want\\nplatform: \${CorePlatform.platformLabel}\\navailable: \$names',
      );""",
    text,
    flags=re.S,
    count=1,
)
p.write_text(text, encoding="utf-8", newline="\n")
print("fixed core_downloader, replacement chars left:", p.read_text(encoding="utf-8").count("\ufffd"))
