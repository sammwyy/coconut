#!/usr/bin/env python3
"""Generate Coconut's compact country/subdivision catalog from a pinned CLDR release."""
import json
import pathlib
import urllib.request
import xml.etree.ElementTree as ET

VERSION = "48.2.0"
BASE = f"https://raw.githubusercontent.com/unicode-org/cldr-json/{VERSION}/cldr-json/cldr-localenames-full/main/en"
ROOT = pathlib.Path(__file__).resolve().parents[1]
OUT = ROOT / "crates/core/assets/geo.json"

def fetch(name):
    with urllib.request.urlopen(f"{BASE}/{name}.json") as response:
        return json.load(response)["main"]["en"]["localeDisplayNames"][name]

territories = fetch("territories")
with urllib.request.urlopen("https://raw.githubusercontent.com/unicode-org/cldr/release-48-2/common/subdivisions/en.xml") as response:
    subdivisions = {node.attrib["type"]: node.text for node in ET.fromstring(response.read()).findall(".//subdivision")}
countries = [
    {"code": code, "name": name}
    for code, name in territories.items()
    if len(code) == 2 and code.isalpha() and code != "ZZ"
]
regions = [
    {"code": f"{code[:2].upper()}-{code[2:].upper()}", "country": code[:2].upper(), "name": name}
    for code, name in subdivisions.items()
    if len(code) > 2 and code[:2].isalpha()
]
OUT.parent.mkdir(parents=True, exist_ok=True)
OUT.write_text(json.dumps({"countries": countries, "regions": regions}, ensure_ascii=False, separators=(",", ":")))
