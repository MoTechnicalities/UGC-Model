#!/usr/bin/env python3
import argparse
import json
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Dict, List


def run_json_command(cmd: List[str]) -> Dict[str, Any]:
    proc = subprocess.run(cmd, check=False, capture_output=True, text=True)
    if proc.returncode != 0:
        message = proc.stderr.strip() or proc.stdout.strip() or f"command failed: {' '.join(cmd)}"
        raise RuntimeError(message)

    try:
        payload = json.loads(proc.stdout)
    except json.JSONDecodeError as exc:
        raise RuntimeError(f"invalid JSON from {' '.join(cmd)}: {exc}") from exc

    if not isinstance(payload, dict):
        raise RuntimeError(f"unexpected non-object payload from {' '.join(cmd)}")

    return payload


def md_line(items: List[str]) -> str:
    return "| " + " | ".join(items) + " |"


def bool_text(value: Any) -> str:
    return "true" if bool(value) else "false"


def suite_markdown(report: Dict[str, Any]) -> str:
    summary = report["summary"]
    outputs = report["outputs"]

    deutsch = report["deutsch"]
    deutsch_jozsa = report["deutsch_jozsa"]
    grover_opt = report["grover"]["optimal"]
    grover_sqrt = report["grover"]["sqrt_reference"]
    simon = report["simon"]

    lines: List[str] = []
    lines.append("# UGC Quantum-Analog Suite Report")
    lines.append("")
    lines.append(f"Generated UTC: {report['generated_utc']}")
    lines.append("")
    lines.append("## Command")
    lines.append("")
    lines.append("```bash")
    lines.append(report["invocation"])
    lines.append("```")
    lines.append("")
    lines.append("## Overall Summary")
    lines.append("")
    lines.append(md_line(["Metric", "Value"]))
    lines.append(md_line(["---", "---"]))
    lines.append(md_line(["all_pass", bool_text(summary["all_pass"])]))
    lines.append(md_line(["deterministic_hash_stable", bool_text(summary["deterministic_hash_stable"])]))
    lines.append(md_line(["total_probe_groups", str(summary["total_probe_groups"])]))
    lines.append(md_line(["total_trials", str(summary["total_trials"])]))
    lines.append(md_line(["total_passed", str(summary["total_passed"])]))
    lines.append(md_line(["total_failed", str(summary["total_failed"])]))
    lines.append("")
    lines.append("## Probe Results")
    lines.append("")
    lines.append(md_line(["Probe", "Trials", "Passed", "Failed", "Hash Stable", "Key Signal"]))
    lines.append(md_line(["---", "---:", "---:", "---:", "---", "---"]))
    lines.append(
        md_line(
            [
                "Deutsch",
                str(deutsch["summary"]["cases"]),
                str(deutsch["summary"]["passed"]),
                str(deutsch["summary"]["failed"]),
                "n/a",
                "constant/balanced classification",
            ]
        )
    )
    dj_trials = sum(batch["oracle_count"] for batch in deutsch_jozsa["batches"])
    dj_passed = sum(batch["summary"]["passed"] for batch in deutsch_jozsa["batches"])
    dj_failed = sum(batch["summary"]["failed"] for batch in deutsch_jozsa["batches"])
    dj_stable = all(batch["summary"]["stability_hash_stable"] for batch in deutsch_jozsa["batches"])
    lines.append(md_line(["Deutsch-Jozsa (n=3,4)", str(dj_trials), str(dj_passed), str(dj_failed), bool_text(dj_stable), "oracle scaling checks"]))

    grover_trials = len(grover_opt["trials"])
    grover_passed = sum(1 for t in grover_opt["trials"] if t["result"]["correct"])
    grover_failed = grover_trials - grover_passed
    lines.append(
        md_line(
            [
                "Grover (n=20 optimal)",
                str(grover_trials),
                str(grover_passed),
                str(grover_failed),
                bool_text(grover_opt["hash_stable"]),
                f"iter={grover_opt['trials'][0]['grover_iterations']}, p={grover_opt['trials'][0]['result']['success_probability']}",
            ]
        )
    )
    lines.append(
        md_line(
            [
                "Simon (n=8)",
                str(simon["summary"]["trials"]),
                str(simon["summary"]["passed"]),
                str(simon["summary"]["failed"]),
                bool_text(simon["hash_stable"]),
                "hidden period recovery",
            ]
        )
    )
    lines.append("")
    lines.append("## Grover n=20 Highlights")
    lines.append("")
    gt = grover_opt["trials"][0]
    lines.append(md_line(["Field", "Value"]))
    lines.append(md_line(["---", "---"]))
    lines.append(md_line(["search_space", str(gt["search_space"])]))
    lines.append(md_line(["grover_iterations", str(gt["grover_iterations"])]))
    lines.append(md_line(["pi_over_4_sqrt_n_reference", str(gt["pi_over_4_sqrt_n_reference"])]))
    lines.append(md_line(["sqrt_n_reference", str(gt["sqrt_n_reference"])]))
    lines.append(md_line(["success_probability", str(gt["result"]["success_probability"])]))
    lines.append(md_line(["phase_operations_equivalent", str(gt["metrics"]["phase_operations_equivalent"])]))
    lines.append(md_line(["peak_memory_bytes_reduced_model", str(gt["metrics"]["peak_memory_bytes_reduced_model"])]))
    lines.append(md_line(["peak_memory_bytes_full_state_equivalent", str(gt["metrics"]["peak_memory_bytes_full_state_equivalent"])]))
    lines.append(md_line(["max_torsion_radians", str(gt["metrics"]["max_torsion_radians"])]))
    lines.append(md_line(["runtime_ms", str(gt["metrics"]["runtime_ms"])]))
    lines.append(md_line(["classical_avg_queries", str(gt["baseline"]["classical_avg_queries"])]))
    lines.append(md_line(["grover_query_ratio_vs_avg", str(gt["baseline"]["grover_query_ratio_vs_avg"])]))
    lines.append("")
    lines.append("## Output Artifacts")
    lines.append("")
    lines.append(f"- JSON report: {outputs['json_report_path']}")
    lines.append(f"- Markdown report: {outputs['markdown_report_path']}")
    lines.append("")
    return "\n".join(lines)


