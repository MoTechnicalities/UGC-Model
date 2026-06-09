#!/usr/bin/env python3
import argparse
import csv
import json
import math
import subprocess
import sys
import time
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Dict, List, Optional, Sequence


def run_summary_command(cmd: List[str], cwd: Path) -> Dict[str, Any]:
    start = time.perf_counter()
    proc = subprocess.run(cmd, cwd=str(cwd), check=False, capture_output=True, text=True)
    elapsed_ms = round((time.perf_counter() - start) * 1000.0, 6)
    if proc.returncode != 0:
        message = proc.stderr.strip() or proc.stdout.strip() or f"command failed: {' '.join(cmd)}"
        raise RuntimeError(message)

    try:
        payload = json.loads(proc.stdout)
    except json.JSONDecodeError as exc:
        raise RuntimeError(f"invalid JSON from {' '.join(cmd)}: {exc}") from exc

    if not isinstance(payload, dict):
        raise RuntimeError("dirac summary command returned non-object JSON")

    payload["_wall_runtime_ms"] = elapsed_ms
    return payload


def svg_escape(text: str) -> str:
    return (
        text.replace("&", "&amp;")
        .replace("<", "&lt;")
        .replace(">", "&gt;")
        .replace('"', "&quot;")
    )


def metric_value_for_row(row: Dict[str, Any], svg_metric: str) -> Optional[float]:
    if svg_metric == "volatility":
        value = row.get("volatility_index_mean")
        return None if value is None else float(value)
    value = row.get("first_crossing_density")
    return None if value is None else float(value)


