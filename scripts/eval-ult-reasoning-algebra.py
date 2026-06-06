#!/usr/bin/env python3
import argparse
import json
import re
import sys
from pathlib import Path
from typing import Dict, List, Optional, Set, Tuple

SCRIPT_VERSION = "1.0.0"
CORE_FIELDS = [
    "family_id",
    "canonical_form",
    "ontology_tags",
    "dimensionality_inference",
    "boundary_condition_classification",
]

# Minimal partial orders used when the spec does not provide explicit lattices.
FAMILY_PARENTS = {
    "agent_action_location_change": "agent_action",
    "cooperative_action": "agent_action",
    "physical_law": "physical_process",
    "event_causation": "physical_process",
    "machine_state_change": "physical_process",
}
DOMAIN_PARENTS = {
    "narrative_event": "event_like",
    "social_process": "event_like",
    "social_interaction": "event_like",
    "event_time": "temporal_relation",
    "physical_state": "physical_process",
}
BOUNDARY_PARENTS = {
    "explicit_context": "contextualized",
    "implicit_context": "contextualized",
    "conditional_dependency": "contextualized",
    "temporal_dependency": "contextualized",
    "temporal_constraint": "contextualized",
    "causal_dependency": "contextualized",
    "epistemic_context": "contextualized",
    "environmental_constraint": "contextualized",
    "shared_intent": "contextualized",
}

TOP_SENTINELS = {
    "family_id": "family_top",
    "dimensionality_inference": "domain_top",
    "boundary_condition_classification": "contextualized",
}
BOTTOM_SENTINELS = {
    "family_id": "bottom",
    "canonical_form": "bottom_canonical",
    "dimensionality_inference": "bottom",
    "boundary_condition_classification": "bottom",
}

OPPOSED_TAGS = {
    frozenset(("alive", "dead")),
    frozenset(("present_state", "nonexistent")),
    frozenset(("rest_state", "active_motion")),
}

MUTUALLY_EXCLUSIVE_PREDICATES = {
    frozenset(("sleep", "run")),
    frozenset(("alive", "dead")),
    frozenset(("exist", "nonexistent")),
}

VAR_RE = re.compile(r"^[A-Z][A-Za-z0-9_]*$")
PRED_RE = re.compile(r"^\s*([A-Za-z_][A-Za-z0-9_]*)\((.*)\)\s*$")


def fail(msg: str) -> None:
    print(f"error: {msg}", file=sys.stderr)
    raise SystemExit(2)


def load_json(path: Path) -> Dict:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError:
        fail(f"file not found: {path}")
    except json.JSONDecodeError as exc:
        fail(f"invalid JSON in {path}: {exc}")


def normalize_ult(obj: Dict) -> Dict:
    # Accept either a direct ULT record or {"ult": {...}} wrapper.
    ult = obj.get("ult") if isinstance(obj, dict) and "ult" in obj else obj
    if not isinstance(ult, dict):
        fail("ULT input must be an object or contain an 'ult' object")

    missing = [k for k in CORE_FIELDS if k not in ult]
    if missing:
        fail(f"ULT input missing required fields: {', '.join(missing)}")

    out = {k: ult[k] for k in CORE_FIELDS}
    if not isinstance(out["ontology_tags"], list):
        fail("ontology_tags must be an array")

    out["ontology_tags"] = sorted(set(str(t) for t in out["ontology_tags"]))
    for k in CORE_FIELDS:
        if k != "ontology_tags":
            out[k] = str(out[k])
    return out


def ancestors(node: str, parents: Dict[str, str]) -> List[str]:
    chain = [node]
    cur = node
    seen = {node}
    while cur in parents:
        cur = parents[cur]
        if cur in seen:
            break
        chain.append(cur)
        seen.add(cur)
    return chain


def leq(x: str, y: str, parents: Dict[str, str], top: str) -> bool:
    if x == y or y == top:
        return True
    return y in ancestors(x, parents)


def lub(x: str, y: str, parents: Dict[str, str], top: str) -> str:
    if x == y:
        return x
    ax = ancestors(x, parents)
    ay = set(ancestors(y, parents))
    for n in ax:
        if n in ay:
            return n
    return top


def glb(x: str, y: str, parents: Dict[str, str], bottom: str) -> str:
    if x == y:
        return x
    if x in ancestors(y, parents):
        return y
    if y in ancestors(x, parents):
        return x
    return bottom


def split_args(arg_str: str) -> List[str]:
    args: List[str] = []
    depth = 0
    start = 0
    for i, ch in enumerate(arg_str):
        if ch == "(":
            depth += 1
        elif ch == ")":
            depth = max(0, depth - 1)
        elif ch == "," and depth == 0:
            args.append(arg_str[start:i].strip())
            start = i + 1
    tail = arg_str[start:].strip()
    if tail:
        args.append(tail)
    return args