def summarize(report: Dict[str, Any]) -> Dict[str, Any]:
    deutsch = report["deutsch"]
    deutsch_jozsa = report["deutsch_jozsa"]
    grover = report["grover"]
    simon = report["simon"]

    total_trials = 0
    total_passed = 0
    total_failed = 0

    d_trials = deutsch["summary"]["cases"]
    d_passed = deutsch["summary"]["passed"]
    d_failed = deutsch["summary"]["failed"]
    total_trials += d_trials
    total_passed += d_passed
    total_failed += d_failed

    for batch in deutsch_jozsa["batches"]:
        total_trials += batch["oracle_count"]
        total_passed += batch["summary"]["passed"]
        total_failed += batch["summary"]["failed"]

    g_trials = len(grover["optimal"]["trials"])
    g_passed = sum(1 for t in grover["optimal"]["trials"] if t["result"]["correct"])
    g_failed = g_trials - g_passed
    total_trials += g_trials
    total_passed += g_passed
    total_failed += g_failed

    total_trials += simon["summary"]["trials"]
    total_passed += simon["summary"]["passed"]
    total_failed += simon["summary"]["failed"]

    deterministic_hash_stable = (
        all(batch["summary"]["stability_hash_stable"] for batch in deutsch_jozsa["batches"])
        and bool(grover["optimal"]["hash_stable"])
        and bool(simon["hash_stable"])
    )

    return {
        "all_pass": total_failed == 0,
        "deterministic_hash_stable": deterministic_hash_stable,
        "total_probe_groups": 4,
        "total_trials": total_trials,
        "total_passed": total_passed,
        "total_failed": total_failed,
    }


def main() -> None:
    parser = argparse.ArgumentParser(description="Run consolidated UGC quantum-analog suite and emit publish-ready JSON/Markdown reports.")
    parser.add_argument("--python", default=sys.executable, help="Python executable for invoking probe scripts")
    parser.add_argument("--output-json", default="docs/demo/quantum-suite-report.json", help="Output JSON report path")
    parser.add_argument("--output-md", default="docs/demo/quantum-suite-report.md", help="Output Markdown report path")
    parser.add_argument("--pretty", action="store_true", help="Pretty-print JSON report")
    args = parser.parse_args()

    py = args.python

    deutsch = run_json_command([py, "scripts/eval-ugc-deutsch-probe.py"])
    deutsch_jozsa = run_json_command([
        py,
        "scripts/eval-ugc-deutsch-jozsa-probe.py",
        "--n-values",
        "3,4",
        "--stability-runs",
        "3",
    ])
    grover_opt = run_json_command([
        py,
        "scripts/eval-ugc-grover-probe.py",
        "--n-bits",
        "20",
        "--trials",
        "3",
        "--seed",
        "20260606",
        "--iteration-policy",
        "optimal",
        "--trace-every",
        "128",
        "--stability-runs",
        "3",
    ])
    grover_sqrt = run_json_command([
        py,
        "scripts/eval-ugc-grover-probe.py",
        "--n-bits",
        "20",
        "--marked-item",
        "263723",
        "--iteration-policy",
        "sqrt",
        "--trace-every",
        "1024",
        "--stability-runs",
        "1",
    ])
    simon = run_json_command([
        py,
        "scripts/eval-ugc-simon-probe.py",
        "--n-bits",
        "8",
        "--secrets",
        "10101101,01011011,11100010",
        "--stability-runs",
        "3",
    ])

    output_json = Path(args.output_json)
    output_md = Path(args.output_md)
    output_json.parent.mkdir(parents=True, exist_ok=True)
    output_md.parent.mkdir(parents=True, exist_ok=True)

    report: Dict[str, Any] = {
        "object": "ugc.quantum_analog.suite_report",
        "schema_version": "ugc_quantum_suite_report_v1",
        "generated_utc": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "invocation": "python3 scripts/run-ugc-quantum-suite.py --pretty",
        "deutsch": deutsch,
        "deutsch_jozsa": deutsch_jozsa,
        "grover": {
            "optimal": grover_opt,
            "sqrt_reference": grover_sqrt,
        },
        "simon": simon,
        "outputs": {
            "json_report_path": str(output_json),
            "markdown_report_path": str(output_md),
        },
    }

    report["summary"] = summarize(report)

    json_payload = json.dumps(
        report,
        sort_keys=True,
        ensure_ascii=True,
        indent=2 if args.pretty else None,
        separators=None if args.pretty else (",", ":"),
    )
    output_json.write_text(json_payload + ("\n" if args.pretty else ""), encoding="utf-8")

    md_payload = suite_markdown(report)
    output_md.write_text(md_payload + "\n", encoding="utf-8")

    print(
        json.dumps(
            {
                "status": "ok",
                "json_report": str(output_json),
                "markdown_report": str(output_md),
                "summary": report["summary"],
            },
            sort_keys=True,
            ensure_ascii=True,
            indent=2,
        )
    )


if __name__ == "__main__":
    main()
