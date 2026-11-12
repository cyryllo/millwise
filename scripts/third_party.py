#!/usr/bin/env python3
"""Generuje THIRD-PARTY.md — listę bibliotek wbudowanych w Millwise z ich licencjami.

Użycie (z katalogu projektu):  python3 scripts/third_party.py
"""
import json
import subprocess

meta = json.loads(subprocess.check_output(["cargo", "metadata", "--format-version", "1", "--locked"]))
ws = set(meta["workspace_members"])
nodes = {n["id"]: n for n in meta["resolve"]["nodes"]}
pkgs = {p["id"]: p for p in meta["packages"]}

# tylko zależności programu (bez dev/build-only) — przejście po grafie od pakietu millwise
root = next(i for i in ws if pkgs[i]["name"] == "millwise")
seen, stack = set(), [root]
while stack:
    i = stack.pop()
    if i in seen:
        continue
    seen.add(i)
    for d in nodes[i]["deps"]:
        if any(k["kind"] in (None, "normal") for k in d["dep_kinds"]):
            stack.append(d["pkg"])

rows = sorted(
    {(pkgs[i]["name"], pkgs[i]["version"], pkgs[i].get("license") or "zob. repozytorium", pkgs[i].get("repository") or "")
     for i in seen if i not in ws}
)
out = [
    "# Biblioteki zewnętrzne",
    "",
    "Millwise jest rozpowszechniany na licencji GPL-3.0-or-later (plik `LICENSE`).",
    "Program zawiera poniższe biblioteki open source; każda zachowuje własną licencję.",
    "Pełne teksty licencji znajdują się w repozytoriach bibliotek.",
    "",
    "Wbudowane czcionki interfejsu (egui / epaint_default_fonts): Ubuntu Font Licence 1.0,",
    "SIL Open Font License 1.1 (Hack, Noto Emoji, emoji-icon-font).",
    "",
    f"Liczba bibliotek: {len(rows)}",
    "",
    "| Biblioteka | Wersja | Licencja | Źródło |",
    "|---|---|---|---|",
]
for name, ver, lic, repo in rows:
    out.append(f"| {name} | {ver} | {lic} | {repo} |")
open("THIRD-PARTY.md", "w", encoding="utf-8").write("\n".join(out) + "\n")
print(f"THIRD-PARTY.md: {len(rows)} bibliotek")