def parse_predicate(canonical: str) -> Optional[Tuple[str, List[str]]]:
    m = PRED_RE.match(canonical)
    if not m:
        return None
    name = m.group(1)
    args = split_args(m.group(2))
    return name, args


def canonical_join(c1: str, c2: str) -> str:
    if c1 == c2:
        return c1
    p1 = parse_predicate(c1)
    p2 = parse_predicate(c2)
    if p1 and p2 and p1[0] == p2[0] and len(p1[1]) == len(p2[1]):
        vars_ = [f"X{i + 1}" for i in range(len(p1[1]))]
        return f"{p1[0]}({','.join(vars_)})"
    return f"mgu({c1} || {c2})"


def is_instance_of(instance_cf: str, template_cf: str) -> bool:
    pi = parse_predicate(instance_cf)
    pt = parse_predicate(template_cf)
    if not pi or not pt:
        return False
    if pi[0] != pt[0] or len(pi[1]) != len(pt[1]):
        return False
    for ai, at in zip(pi[1], pt[1]):
        if VAR_RE.match(at):
            continue
        if ai != at:
            return False
    return True


def canonical_meet(c1: str, c2: str) -> str:
    if c1 == c2:
        return c1
    if is_instance_of(c1, c2):
        return c1
    if is_instance_of(c2, c1):
        return c2
    return BOTTOM_SENTINELS["canonical_form"]


def contradictions(u1: Dict, u2: Dict) -> List[str]:
    reasons: List[str] = []

    for t1 in u1["ontology_tags"]:
        for t2 in u2["ontology_tags"]:
            if frozenset((t1, t2)) in OPPOSED_TAGS:
                reasons.append(f"opposed_ontology_tags:{t1}~{t2}")

    p1 = parse_predicate(u1["canonical_form"])
    p2 = parse_predicate(u2["canonical_form"])
    if p1 and p2 and p1[1] and p2[1]:
        same_subject = p1[1][0] == p2[1][0]
        excl = frozenset((p1[0], p2[0])) in MUTUALLY_EXCLUSIVE_PREDICATES
        if same_subject and excl:
            reasons.append("canonical_form_mutual_exclusion")

    bpair = frozenset(
        (
            u1["boundary_condition_classification"],
            u2["boundary_condition_classification"],
        )
    )
    if bpair == frozenset(("present_state", "nonexistent_state")):
        reasons.append("boundary_conflict")

    if (
        u1["canonical_form"] == u2["canonical_form"]
        and u1["dimensionality_inference"] != u2["dimensionality_inference"]
        and (
            "nonexistent" in u1["dimensionality_inference"]
            or "nonexistent" in u2["dimensionality_inference"]
        )
    ):
        reasons.append("dimensionality_clash")

    return sorted(set(reasons))


def compute_join(u1: Dict, u2: Dict) -> Dict:
    d1 = u1["dimensionality_inference"]
    d2 = u2["dimensionality_inference"]
    # Explicit domain LUB special-case from spec notes.
    if {d1, d2} == {"narrative_event", "social_process"}:
        d_star = "event_like"
    else:
        d_star = lub(d1, d2, DOMAIN_PARENTS, TOP_SENTINELS["dimensionality_inference"])

    b1 = u1["boundary_condition_classification"]
    b2 = u2["boundary_condition_classification"]
    if {b1, b2} == {"explicit_context", "implicit_context"}:
        b_star = "contextualized"
    else:
        b_star = lub(b1, b2, BOUNDARY_PARENTS, TOP_SENTINELS["boundary_condition_classification"])

    return {
        "family_id": lub(
            u1["family_id"],
            u2["family_id"],
            FAMILY_PARENTS,
            TOP_SENTINELS["family_id"],
        ),
        "canonical_form": canonical_join(u1["canonical_form"], u2["canonical_form"]),
        "ontology_tags": sorted(set(u1["ontology_tags"]) | set(u2["ontology_tags"])),
        "dimensionality_inference": d_star,
        "boundary_condition_classification": b_star,
    }


