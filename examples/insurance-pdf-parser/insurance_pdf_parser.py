#!/usr/bin/env python3
"""Parse a fixed insurance claim PDF or text file into JSON.

The PDF support is intentionally lightweight: it extracts literal strings from
simple PDF content streams, which is enough for fixed-layout generated forms and
keeps this example dependency-free.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path
from typing import Any


PATTERNS = {
    "claim_number": [
        r"\bclaim\s*(?:number|no\.?|#)\s*[:\-]\s*([A-Z0-9][A-Z0-9\-\/]+)",
        r"\bclaim\s+([A-Z]{2,}-[0-9][A-Z0-9\-\/]+)",
    ],
    "patient_name": [
        r"\bpatient\s*(?:name)?\s*[:\-]\s*([A-Z][A-Za-z ,.'-]+)",
        r"\bmember\s*(?:name)?\s*[:\-]\s*([A-Z][A-Za-z ,.'-]+)",
    ],
    "service_date": [
        r"\b(?:service|visit|claim)\s*date\s*[:\-]\s*([0-9]{4}-[0-9]{2}-[0-9]{2})",
        r"\b(?:service|visit|claim)\s*date\s*[:\-]\s*([0-9]{1,2}[\/\-][0-9]{1,2}[\/\-][0-9]{2,4})",
    ],
    "provider": [
        r"\bprovider\s*[:\-]\s*([A-Z][A-Za-z0-9 &.,'-]+)",
        r"\bfacility\s*[:\-]\s*([A-Z][A-Za-z0-9 &.,'-]+)",
    ],
    "total_amount": [
        r"\b(?:total|amount due|claim total|billed amount)\s*[:\-]?\s*\$?\s*([0-9,]+(?:\.[0-9]{2})?)",
    ],
}


def decode_pdf_literal(raw: str) -> str:
    """Decode a simple PDF literal string."""
    replacements = {
        r"\(": "(",
        r"\)": ")",
        r"\\": "\\",
        r"\n": "\n",
        r"\r": "\r",
        r"\t": "\t",
    }
    value = raw
    for old, new in replacements.items():
        value = value.replace(old, new)
    return value


def extract_text_from_pdf_bytes(data: bytes) -> str:
    """Extract text from simple PDF literal-string drawing operators."""
    source = data.decode("latin-1", errors="ignore")
    literal_pattern = re.compile(r"\((?:\\.|[^\\()])*\)\s*Tj")
    array_pattern = re.compile(r"\[((?:\s*\((?:\\.|[^\\()])*\)\s*-?\d*)+)\]\s*TJ")
    chunks: list[str] = []

    for match in literal_pattern.finditer(source):
        literal = match.group(0).rsplit(")", 1)[0][1:]
        chunks.append(decode_pdf_literal(literal))

    for match in array_pattern.finditer(source):
        for literal in re.findall(r"\((?:\\.|[^\\()])*\)", match.group(1)):
            chunks.append(decode_pdf_literal(literal[1:-1]))

    if chunks:
        return "\n".join(chunks)
    return source


def read_input(path: Path) -> str:
    data = path.read_bytes()
    if data.startswith(b"%PDF"):
        return extract_text_from_pdf_bytes(data)
    return data.decode("utf-8", errors="replace")


def clean_value(value: str) -> str:
    return re.sub(r"\s+", " ", value).strip(" .,\t\r\n")


def first_match(text: str, patterns: list[str]) -> str | None:
    for pattern in patterns:
        match = re.search(pattern, text, flags=re.IGNORECASE | re.MULTILINE)
        if match:
            return clean_value(match.group(1))
    return None


def parse_amount(value: str | None) -> float | None:
    if not value:
        return None
    try:
        return float(value.replace(",", ""))
    except ValueError:
        return None


def extract_claim(text: str) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for field, patterns in PATTERNS.items():
        result[field] = first_match(text, patterns)

    result["total_amount"] = parse_amount(result.get("total_amount"))
    result["currency"] = "USD" if "$" in text or result.get("total_amount") is not None else None
    return {key: value for key, value in result.items() if value not in (None, "")}


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Extract fixed claim PDF fields into JSON.")
    parser.add_argument("input", type=Path, help="PDF or text file to parse.")
    parser.add_argument("--out", type=Path, help="Optional path to write JSON output.")
    parser.add_argument("--expect", type=Path, help="Optional expected JSON for smoke testing.")
    return parser.parse_args(argv)


def main(argv: list[str]) -> int:
    args = parse_args(argv)
    result = extract_claim(read_input(args.input))
    output = json.dumps(result, indent=2, sort_keys=True)

    if args.out:
        args.out.write_text(output + "\n", encoding="utf-8")
    else:
        print(output)

    if args.expect:
        expected = json.loads(args.expect.read_text(encoding="utf-8"))
        if result != expected:
            print("Parsed JSON did not match expected fixture.", file=sys.stderr)
            print("Expected:", json.dumps(expected, indent=2, sort_keys=True), file=sys.stderr)
            print("Actual:", output, file=sys.stderr)
            return 1
        print("Smoke test passed.", file=sys.stderr)

    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
