#!/usr/bin/env python3
import argparse
import hashlib
import json
import sys
from pathlib import Path
from typing import Any, Dict, List, Tuple


def fail(message: str) -> None:
    print(f"error: {message}", file=sys.stderr)
    raise SystemExit(2)


def load_json(path: Path) -> Any:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError:
        fail(f"file not found: {path}")
    except json.JSONDecodeError as exc:
        fail(f"invalid JSON in {path}: {exc}")


def normalize_realizations(entry: Dict[str, Any]) -> Dict[str, Dict[str, List[str]]]:
    raw = entry.get("languages")
    if raw is None:
        raw = entry.get("realizations")
    if raw is None:
        return {}
    if not isinstance(raw, dict):
        fail("entry languages/realizations must be an object")

    out: Dict[str, Dict[str, List[str]]] = {}
    for lang, payload in raw.items():
        code = str(lang).strip().lower()
        if not code:
            fail("language code cannot be empty")
        if not isinstance(payload, dict):
            fail(f"language payload for {code} must be an object")

        lemmas_raw = payload.get("lemmas")
        if lemmas_raw is None and payload.get("lemma") is not None:
            lemmas_raw = [payload["lemma"]]

        patterns_raw = payload.get("patterns")
        if patterns_raw is None and payload.get("pattern_templates") is not None:
            patterns_raw = payload["pattern_templates"]

        lemmas = [str(x).strip() for x in (lemmas_raw or []) if str(x).strip()]
        patterns = [str(x).strip() for x in (patterns_raw or []) if str(x).strip()]
        out[code] = {
            "lemmas": sorted(set(lemmas)),
            "patterns": sorted(set(patterns)),
        }

    return dict(sorted(out.items(), key=lambda kv: kv[0]))


def get_entries(lexicon: Any) -> List[Dict[str, Any]]:
    if isinstance(lexicon, list):
        items = lexicon
    elif isinstance(lexicon, dict) and isinstance(lexicon.get("predicate_inventory"), list):
        items = lexicon["predicate_inventory"]
    elif isinstance(lexicon, dict) and isinstance(lexicon.get("entries"), list):
        items = lexicon["entries"]
    else:
        fail("lexicon must be an array, or an object with predicate_inventory/entries")

    normalized: List[Dict[str, Any]] = []
    for item in items:
        if not isinstance(item, dict):
            fail("each entry must be an object")
        canonical = str(item.get("canonical_form", "")).strip()
        if not canonical:
            fail("entry canonical_form is required")
        normalized.append(
            {
                "canonical_form": canonical,
                "planned": bool(item.get("planned", False)),
                "realizations": normalize_realizations(item),
            }
        )

    return sorted(normalized, key=lambda e: e["canonical_form"])


def evaluate(entries: List[Dict[str, Any]], target_lang: str) -> Tuple[Dict[str, Any], bool]:
    target = target_lang.strip().lower()
    if not target:
        fail("target language code cannot be empty")

    duplicates: List[str] = []
    seen = set()

    with_realization: List[str] = []
    planned_missing: List[str] = []
    missing_unplanned: List[str] = []

    for entry in entries:
        canonical = entry["canonical_form"]
        if canonical in seen:
            duplicates.append(canonical)
        else:
            seen.add(canonical)

        lang_payload = entry["realizations"].get(target)
        has_realization = bool(
            lang_payload
            and lang_payload.get("lemmas")
            and lang_payload.get("patterns")
        )

        if has_realization:
            with_realization.append(canonical)
        elif entry["planned"]:
            planned_missing.append(canonical)
        else:
            missing_unplanned.append(canonical)

    realization_surface = [
        {
            "canonical_form": e["canonical_form"],
            "planned": e["planned"],
            "languages": e["realizations"],
        }
        for e in entries
    ]
    realization_blob = json.dumps(realization_surface, sort_keys=True, separators=(",", ":"), ensure_ascii=True)
    realization_hash = hashlib.sha256(realization_blob.encode("utf-8")).hexdigest()

    report = {
        "target_language": target,
        "totals": {
            "predicates": len(entries),
            "with_realization": len(with_realization),
            "planned_missing": len(planned_missing),
            "missing_unplanned": len(missing_unplanned),
            "duplicates": len(duplicates),
        },
        "with_realization": with_realization,
        "planned_missing": planned_missing,
        "missing_unplanned": missing_unplanned,
        "duplicate_canonical_forms": duplicates,
        "realization_hash_sha256": realization_hash,
    }

    ok = not duplicates and not missing_unplanned
    return report, ok


def main() -> None:
    parser = argparse.ArgumentParser(description="Evaluate ULT lexicon coverage for a target language.")
    parser.add_argument("--lexicon", default="docs/ult/ult-lexicon.spec.json", help="Path to lexicon spec/package JSON")
    parser.add_argument("--target-language", required=True, help="Language code to evaluate (e.g. en, es)")
    parser.add_argument("--expected-realization-hash", default="", help="Expected realization hash for stability checks")
    parser.add_argument("--allow-missing-unplanned", action="store_true", help="Allow predicates with missing realizations and planned=false")
    args = parser.parse_args()

    lexicon = load_json(Path(args.lexicon))
    entries = get_entries(lexicon)
    report, ok = evaluate(entries, args.target_language)

    if args.expected_realization_hash:
        if report["realization_hash_sha256"] != args.expected_realization_hash:
            ok = False
            report["hash_mismatch"] = {
                "expected": args.expected_realization_hash,
                "actual": report["realization_hash_sha256"],
            }

    if args.allow_missing_unplanned:
        ok = (
            not report["duplicate_canonical_forms"]
            and "hash_mismatch" not in report
        )

    print(json.dumps(report, indent=2, sort_keys=True, ensure_ascii=True))

    if not ok:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
