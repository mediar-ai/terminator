# Insurance PDF Parser

Example for the InsurancePDFParser bounty in issue #24.

It parses a fixed insurance claim PDF into structured JSON. The example keeps the
workflow intentionally deterministic so it can be used as a building block for a
Terminator copilot flow:

1. Read a claim PDF or already-extracted claim text.
2. Extract a small fixed set of claim fields.
3. Export normalized JSON for downstream UI automation.

The included sample PDF is a tiny fixed-layout claim document generated as a
plain PDF content stream. The parser also accepts `.txt` input so the extraction
logic can be tested without a PDF renderer.

## Fields

- `claim_number`
- `patient_name`
- `service_date`
- `provider`
- `total_amount`
- `currency`

## Run

From the repository root:

```bash
python examples/insurance-pdf-parser/insurance_pdf_parser.py \
  examples/insurance-pdf-parser/sample_claim.pdf
```

Write JSON to a file:

```bash
python examples/insurance-pdf-parser/insurance_pdf_parser.py \
  examples/insurance-pdf-parser/sample_claim.pdf \
  --out examples/insurance-pdf-parser/sample_claim.output.json
```

Run the smoke test:

```bash
python examples/insurance-pdf-parser/insurance_pdf_parser.py \
  examples/insurance-pdf-parser/sample_claim.pdf \
  --expect examples/insurance-pdf-parser/expected_claim.json
```

## Why This Shape

This example is meant to be a small copilot step, not a fully autonomous claims
system. A human can review the extracted JSON before Terminator uses it to fill
another desktop or web UI.

For real claim templates, add field-specific regexes in `PATTERNS` and save a
known-good expected JSON fixture for each supported layout.