def render_svg(rows: Sequence[Dict[str, Any]], output_path: Path, svg_metric: str) -> None:
    if not rows:
        output_path.write_text(
            "<svg xmlns='http://www.w3.org/2000/svg' width='960' height='540'><text x='20' y='30'>No rows available.</text></svg>\n",
            encoding="utf-8",
        )
        return

    ys = [metric_value_for_row(row, svg_metric) for row in rows]
    ys = [float(v) for v in ys if v is not None]
    if svg_metric == "volatility":
        ymins = ys.copy()
        ymaxs = ys.copy()
    else:
        ymins = [float(row["spread_min"]) for row in rows if row["spread_min"] is not None]
        ymaxs = [float(row["spread_max"]) for row in rows if row["spread_max"] is not None]
    profiles = sorted({str(row["state_model"]) for row in rows})
    amplitudes = sorted({float(row.get("perturbation_amplitude", 0.0)) for row in rows})
    frequencies = sorted({float(row.get("perturbation_frequency", 0.0)) for row in rows})
    multi_schedule = len(amplitudes) > 1 or len(frequencies) > 1
    series = sorted(
        {
            (
                str(row["state_model"]),
                float(row.get("perturbation_amplitude", 0.0)),
                float(row.get("perturbation_frequency", 0.0)),
            )
            for row in rows
        },
        key=lambda v: (v[0], v[2], v[1]),
    )
    n_values = sorted({int(row["n_qubits"]) for row in rows})
    palette = {
        "uniform-random": ("#08519c", "#9ecae1"),
        "contiguous-band": ("#a50f15", "#fcbba1"),
        "harmonic-stride": ("#006d2c", "#a1d99b"),
        "low-grade-bias": ("#54278f", "#dadaeb"),
        "high-grade-bias": ("#8c510a", "#dfc27d"),
    }

    cols = min(3, max(1, len(n_values)))
    rows_grid = (len(n_values) + cols - 1) // cols
    panel_width = 320
    panel_height = 240
    outer_margin_left = 70
    outer_margin_right = 20
    outer_margin_top = 50
    outer_margin_bottom = 60
    width = outer_margin_left + outer_margin_right + cols * panel_width
    height = outer_margin_top + outer_margin_bottom + rows_grid * panel_height

    if svg_metric == "volatility":
        min_y = min(ymins or [0.0])
        max_y = max(ymaxs or [1.0])
    else:
        min_y = min(ymins or ys or [0.0])
        max_y = max(ymaxs or ys or [1.0])
    if math.isclose(min_y, max_y):
        min_y = max(0.0, min_y - 0.05)
        max_y = min(1.0, max_y + 0.05)

    def to_panel_coords(panel_x: float, panel_y: float, y: float, profile_idx: int, profile_count: int) -> tuple[float, float]:
        inner_left = 40.0
        inner_right = 14.0
        inner_top = 24.0
        inner_bottom = 34.0
        inner_width = panel_width - inner_left - inner_right
        inner_height = panel_height - inner_top - inner_bottom
        step = inner_width / max(profile_count, 1)
        x = panel_x + inner_left + step * (profile_idx + 0.5)
        y_px = panel_y + inner_top + ((max_y - y) / max(max_y - min_y, 1e-9)) * inner_height
        return x, y_px

    parts: List[str] = []
    parts.append(f"<svg xmlns='http://www.w3.org/2000/svg' width='{width}' height='{height}' viewBox='0 0 {width} {height}'>")
    parts.append("<rect x='0' y='0' width='100%' height='100%' fill='white'/>")
    title = "UGC Dirac Threshold Sweep: small multiples by n_qubits"
    if multi_schedule:
        title += " (perturbation schedule sweep)"
    title += f" ({svg_metric} emphasis)"
    parts.append(
        f"<text x='70' y='24' font-family='monospace' font-size='14'>{svg_escape(title)}</text>"
    )

    for n_idx, n_value in enumerate(n_values):
        col = n_idx % cols
        row_idx = n_idx // cols
        panel_x = outer_margin_left + col * panel_width
        panel_y = outer_margin_top + row_idx * panel_height
        inner_left = 40.0
        inner_right = 14.0
        inner_top = 24.0
        inner_bottom = 34.0
        inner_width = panel_width - inner_left - inner_right
        inner_height = panel_height - inner_top - inner_bottom
        x0 = panel_x + inner_left
        x1 = panel_x + inner_left + inner_width
        y0 = panel_y + inner_top
        y1 = panel_y + inner_top + inner_height

        parts.append(f"<rect x='{panel_x}' y='{panel_y}' width='{panel_width}' height='{panel_height}' fill='none' stroke='#dddddd'/>")
        parts.append(f"<text x='{panel_x + 8}' y='{panel_y + 16}' font-family='monospace' font-size='12'>n={n_value}</text>")
        parts.append(f"<line x1='{x0}' y1='{y1}' x2='{x1}' y2='{y1}' stroke='black'/>")
        parts.append(f"<line x1='{x0}' y1='{y0}' x2='{x0}' y2='{y1}' stroke='black'/>")

        ticks = [0.0, 0.1, 0.2, 0.4, 0.6, 0.8, 1.0] if svg_metric == "volatility" else [0.0, 0.04, 0.08, 0.12, 0.16, 0.20, 0.24, 0.32, 0.40]
        for tick in ticks:
            if tick < min_y - 1e-9 or tick > max_y + 1e-9:
                continue
            y_tick = panel_y + inner_top + ((max_y - tick) / max(max_y - min_y, 1e-9)) * inner_height
            parts.append(f"<line x1='{x0 - 4}' y1='{y_tick:.2f}' x2='{x0}' y2='{y_tick:.2f}' stroke='#666'/>")
            if col == 0:
                parts.append(f"<text x='{x0 - 7}' y='{y_tick + 3:.2f}' text-anchor='end' font-family='monospace' font-size='9'>{svg_escape(f'{tick:.2f}')}</text>")

        panel_rows = [entry for entry in rows if int(entry["n_qubits"]) == n_value]
        series_to_row = {
            (
                str(entry["state_model"]),
                float(entry.get("perturbation_amplitude", 0.0)),
                float(entry.get("perturbation_frequency", 0.0)),
            ): entry
            for entry in panel_rows
        }
        for series_idx, series_item in enumerate(series):
            profile, amp, freq = series_item
            x, _ = to_panel_coords(panel_x, panel_y, min_y, series_idx, len(series))
            label = profile if not multi_schedule else f"{profile}@A{amp:.2f}F{freq:.1f}"
            parts.append(f"<text x='{x:.2f}' y='{y1 + 16:.2f}' text-anchor='middle' font-family='monospace' font-size='9'>{svg_escape(label)}</text>")
            row_entry = series_to_row.get(series_item)
            if row_entry is None:
                continue
            point_color, bar_color = palette.get(profile, ("#444444", "#bbbbbb"))
            first = row_entry.get("first_crossing_density")
            metric_y = metric_value_for_row(row_entry, svg_metric)
            spread_min = row_entry.get("spread_min")
            spread_max = row_entry.get("spread_max")
            if svg_metric != "volatility" and spread_min is not None and spread_max is not None:
                _, y_top = to_panel_coords(panel_x, panel_y, float(spread_max), series_idx, len(series))
                _, y_bottom = to_panel_coords(panel_x, panel_y, float(spread_min), series_idx, len(series))
                parts.append(f"<line x1='{x:.2f}' y1='{y_top:.2f}' x2='{x:.2f}' y2='{y_bottom:.2f}' stroke='{bar_color}' stroke-width='6' stroke-linecap='round'/>")
            if metric_y is not None:
                _, y_point = to_panel_coords(panel_x, panel_y, float(metric_y), series_idx, len(series))
                parts.append(f"<circle cx='{x:.2f}' cy='{y_point:.2f}' r='5' fill='{point_color}'/>")
                delta = row_entry.get("delta_from_uniform")
                ratio = row_entry.get("ratio_to_uniform")
                crossing = row_entry.get("first_crossing_density")
                volatility = row_entry.get("volatility_index_mean")
                annotation = None
                if svg_metric == "ratio" and ratio is not None:
                    annotation = f"R{float(ratio):0.2f}x"
                elif svg_metric == "crossing" and crossing is not None:
                    annotation = f"C{float(crossing):0.2f}"
                elif svg_metric == "volatility" and volatility is not None:
                    annotation = f"V{float(volatility):0.2f}"
                elif delta is not None:
                    annotation = f"Δ{float(delta):+0.2f}"
                if annotation is not None:
                    parts.append(f"<text x='{x + 8:.2f}' y='{y_point - 6:.2f}' font-family='monospace' font-size='9' fill='{point_color}'>{svg_escape(annotation)}</text>")

    x_axis_title = "state model / rotor profile"
    if multi_schedule:
        x_axis_title = "state model with perturbation schedule (A=amplitude, F=frequency)"
    parts.append(f"<text x='{width / 2:.2f}' y='{height - 20}' text-anchor='middle' font-family='monospace' font-size='12'>{svg_escape(x_axis_title)}</text>")
    y_axis_label = "first_crossing_density (error bar = per-seed spread)"
    if svg_metric == "volatility":
        y_axis_label = "volatility_index_mean"
    parts.append(f"<text transform='translate(24 {height / 2:.2f}) rotate(-90)' text-anchor='middle' font-family='monospace' font-size='12'>{svg_escape(y_axis_label)}</text>")
    legend_x = outer_margin_left
    legend_y = 26
    legend_h = 22 * max(1, len(profiles)) + 10
    parts.append(f"<rect x='{legend_x}' y='{legend_y}' width='280' height='{legend_h}' fill='white' stroke='#cccccc'/>")
    for idx, profile in enumerate(profiles):
        point_color, _ = palette.get(profile, ("#444444", "#bbbbbb"))
        y = legend_y + 18 + idx * 22
        deltas = [row.get("delta_from_uniform") for row in rows if row["state_model"] == profile and row.get("delta_from_uniform") is not None]
        ratios = [row.get("ratio_to_uniform") for row in rows if row["state_model"] == profile and row.get("ratio_to_uniform") is not None]
        volatilities = [row.get("volatility_index_mean") for row in rows if row["state_model"] == profile and row.get("volatility_index_mean") is not None]
        if svg_metric == "ratio" and ratios:
            metric_label = f"avg R{sum(float(v) for v in ratios) / len(ratios):.2f}x"
        elif svg_metric == "volatility" and volatilities:
            metric_label = f"avg V{sum(float(v) for v in volatilities) / len(volatilities):.2f}"
        elif svg_metric == "crossing":
            crossings = [row.get("first_crossing_density") for row in rows if row["state_model"] == profile and row.get("first_crossing_density") is not None]
            metric_label = "baseline" if not crossings else f"avg C{sum(float(v) for v in crossings) / len(crossings):.2f}"
        else:
            metric_label = "baseline" if not deltas else f"avg Δ{sum(float(v) for v in deltas) / len(deltas):+.2f}"
        ratio_label = "" if svg_metric in {"crossing", "volatility"} else ("" if not ratios else f", avg R{sum(float(v) for v in ratios) / len(ratios):.2f}x")
        parts.append(f"<circle cx='{legend_x + 12}' cy='{y}' r='4' fill='{point_color}'/>")
        parts.append(f"<text x='{legend_x + 24}' y='{y + 4}' font-family='monospace' font-size='11'>{svg_escape(profile)} ({svg_escape(metric_label + ratio_label)})</text>")
    parts.append("</svg>")
    output_path.write_text("\n".join(parts) + "\n", encoding="utf-8")


