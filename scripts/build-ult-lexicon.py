#!/usr/bin/env python3
import argparse
import hashlib
import json
import sys
from pathlib import Path
from typing import Any, Dict, List, Tuple

DEFAULT_SPEC = "docs/ult/ult.spec.json"


def fail(msg: str) -> None:
    print(f"error: {msg}", file=sys.stderr)
    raise SystemExit(2)


def load_json(path: Path) -> Any:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError:
        fail(f"file not found: {path}")
    except json.JSONDecodeError as exc:
        fail(f"invalid JSON in {path}: {exc}")


def ensure_array_of_strings(value: Any, field_name: str) -> List[str]:
    if not isinstance(value, list) or len(value) == 0:
        fail(f"{field_name} must be a non-empty array")
    normalized = sorted({str(v).strip() for v in value if str(v).strip()})
    if not normalized:
        fail(f"{field_name} must contain at least one non-empty string")
    return normalized


def normalize_languages(value: Any) -> Dict[str, Dict[str, List[str]]]:
    if not isinstance(value, dict) or not value:
        fail("languages must be a non-empty object")

    out: Dict[str, Dict[str, List[str]]] = {}
    for lang, payload in value.items():
        lang_code = str(lang).strip().lower()
        if not lang_code:
            fail("language code cannot be empty")
        if not isinstance(payload, dict):
            fail(f"languages.{lang_code} must be an object")

        lemmas = ensure_array_of_strings(payload.get("lemmas"), f"languages.{lang_code}.lemmas")
        patterns = ensure_array_of_strings(payload.get("patterns"), f"languages.{lang_code}.patterns")
        out[lang_code] = {"lemmas": lemmas, "patterns": patterns}

    return dict(sorted(out.items(), key=lambda kv: kv[0]))


def normalize_realizations(value: Any) -> Dict[str, Dict[str, List[str]]]:
    if value is None:
        return {}
    if not isinstance(value, dict):
        fail("realizations must be an object")

    out: Dict[str, Dict[str, List[str]]] = {}
    for lang, payload in value.items():
        lang_code = str(lang).strip().lower()
        if not lang_code:
            fail("realizations language code cannot be empty")
        if not isinstance(payload, dict):
            fail(f"realizations.{lang_code} must be an object")

        lemma_value = payload.get("lemma")
        lemmas_value = payload.get("lemmas")
        if lemmas_value is None and lemma_value is not None:
            lemmas_value = [lemma_value]

        templates_value = payload.get("pattern_templates")
        patterns_value = payload.get("patterns")
        if patterns_value is None and templates_value is not None:
            patterns_value = templates_value

        if lemmas_value is None or patterns_value is None:
            fail(f"realizations.{lang_code} requires lemma/lemmas and pattern_templates/patterns")

        lemmas = ensure_array_of_strings(lemmas_value, f"realizations.{lang_code}.lemmas")
        patterns = ensure_array_of_strings(patterns_value, f"realizations.{lang_code}.patterns")
        out[lang_code] = {"lemmas": lemmas, "patterns": patterns}

    return dict(sorted(out.items(), key=lambda kv: kv[0]))


def normalize_entry(entry: Any) -> Dict[str, Any]:
    if not isinstance(entry, dict):
        fail("each entry must be an object")

    required = [
        "canonical_form",
        "family_id",
        "ontology_tags",
        "dimensionality_inference",
        "boundary_condition_classification",
    ]
    for key in required:
        if key not in entry:
            fail(f"entry missing required field: {key}")

    canonical_form = str(entry["canonical_form"]).strip()
    family_id = str(entry["family_id"]).strip()
    dimensionality = str(entry["dimensionality_inference"]).strip()
    boundary = str(entry["boundary_condition_classification"]).strip()

    if not canonical_form or not family_id or not dimensionality or not boundary:
        fail("canonical_form, family_id, dimensionality_inference, and boundary_condition_classification must be non-empty")

    ontology_tags = ensure_array_of_strings(entry["ontology_tags"], "ontology_tags")
    planned = bool(entry.get("planned", False))

    if "languages" in entry and "realizations" in entry:
        fail("entry cannot contain both languages and realizations")

    if "languages" in entry:
        languages = normalize_languages(entry["languages"])
    else:
        languages = normalize_realizations(entry.get("realizations"))

    if not planned and not languages:
        fail("predicate without realizations must set planned=true")

    provenance = entry.get("provenance")
    if provenance is not None and not isinstance(provenance, dict):
        fail("provenance must be an object when provided")

    entry_id_seed = f"{canonical_form}|{family_id}"
    entry_id = hashlib.sha256(entry_id_seed.encode("utf-8")).hexdigest()[:16]

    normalized = {
        "entry_id": entry_id,
        "canonical_form": canonical_form,
        "family_id": family_id,
        "ontology_tags": ontology_tags,
        "dimensionality_inference": dimensionality,
        "boundary_condition_classification": boundary,
        "languages": languages,
    }

    if planned:
        normalized["planned"] = True
    if provenance is not None:
        normalized["provenance"] = provenance

    return normalized


