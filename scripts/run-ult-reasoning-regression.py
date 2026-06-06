#!/usr/bin/env python3
import argparse
import json
import subprocess
import sys
import tempfile
from pathlib import Path


def fail(message: str) -> None:
    print(f"error: {message}", file=sys.stderr)
    raise SystemExit(2)


def load_json(path: Path):
    try:
      return json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError:
      fail(f"file not found: {path}")
    except json.JSONDecodeError as exc:
      fail(f"invalid JSON in {path}: {exc}")


def run_case(evaluator: Path, spec: Path, case: dict):
    with tempfile.TemporaryDirectory(prefix="ult-regression-") as tmpdir:
        tmp_path = Path(tmpdir)
        u1_path = tmp_path / "u1.json"
        u2_path = tmp_path / "u2.json"
        u1_path.write_text(json.dumps(case["u1"], sort_keys=True), encoding="utf-8")
        u2_path.write_text(json.dumps(case["u2"], sort_keys=True), encoding="utf-8")

        proc = subprocess.run(
            [
                sys.executable,
                str(evaluator),
                "--u1",
                str(u1_path),
                "--u2",
                str(u2_path),
                "--spec",
                str(spec),
            ],
            check=False,
            capture_output=True,
            text=True,
        )
        if proc.returncode != 0:
            return None, f"evaluator failed: {proc.stderr.strip() or proc.stdout.strip()}"

        try:
            actual = json.loads(proc.stdout)
        except json.JSONDecodeError as exc:
            return None, f"evaluator produced invalid JSON: {exc}"

        return actual, None


def main() -> None:
    parser = argparse.ArgumentParser(description="Run fixed ULT reasoning regression corpus.")
    parser.add_argument(
        "--corpus",
        default="tests/conformance/ult_reasoning_regression.json",
        help="Path to regression corpus JSON",
    )
    parser.add_argument(
        "--spec",
        default="docs/ult/ult.spec.json",
        help="Path to ULT spec JSON",
    )
    parser.add_argument(
        "--evaluator",
        default="scripts/eval-ult-reasoning-algebra.py",
        help="Path to evaluator script",
    )
    args = parser.parse_args()

    corpus_path = Path(args.corpus)
    spec_path = Path(args.spec)
    evaluator_path = Path(args.evaluator)

    corpus = load_json(corpus_path)
    if not isinstance(corpus, dict) or not isinstance(corpus.get("cases"), list):
        fail("corpus must be an object with a 'cases' array")

    passed = 0
    failed = 0
    print("ULT reasoning regression")
    print(f"  corpus: {corpus_path}")
    print(f"  spec: {spec_path}")
    print(f"  evaluator: {evaluator_path}")
    print(f"  cases: {len(corpus['cases'])}")

    for case in corpus["cases"]:
        case_id = case.get("case_id", "unknown_case")
        actual, error = run_case(evaluator_path, spec_path, case)
        if error is not None:
            print(f"\n[FAIL] {case_id}")
            print(f"  reason: {error}")
            failed += 1
            continue

        expected = case.get("expected", {})
        actual_results = actual.get("results")
        expected_results = expected.get("results")

        if actual_results == expected_results:
            print(f"\n[PASS] {case_id}")
            passed += 1
            continue

        print(f"\n[FAIL] {case_id}")
        print("  reason: actual results differ from expected results")
        print("  expected:")
        print(json.dumps(expected_results, indent=2, sort_keys=True, ensure_ascii=True))
        print("  actual:")
        print(json.dumps(actual_results, indent=2, sort_keys=True, ensure_ascii=True))
        failed += 1

    print("\nSummary")
    print(f"  passed: {passed}")
    print(f"  failed: {failed}")
    if failed:
        raise SystemExit(1)


if __name__ == "__main__":
    main()