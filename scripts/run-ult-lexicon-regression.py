#!/usr/bin/env python3
import argparse
import json
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any, Dict, Tuple


def fail(message: str) -> None:
    print(f"error: {message}", file=sys.stderr)
    raise SystemExit(2)


def load_json(path: Path) -> Dict[str, Any]:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError:
        fail(f"file not found: {path}")
    except json.JSONDecodeError as exc:
        fail(f"invalid JSON in {path}: {exc}")


def run_builder(builder: Path, spec: Path, case_input: Any) -> Tuple[Dict[str, Any], str]:
    with tempfile.TemporaryDirectory(prefix="ult-lexicon-regression-") as tmpdir:
        tmp = Path(tmpdir)
        input_path = tmp / "input.json"
        input_path.write_text(json.dumps(case_input, sort_keys=True, ensure_ascii=True), encoding="utf-8")

        proc = subprocess.run(
            [
                sys.executable,
                str(builder),
                "--input",
                str(input_path),
                "--spec",
                str(spec),
            ],
            check=False,
            capture_output=True,
            text=True,
        )

        if proc.returncode != 0:
            return {}, f"builder failed: {proc.stderr.strip() or proc.stdout.strip()}"

        try:
            actual = json.loads(proc.stdout)
        except json.JSONDecodeError as exc:
            return {}, f"builder output is not valid JSON: {exc}"

        return actual, ""


def has_target_realization(entry: Dict[str, Any], target_language: str) -> bool:
    payload = entry.get("languages", {}).get(target_language)
    if not isinstance(payload, dict):
        return False
    lemmas = payload.get("lemmas")
    patterns = payload.get("patterns")
    return bool(isinstance(lemmas, list) and lemmas and isinstance(patterns, list) and patterns)


def validate_output_gates(case_id: str, output: Dict[str, Any], target_language: str) -> str:
    entries = output.get("entries")
    if not isinstance(entries, list):
        return "builder output must include an 'entries' array"

    seen = set()
    duplicates = []
    missing_unplanned = []

    for entry in entries:
        if not isinstance(entry, dict):
            return "each output entry must be an object"

        canonical = str(entry.get("canonical_form", "")).strip()
        if not canonical:
            return "each output entry must include canonical_form"

        if canonical in seen:
            duplicates.append(canonical)
        else:
            seen.add(canonical)

        planned = bool(entry.get("planned", False))
        if not has_target_realization(entry, target_language) and not planned:
            missing_unplanned.append(canonical)

    if duplicates:
        return f"duplicate canonical_form values: {sorted(set(duplicates))}"

    if missing_unplanned:
        return (
            "predicates missing target-language realizations without planned=true: "
            f"{sorted(set(missing_unplanned))}"
        )

    audit = output.get("audit")
    if not isinstance(audit, dict) or not str(audit.get("realization_hash_sha256", "")).strip():
        return "builder output must include audit.realization_hash_sha256"

    return ""


def main() -> None:
    parser = argparse.ArgumentParser(description="Run fixed ULT lexicon regression corpus.")
    parser.add_argument("--corpus", default="tests/conformance/ult_lexicon_regression.json", help="Path to regression corpus")
    parser.add_argument("--spec", default="docs/ult/ult.spec.json", help="Path to ULT spec package")
    parser.add_argument("--builder", default="scripts/build-ult-lexicon.py", help="Path to lexicon builder script")
    args = parser.parse_args()

    corpus_path = Path(args.corpus)
    spec_path = Path(args.spec)
    builder_path = Path(args.builder)

    corpus = load_json(corpus_path)
    cases = corpus.get("cases")
    if not isinstance(cases, list):
        fail("corpus must contain a 'cases' array")

    print("ULT lexicon regression")
    print(f"  corpus: {corpus_path}")
    print(f"  spec: {spec_path}")
    print(f"  builder: {builder_path}")
    print(f"  cases: {len(cases)}")

    passed = 0
    failed = 0

    for case in cases:
        case_id = case.get("case_id", "unknown_case")
        case_input = case.get("input")
        expected_output = case.get("expected_output")
        target_language = str(case.get("target_language", "en")).strip().lower() or "en"

        if case_input is None or expected_output is None:
            print(f"\n[FAIL] {case_id}")
            print("  reason: case must include 'input' and 'expected_output'")
            failed += 1
            continue

        actual_output, error = run_builder(builder_path, spec_path, case_input)
        if error:
            print(f"\n[FAIL] {case_id}")
            print(f"  reason: {error}")
            failed += 1
            continue

        gate_error = validate_output_gates(case_id, actual_output, target_language)
        if gate_error:
            print(f"\n[FAIL] {case_id}")
            print(f"  reason: gate violation: {gate_error}")
            failed += 1
            continue

        if actual_output == expected_output:
            print(f"\n[PASS] {case_id}")
            passed += 1
        else:
            print(f"\n[FAIL] {case_id}")
            print("  reason: actual output differs from expected output")
            print("  expected:")
            print(json.dumps(expected_output, indent=2, sort_keys=True, ensure_ascii=True))
            print("  actual:")
            print(json.dumps(actual_output, indent=2, sort_keys=True, ensure_ascii=True))
            failed += 1

    print("\nSummary")
    print(f"  passed: {passed}")
    print(f"  failed: {failed}")

    if failed > 0:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