def normalize_input(input_json: Any) -> Tuple[List[Dict[str, Any]], Dict[str, Any]]:
    if isinstance(input_json, list):
        raw_entries = input_json
        source_meta: Dict[str, Any] = {}
    elif isinstance(input_json, dict) and isinstance(input_json.get("entries"), list):
        raw_entries = input_json["entries"]
        source_meta = {
            "spec_id": input_json.get("spec_id"),
            "spec_version": input_json.get("spec_version"),
        }
    elif isinstance(input_json, dict) and isinstance(input_json.get("predicate_inventory"), list):
        raw_entries = input_json["predicate_inventory"]
        source_meta = {
            "spec_id": input_json.get("spec_id"),
            "spec_version": input_json.get("spec_version"),
        }
    else:
        fail("input must be an array, an object with 'entries', or an object with 'predicate_inventory'")

    entries = [normalize_entry(e) for e in raw_entries]
    seen = set()
    for e in entries:
        key = e["canonical_form"]
        if key in seen:
            fail(f"duplicate canonical_form in input: {key}")
        seen.add(key)

    entries.sort(key=lambda e: (e["canonical_form"], e["family_id"], e["entry_id"]))

    return entries, source_meta


def build_lexicon(input_entries: List[Dict[str, Any]], ult_spec: Dict[str, Any], source_meta: Dict[str, Any]) -> Dict[str, Any]:
    entries_blob = json.dumps(input_entries, sort_keys=True, separators=(",", ":"), ensure_ascii=True)
    entries_hash = hashlib.sha256(entries_blob.encode("utf-8")).hexdigest()

    realization_surface = [
        {
            "canonical_form": e["canonical_form"],
            "planned": bool(e.get("planned", False)),
            "languages": e["languages"],
        }
        for e in input_entries
    ]
    realization_blob = json.dumps(realization_surface, sort_keys=True, separators=(",", ":"), ensure_ascii=True)
    realization_hash = hashlib.sha256(realization_blob.encode("utf-8")).hexdigest()

    out = {
        "spec_id": "ult_lexicon_package",
        "spec_version": "1.0.0",
        "ult_spec_ref": {
            "spec_id": ult_spec.get("spec_id", "ult_universal_language_tensor_spec"),
            "spec_version": ult_spec.get("spec_version", "unknown"),
        },
        "entry_count": len(input_entries),
        "entries": input_entries,
        "audit": {
            "canonicalization": "json_sort_keys",
            "hash_algorithm": "sha256",
            "entries_sha256": entries_hash,
            "realization_hash_sha256": realization_hash,
        },
    }

    if source_meta.get("spec_id") or source_meta.get("spec_version"):
        out["source_spec_ref"] = {
            "spec_id": source_meta.get("spec_id"),
            "spec_version": source_meta.get("spec_version"),
        }

    return out


def main() -> None:
    parser = argparse.ArgumentParser(description="Build deterministic ULT lexicon package from canonical predicate entries.")
    parser.add_argument("--input", required=True, help="Path to source JSON (entries array or object containing entries)")
    parser.add_argument("--spec", default=DEFAULT_SPEC, help=f"Path to ULT spec package (default: {DEFAULT_SPEC})")
    parser.add_argument("--output", help="Optional output file path; if omitted, prints JSON to stdout")
    parser.add_argument("--pretty", action="store_true", help="Pretty print JSON output")
    args = parser.parse_args()

    source = load_json(Path(args.input))
    ult_spec = load_json(Path(args.spec))

    entries, source_meta = normalize_input(source)
    lexicon = build_lexicon(entries, ult_spec, source_meta)

    payload = json.dumps(
        lexicon,
        sort_keys=True,
        ensure_ascii=True,
        indent=2 if args.pretty else None,
        separators=None if args.pretty else (",", ":"),
    )

    if args.output:
        Path(args.output).write_text(payload + ("\n" if args.pretty else ""), encoding="utf-8")
    else:
        print(payload)


if __name__ == "__main__":
    main()