def render_markdown(report: Dict[str, Any]) -> str:
    lines: List[str] = []
    lines.append("# Dirac Threshold Sweep Report")
    lines.append("")
    lines.append(f"Generated UTC: {report['generated_utc']}")
    lines.append("")
    lines.append("## Invocation")
    lines.append("")
    lines.append("```bash")
    lines.append(report["invocation"])
    lines.append("```")
    lines.append("")
    lines.append("## Interpretation")
    lines.append("")
    lines.append(f"- Computational analog label: `{report['computational_analog_label']}`")
    lines.append(f"- Analog scope: `{report['computational_analog_scope']}`")
    lines.append(
        f"- Perturbation schedule sweep: amplitudes `{report['perturbation_amplitudes']}` at frequencies `{report['perturbation_frequencies']}`"
    )
    lines.append("- Per-profile threshold narrative:")
    baseline_rows = [row for row in report["rows"] if row.get("state_model") == "uniform-random"]
    baseline_by_n = {row["n_qubits"]: row for row in baseline_rows}
    profile_rows: Dict[str, List[Dict[str, Any]]] = {}
    for row in report["rows"]:
        profile_rows.setdefault(str(row["state_model"]), []).append(row)
    for profile, rows in sorted(profile_rows.items()):
        if profile == "uniform-random":
            lines.append("  - `uniform-random`: baseline reference curve for `delta_from_uniform` comparisons.")
            continue
        deltas = [row.get("delta_from_uniform") for row in rows if row.get("delta_from_uniform") is not None]
        crossings = [row.get("first_crossing_density") for row in rows if row.get("first_crossing_density") is not None]
        if not deltas or not crossings:
            lines.append(f"  - `{profile}`: no crossing observed in the sampled density range, so no baseline delta was measurable.")
            continue
        avg_delta = sum(float(v) for v in deltas) / len(deltas)
        ratios = [row.get("ratio_to_uniform") for row in rows if row.get("ratio_to_uniform") is not None]
        direction = "earlier" if avg_delta < 0 else "later" if avg_delta > 0 else "aligned"
        min_cross = min(float(v) for v in crossings)
        max_cross = max(float(v) for v in crossings)
        ratio_text = ""
        if ratios:
            avg_ratio = sum(float(v) for v in ratios) / len(ratios)
            ratio_text = f", mean `crossing_density_ratio_to_uniform={avg_ratio:.2f}`"
        volatilities = [row.get("volatility_index_mean") for row in rows if row.get("volatility_index_mean") is not None]
        volatility_text = ""
        if volatilities:
            volatility_text = f", mean `volatility_index_mean={sum(float(v) for v in volatilities) / len(volatilities):.2f}`"
        lines.append(
            f"  - `{profile}`: crosses {direction} than `uniform-random` on average (mean `delta_from_uniform={avg_delta:+.2f}`{ratio_text}{volatility_text}), with sampled first-crossing range `{min_cross:.2f}` to `{max_cross:.2f}`."
        )
    lines.append("")
    lines.append("## Summary Rows")
    lines.append("")
    lines.append("| profile | n_qubits | perturbation_amplitude | perturbation_frequency | first_crossing_density | delta_from_uniform | ratio_to_uniform | volatility_index_mean | catastrophic_unraveling_amplitude | spread_min | spread_max | spread | crossing_seed_count | threshold | runtime_ms |")
    lines.append("| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |")
    for row in report["rows"]:
        lines.append(
            f"| {row['state_model']} | {row['n_qubits']} | {row['perturbation_amplitude']} | {row['perturbation_frequency']} | {row['first_crossing_density']} | {row['delta_from_uniform']} | {row['ratio_to_uniform']} | {row['volatility_index_mean']} | {row['catastrophic_unraveling_amplitude']} | {row['spread_min']} | {row['spread_max']} | {row['spread']} | {row['crossing_seed_count']} | {row['density_threshold']} | {row['wall_runtime_ms']} |"
        )
    lines.append("")
    lines.append("## Artifacts")
    lines.append("")
    lines.append(f"- JSON: {report['outputs']['json_report_path']}")
    lines.append(f"- CSV: {report['outputs']['csv_report_path']}")
    lines.append(f"- Markdown: {report['outputs']['markdown_report_path']}")
    lines.append(f"- SVG: {report['outputs']['plot_svg_path']}")
    lines.append("")
    return "\n".join(lines) + "\n"