def compute_meet(u1: Dict, u2: Dict, contradiction_reasons: List[str]) -> Dict:
    meet_obj = {
        "family_id": glb(
            u1["family_id"],
            u2["family_id"],
            FAMILY_PARENTS,
            BOTTOM_SENTINELS["family_id"],
        ),
        "canonical_form": canonical_meet(u1["canonical_form"], u2["canonical_form"]),
        "ontology_tags": sorted(set(u1["ontology_tags"]) & set(u2["ontology_tags"])),
        "dimensionality_inference": glb(
            u1["dimensionality_inference"],
            u2["dimensionality_inference"],
            DOMAIN_PARENTS,
            BOTTOM_SENTINELS["dimensionality_inference"],
        ),
        "boundary_condition_classification": glb(
            u1["boundary_condition_classification"],
            u2["boundary_condition_classification"],
            BOUNDARY_PARENTS,
            BOTTOM_SENTINELS["boundary_condition_classification"],
        ),
    }

    bottom = contradiction_reasons or (
        meet_obj["family_id"] == BOTTOM_SENTINELS["family_id"]
        or meet_obj["canonical_form"] == BOTTOM_SENTINELS["canonical_form"]
        or meet_obj["dimensionality_inference"] == BOTTOM_SENTINELS["dimensionality_inference"]
        or meet_obj["boundary_condition_classification"] == BOTTOM_SENTINELS["boundary_condition_classification"]
    )
    return {"is_bottom": bool(bottom), "value": None if bottom else meet_obj}


def entails(u1: Dict, u2: Dict) -> bool:
    family_ok = leq(
        u1["family_id"],
        u2["family_id"],
        FAMILY_PARENTS,
        TOP_SENTINELS["family_id"],
    )
    canonical_ok = is_instance_of(u1["canonical_form"], u2["canonical_form"]) or (
        u1["canonical_form"] == u2["canonical_form"]
    )
    tags_ok = set(u2["ontology_tags"]).issubset(set(u1["ontology_tags"]))
    domain_ok = leq(
        u1["dimensionality_inference"],
        u2["dimensionality_inference"],
        DOMAIN_PARENTS,
        TOP_SENTINELS["dimensionality_inference"],
    )
    boundary_ok = leq(
        u1["boundary_condition_classification"],
        u2["boundary_condition_classification"],
        BOUNDARY_PARENTS,
        TOP_SENTINELS["boundary_condition_classification"],
    )
    return family_ok and canonical_ok and tags_ok and domain_ok and boundary_ok


def compose(u1: Dict, u2: Dict) -> Dict:
    b1 = u1["boundary_condition_classification"]
    b2 = u2["boundary_condition_classification"]
    if "temporal_dependency" in {b1, b2}:
        b_out = "ordered_context"
    elif b1 == b2:
        b_out = b1
    else:
        b_out = "contextualized"

    return {
        "family_id": f"sequence({u1['family_id']}->{u2['family_id']})",
        "canonical_form": f"{u1['canonical_form']} then {u2['canonical_form']}",
        "ontology_tags": sorted(
            set(u1["ontology_tags"]) | set(u2["ontology_tags"]) | {"sequence", "temporal_order"}
        ),
        "dimensionality_inference": lub(
            u1["dimensionality_inference"],
            u2["dimensionality_inference"],
            DOMAIN_PARENTS,
            TOP_SENTINELS["dimensionality_inference"],
        ),
        "boundary_condition_classification": b_out,
    }


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Evaluate ULT reasoning algebra operators for two ULT records."
    )
    parser.add_argument("--u1", required=True, help="Path to first ULT JSON record")
    parser.add_argument("--u2", required=True, help="Path to second ULT JSON record")
    parser.add_argument(
        "--spec",
        default="docs/ult/ult.spec.json",
        help="Path to ULT spec package (default: docs/ult/ult.spec.json)",
    )
    parser.add_argument(
        "--pretty",
        action="store_true",
        help="Pretty-print JSON output instead of compact form",
    )
    args = parser.parse_args()

    spec = load_json(Path(args.spec))
    u1 = normalize_ult(load_json(Path(args.u1)))
    u2 = normalize_ult(load_json(Path(args.u2)))

    contradiction_reasons = contradictions(u1, u2)

    result = {
        "spec_id": spec.get("spec_id"),
        "spec_version": spec.get("spec_version"),
        "evaluator": {
            "name": "ult_reasoning_algebra_evaluator",
            "version": SCRIPT_VERSION,
            "deterministic_output": True,
            "serialization": "json_sort_keys",
        },
        "inputs": {"u1": u1, "u2": u2},
        "results": {
            "join": compute_join(u1, u2),
            "meet": compute_meet(u1, u2, contradiction_reasons),
            "entailment": {
                "u1_entails_u2": entails(u1, u2),
                "u2_entails_u1": entails(u2, u1),
            },
            "contradiction": {
                "value": len(contradiction_reasons) > 0,
                "reasons": contradiction_reasons,
            },
            "composition": {
                "u1_then_u2": compose(u1, u2),
                "u2_then_u1": compose(u2, u1),
            },
        },
    }

    if args.pretty:
        print(json.dumps(result, indent=2, sort_keys=True, ensure_ascii=True))
    else:
        print(json.dumps(result, sort_keys=True, separators=(",", ":"), ensure_ascii=True))


if __name__ == "__main__":
    main()