def parse_csv_list(value: str, cast) -> List[Any]:
    out = []
    for item in value.split(','):
        token = item.strip()
        if token:
            out.append(cast(token))
    if not out:
        raise ValueError(f"empty csv list: {value}")
    return out


def main() -> int:
    parser = argparse.ArgumentParser(description="Run reproducible dirac threshold sweeps across multiple n values.")
    parser.add_argument("--n-values", default="4,5,6,7,8", help="Comma-separated qubit counts.")
    parser.add_argument("--profiles", default="uniform-random,contiguous-band,harmonic-stride,low-grade-bias,high-grade-bias", help="Comma-separated dirac state models / rotor profiles.")
    parser.add_argument("--densities", default="0.02,0.08,0.10,0.12,0.14,0.2,0.3", help="Comma-separated coupling densities.")
    parser.add_argument("--seeds", default="42,777,20260609", help="Comma-separated integer seeds.")
    parser.add_argument("--perturbation-amplitudes", default="0.0", help="Comma-separated perturbation amplitudes in [0,1].")
    parser.add_argument("--perturbation-frequencies", default="8.0", help="Comma-separated perturbation frequencies in [0,1024].")
    parser.add_argument("--output-prefix", default="docs/demo/dirac-mode-threshold-sweep", help="Artifact path prefix without extension.")
    parser.add_argument("--svg-metric", default="delta", choices=("delta", "ratio", "crossing", "volatility"), help="Comparison emphasis for the SVG annotations and legend.")
    parser.add_argument("--cargo-bin", default="cargo", help="Cargo executable to invoke.")
    args = parser.parse_args()

    repo_root = Path(__file__).resolve().parent.parent
    n_values = parse_csv_list(args.n_values, int)
    profiles = parse_csv_list(args.profiles, str)
    densities = parse_csv_list(args.densities, float)
    seeds = parse_csv_list(args.seeds, int)
    perturbation_amplitudes = parse_csv_list(args.perturbation_amplitudes, float)
    perturbation_frequencies = parse_csv_list(args.perturbation_frequencies, float)
    output_prefix = (repo_root / args.output_prefix).resolve()
    output_prefix.parent.mkdir(parents=True, exist_ok=True)

    rows: List[Dict[str, Any]] = []
    raw_reports: List[Dict[str, Any]] = []
    invocation = (
        f"python3 scripts/run-dirac-threshold-sweep.py --n-values {args.n_values} "
        f"--profiles {args.profiles} "
        f"--densities {args.densities} --seeds {args.seeds} "
        f"--perturbation-amplitudes {args.perturbation_amplitudes} "
        f"--perturbation-frequencies {args.perturbation_frequencies} "
        f"--output-prefix {args.output_prefix} --svg-metric {args.svg_metric}"
    )

    for profile in profiles:
        for n_qubits in n_values:
            for perturbation_frequency in perturbation_frequencies:
                for perturbation_amplitude in perturbation_amplitudes:
                    cmd = [
                        args.cargo_bin,
                        "run",
                        "--quiet",
                        "--",
                        "dirac-mode",
                        "--summary",
                        "--n-qubits",
                        str(n_qubits),
                        "--state-model",
                        profile,
                        "--profile-report",
                        "--sweep-export",
                        "json",
                        "--sweep-coupling-densities",
                        ",".join(str(v) for v in densities),
                        "--sweep-seeds",
                        ",".join(str(v) for v in seeds),
                        "--perturbation-amplitudes",
                        str(perturbation_amplitude),
                        "--perturbation-frequency",
                        str(perturbation_frequency),
                    ]
                    report = run_summary_command(cmd, repo_root)
                    raw_reports.append(report)
                    summary = report["summary"]
                    rows.append(
                        {
                            "state_model": summary.get("state_model"),
                            "n_qubits": n_qubits,
                            "perturbation_amplitude": perturbation_amplitude,
                            "perturbation_frequency": perturbation_frequency,
                            "first_crossing_density": summary.get("first_crossing_density"),
                            "delta_from_uniform": summary.get("first_crossing_density_delta_from_baseline"),
                            "ratio_to_uniform": summary.get("crossing_density_ratio_to_uniform"),
                            "spread_min": summary.get("per_seed_crossing_spread_min"),
                            "spread_max": summary.get("per_seed_crossing_spread_max"),
                            "spread": summary.get("per_seed_crossing_spread"),
                            "crossing_seed_count": summary.get("crossing_seed_count"),
                            "density_threshold": summary.get("density_threshold"),
                            "phase_relaxation_steps_mean": summary.get("phase_relaxation_steps_mean"),
                            "torsion_hysteresis_mean": summary.get("torsion_hysteresis_mean"),
                            "volatility_index_mean": summary.get("volatility_index_mean"),
                            "volatility_index_max": summary.get("volatility_index_max"),
                            "catastrophic_unraveling_amplitude": summary.get("catastrophic_unraveling_amplitude"),
                            "wall_runtime_ms": report.get("_wall_runtime_ms"),
                        }
                    )

    computational_analog_label = raw_reports[0]["summary"].get("computational_analog_label") if raw_reports else None
    computational_analog_scope = raw_reports[0]["summary"].get("computational_analog_scope") if raw_reports else None

    aggregate = {
        "object": "csif.quantum.dirac_mode.threshold_sweep.aggregate",
        "schema_version": "csif_dirac_mode_threshold_sweep_aggregate_v1",
        "generated_utc": datetime.now(timezone.utc).isoformat(),
        "invocation": invocation,
        "computational_analog_label": computational_analog_label,
        "computational_analog_scope": computational_analog_scope,
        "n_values": n_values,
        "profiles": profiles,
        "densities": densities,
        "seeds": seeds,
        "perturbation_amplitudes": perturbation_amplitudes,
        "perturbation_frequencies": perturbation_frequencies,
        "rows": rows,
        "reports": raw_reports,
        "outputs": {
            "json_report_path": str(output_prefix.with_suffix(".json").relative_to(repo_root)),
            "csv_report_path": str(output_prefix.with_suffix(".csv").relative_to(repo_root)),
            "markdown_report_path": str(output_prefix.with_suffix(".md").relative_to(repo_root)),
            "plot_svg_path": str(output_prefix.with_suffix(".svg").relative_to(repo_root)),
        },
    }

    json_path = output_prefix.with_suffix(".json")
    csv_path = output_prefix.with_suffix(".csv")
    markdown_path = output_prefix.with_suffix(".md")
    svg_path = output_prefix.with_suffix(".svg")

    json_path.write_text(json.dumps(aggregate, indent=2) + "\n", encoding="utf-8")

    with csv_path.open("w", encoding="utf-8", newline="") as handle:
        writer = csv.DictWriter(
            handle,
            fieldnames=[
                "n_qubits",
                "state_model",
                "first_crossing_density",
                "delta_from_uniform",
                "ratio_to_uniform",
                "perturbation_amplitude",
                "perturbation_frequency",
                "phase_relaxation_steps_mean",
                "torsion_hysteresis_mean",
                "volatility_index_mean",
                "volatility_index_max",
                "catastrophic_unraveling_amplitude",
                "spread_min",
                "spread_max",
                "spread",
                "crossing_seed_count",
                "density_threshold",
                "wall_runtime_ms",
            ],
        )
        writer.writeheader()
        for row in rows:
            writer.writerow(row)

    markdown_path.write_text(render_markdown(aggregate), encoding="utf-8")
    render_svg(rows, svg_path, args.svg_metric)

    print(json.dumps(aggregate, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())