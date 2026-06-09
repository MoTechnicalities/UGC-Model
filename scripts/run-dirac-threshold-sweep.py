#!/usr/bin/env python3
import argparse
import csv
import json
import math
import random
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


def render_profile_svg(rows: Sequence[Dict[str, Any]], output_path: Path, svg_metric: str) -> None:
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


def render_frequency_svg(rows: Sequence[Dict[str, Any]], output_path: Path, svg_metric: str) -> None:
    if not rows:
        output_path.write_text(
            "<svg xmlns='http://www.w3.org/2000/svg' width='960' height='540'><text x='20' y='30'>No rows available.</text></svg>\n",
            encoding="utf-8",
        )
        return

    points = []
    for row in rows:
        y = metric_value_for_row(row, svg_metric)
        if y is None:
            continue
        points.append(
            {
                "n_qubits": int(row["n_qubits"]),
                "state_model": str(row["state_model"]),
                "perturbation_amplitude": float(row.get("perturbation_amplitude", 0.0)),
                "perturbation_frequency": float(row.get("perturbation_frequency", 0.0)),
                "metric_y": float(y),
            }
        )

    if not points:
        output_path.write_text(
            "<svg xmlns='http://www.w3.org/2000/svg' width='960' height='540'><text x='20' y='30'>No plottable rows for selected metric.</text></svg>\n",
            encoding="utf-8",
        )
        return

    n_values = sorted({point["n_qubits"] for point in points})
    profiles = sorted({point["state_model"] for point in points})
    amplitudes = sorted({point["perturbation_amplitude"] for point in points})
    frequencies = sorted({point["perturbation_frequency"] for point in points})

    cols = min(3, max(1, len(n_values)))
    rows_grid = (len(n_values) + cols - 1) // cols
    panel_width = 360
    panel_height = 250
    outer_margin_left = 80
    outer_margin_right = 20
    outer_margin_top = 50
    outer_margin_bottom = 60
    width = outer_margin_left + outer_margin_right + cols * panel_width
    height = outer_margin_top + outer_margin_bottom + rows_grid * panel_height

    y_values = [point["metric_y"] for point in points]
    min_y = min(y_values)
    max_y = max(y_values)
    if math.isclose(min_y, max_y):
        min_y = max(0.0, min_y - 0.05)
        max_y = max_y + 0.05

    min_freq = min(frequencies)
    max_freq = max(frequencies)
    if math.isclose(min_freq, max_freq):
        min_freq = max(0.0, min_freq - 1.0)
        max_freq = max_freq + 1.0

    palette = {
        "uniform-random": "#08519c",
        "contiguous-band": "#a50f15",
        "harmonic-stride": "#006d2c",
        "low-grade-bias": "#54278f",
        "high-grade-bias": "#8c510a",
    }

    amplitude_dashes = ["", "6,4", "2,3", "10,3,2,3"]

    parts: List[str] = []
    parts.append(f"<svg xmlns='http://www.w3.org/2000/svg' width='{width}' height='{height}' viewBox='0 0 {width} {height}'>")
    parts.append("<rect x='0' y='0' width='100%' height='100%' fill='white'/>")
    parts.append(
        "<text x='80' y='24' font-family='monospace' font-size='14'>"
        + svg_escape(f"UGC Dirac Threshold Sweep: metric vs perturbation_frequency ({svg_metric})")
        + "</text>"
    )

    for n_idx, n_value in enumerate(n_values):
        col = n_idx % cols
        row_idx = n_idx // cols
        panel_x = outer_margin_left + col * panel_width
        panel_y = outer_margin_top + row_idx * panel_height

        inner_left = 44.0
        inner_right = 16.0
        inner_top = 24.0
        inner_bottom = 34.0
        inner_width = panel_width - inner_left - inner_right
        inner_height = panel_height - inner_top - inner_bottom
        x0 = panel_x + inner_left
        x1 = panel_x + inner_left + inner_width
        y0 = panel_y + inner_top
        y1 = panel_y + inner_top + inner_height

        def to_coords(freq: float, value: float) -> tuple[float, float]:
            x = x0 + ((freq - min_freq) / max(max_freq - min_freq, 1e-9)) * inner_width
            y = y0 + ((max_y - value) / max(max_y - min_y, 1e-9)) * inner_height
            return x, y

        parts.append(f"<rect x='{panel_x}' y='{panel_y}' width='{panel_width}' height='{panel_height}' fill='none' stroke='#dddddd'/>")
        parts.append(f"<text x='{panel_x + 8}' y='{panel_y + 16}' font-family='monospace' font-size='12'>n={n_value}</text>")
        parts.append(f"<line x1='{x0}' y1='{y1}' x2='{x1}' y2='{y1}' stroke='black'/>")
        parts.append(f"<line x1='{x0}' y1='{y0}' x2='{x0}' y2='{y1}' stroke='black'/>")

        for tick in frequencies:
            x_tick, _ = to_coords(float(tick), min_y)
            parts.append(f"<line x1='{x_tick:.2f}' y1='{y1}' x2='{x_tick:.2f}' y2='{y1 + 4}' stroke='#666'/>")
            parts.append(f"<text x='{x_tick:.2f}' y='{y1 + 15:.2f}' text-anchor='middle' font-family='monospace' font-size='8'>{svg_escape(f'{tick:.0f}')}</text>")

        y_ticks = [min_y + (max_y - min_y) * fraction for fraction in (0.0, 0.25, 0.5, 0.75, 1.0)]
        for tick in y_ticks:
            _, y_tick = to_coords(min_freq, tick)
            parts.append(f"<line x1='{x0 - 4}' y1='{y_tick:.2f}' x2='{x0}' y2='{y_tick:.2f}' stroke='#666'/>")
            if col == 0:
                parts.append(f"<text x='{x0 - 7}' y='{y_tick + 3:.2f}' text-anchor='end' font-family='monospace' font-size='9'>{svg_escape(f'{tick:.3f}')}</text>")

        panel_points = [point for point in points if point["n_qubits"] == n_value]
        series_keys = sorted({(point["state_model"], point["perturbation_amplitude"]) for point in panel_points}, key=lambda v: (v[0], v[1]))
        for series_idx, series_key in enumerate(series_keys):
            profile, amplitude = series_key
            color = palette.get(profile, "#444444")
            dash = amplitude_dashes[series_idx % len(amplitude_dashes)]
            series_points = [point for point in panel_points if point["state_model"] == profile and point["perturbation_amplitude"] == amplitude]
            series_points.sort(key=lambda point: point["perturbation_frequency"])
            if len(series_points) < 2:
                continue
            path_tokens = []
            for point_idx, point in enumerate(series_points):
                x_pt, y_pt = to_coords(point["perturbation_frequency"], point["metric_y"])
                command = "M" if point_idx == 0 else "L"
                path_tokens.append(f"{command} {x_pt:.2f} {y_pt:.2f}")
                parts.append(f"<circle cx='{x_pt:.2f}' cy='{y_pt:.2f}' r='3' fill='{color}'/>")
            dash_attr = "" if not dash else f" stroke-dasharray='{dash}'"
            parts.append(
                "<path d='"
                + " ".join(path_tokens)
                + f"' fill='none' stroke='{color}' stroke-width='1.8'{dash_attr}/>"
            )

    parts.append(f"<text x='{width / 2:.2f}' y='{height - 20}' text-anchor='middle' font-family='monospace' font-size='12'>perturbation_frequency</text>")
    y_axis_label = "volatility_index_mean" if svg_metric == "volatility" else "first_crossing_density"
    parts.append(f"<text transform='translate(24 {height / 2:.2f}) rotate(-90)' text-anchor='middle' font-family='monospace' font-size='12'>{svg_escape(y_axis_label)}</text>")

    legend_x = outer_margin_left
    legend_y = 26
    series_labels = sorted({(point["state_model"], point["perturbation_amplitude"]) for point in points}, key=lambda v: (v[0], v[1]))
    legend_height = 20 * max(1, len(series_labels)) + 10
    parts.append(f"<rect x='{legend_x}' y='{legend_y}' width='420' height='{legend_height}' fill='white' stroke='#cccccc'/>")
    for idx, (profile, amplitude) in enumerate(series_labels):
        color = palette.get(profile, "#444444")
        y = legend_y + 16 + idx * 20
        parts.append(f"<line x1='{legend_x + 8}' y1='{y}' x2='{legend_x + 26}' y2='{y}' stroke='{color}' stroke-width='2'/>")
        parts.append(f"<circle cx='{legend_x + 17}' cy='{y}' r='2.5' fill='{color}'/>")
        parts.append(f"<text x='{legend_x + 34}' y='{y + 4}' font-family='monospace' font-size='10'>{svg_escape(f'{profile} @A={amplitude:.2f}')}</text>")

    parts.append("</svg>")
    output_path.write_text("\n".join(parts) + "\n", encoding="utf-8")


def render_svg(rows: Sequence[Dict[str, Any]], output_path: Path, svg_metric: str, svg_domain: str) -> None:
    if svg_domain == "frequency":
        render_frequency_svg(rows, output_path, svg_metric)
        return
    render_profile_svg(rows, output_path, svg_metric)


def interpolate_crossing(x1: float, y1: float, x2: float, y2: float, target: float) -> Optional[float]:
    if math.isclose(y1, y2):
        return None
    t = (target - y1) / (y2 - y1)
    if t < -1e-9 or t > 1.0 + 1e-9:
        return None
    return x1 + t * (x2 - x1)


def compute_resonance_analysis(rows: Sequence[Dict[str, Any]]) -> Dict[str, Any]:
    series_points: Dict[tuple[str, int, float], List[tuple[float, float]]] = {}
    for row in rows:
        volatility = row.get("volatility_index_mean")
        if volatility is None:
            continue
        key = (
            str(row["state_model"]),
            int(row["n_qubits"]),
            float(row.get("perturbation_amplitude", 0.0)),
        )
        series_points.setdefault(key, []).append((float(row.get("perturbation_frequency", 0.0)), float(volatility)))

    resonance_entries: List[Dict[str, Any]] = []
    for key, points in sorted(series_points.items(), key=lambda item: (item[0][0], item[0][1], item[0][2])):
        if len(points) < 3:
            continue
        profile, n_qubits, amplitude = key
        ordered = sorted(points, key=lambda item: item[0])
        peak_idx, peak = max(enumerate(ordered), key=lambda item: item[1][1])
        peak_freq, peak_val = peak

        left_baseline = min(value for _, value in ordered[: peak_idx + 1])
        right_baseline = min(value for _, value in ordered[peak_idx:])
        baseline = min(left_baseline, right_baseline)
        half_max = baseline + (peak_val - baseline) * 0.5

        left_crossing = None
        for idx in range(peak_idx, 0, -1):
            left = ordered[idx - 1]
            right = ordered[idx]
            y_low, y_high = sorted((left[1], right[1]))
            if y_low - 1e-9 <= half_max <= y_high + 1e-9:
                left_crossing = interpolate_crossing(left[0], left[1], right[0], right[1], half_max)
                if left_crossing is not None:
                    break

        right_crossing = None
        for idx in range(peak_idx, len(ordered) - 1):
            left = ordered[idx]
            right = ordered[idx + 1]
            y_low, y_high = sorted((left[1], right[1]))
            if y_low - 1e-9 <= half_max <= y_high + 1e-9:
                right_crossing = interpolate_crossing(left[0], left[1], right[0], right[1], half_max)
                if right_crossing is not None:
                    break

        fwhm = None
        q_factor = None
        if left_crossing is not None and right_crossing is not None:
            width = right_crossing - left_crossing
            if width > 1e-9:
                fwhm = width
                q_factor = peak_freq / width

        right_side = ordered[peak_idx + 1 :]
        trough_freq = None
        trough_val = None
        if right_side:
            trough_freq, trough_val = min(right_side, key=lambda item: item[1])

        anti_resonant_delta = None
        peak_to_trough_gap = None
        anti_resonant_slope = None
        if trough_val is not None and trough_freq is not None:
            anti_resonant_delta = peak_val - trough_val
            peak_to_trough_gap = trough_freq - peak_freq
            if not math.isclose(peak_to_trough_gap, 0.0):
                anti_resonant_slope = anti_resonant_delta / peak_to_trough_gap

        resonance_entries.append(
            {
                "state_model": profile,
                "n_qubits": n_qubits,
                "perturbation_amplitude": amplitude,
                "sample_count": len(ordered),
                "peak_frequency": peak_freq,
                "peak_value": peak_val,
                "half_max": half_max,
                "fwhm_reference": "half_prominence",
                "fwhm": fwhm,
                "q_factor": q_factor,
                "left_half_max_frequency": left_crossing,
                "right_half_max_frequency": right_crossing,
                "trough_frequency_after_peak": trough_freq,
                "trough_value_after_peak": trough_val,
                "anti_resonant_delta": anti_resonant_delta,
                "peak_to_trough_frequency_gap": peak_to_trough_gap,
                "anti_resonant_slope": anti_resonant_slope,
            }
        )

    profile_q_stats: List[Dict[str, Any]] = []
    by_profile: Dict[str, List[float]] = {}
    for entry in resonance_entries:
        q_factor = entry.get("q_factor")
        if q_factor is None:
            continue
        by_profile.setdefault(str(entry["state_model"]), []).append(float(q_factor))

    for profile, q_values in sorted(by_profile.items()):
        q_mean = sum(q_values) / len(q_values)
        q_min = min(q_values)
        q_max = max(q_values)
        q_std = math.sqrt(sum((value - q_mean) ** 2 for value in q_values) / len(q_values)) if q_values else None
        profile_q_stats.append(
            {
                "state_model": profile,
                "q_factor_mean": q_mean,
                "q_factor_std": q_std,
                "q_factor_min": q_min,
                "q_factor_max": q_max,
                "q_factor_count": len(q_values),
            }
        )

    profile_impedance_raw: Dict[str, Dict[str, List[float]]] = {}
    for entry in resonance_entries:
        profile = str(entry["state_model"])
        profile_bucket = profile_impedance_raw.setdefault(
            profile,
            {
                "q_factor": [],
                "anti_resonant_slope": [],
                "anti_resonant_delta": [],
                "peak_frequency": [],
            },
        )
        q_factor = entry.get("q_factor")
        anti_slope = entry.get("anti_resonant_slope")
        anti_delta = entry.get("anti_resonant_delta")
        peak_freq = entry.get("peak_frequency")
        if q_factor is not None:
            profile_bucket["q_factor"].append(float(q_factor))
        if anti_slope is not None:
            profile_bucket["anti_resonant_slope"].append(float(anti_slope))
        if anti_delta is not None:
            profile_bucket["anti_resonant_delta"].append(float(anti_delta))
        if peak_freq is not None:
            profile_bucket["peak_frequency"].append(float(peak_freq))

    profile_means: Dict[str, Dict[str, Optional[float]]] = {}
    for profile, buckets in sorted(profile_impedance_raw.items()):
        profile_means[profile] = {
            "q_factor_mean": (sum(buckets["q_factor"]) / len(buckets["q_factor"])) if buckets["q_factor"] else None,
            "anti_resonant_slope_mean": (sum(buckets["anti_resonant_slope"]) / len(buckets["anti_resonant_slope"])) if buckets["anti_resonant_slope"] else None,
            "anti_resonant_delta_mean": (sum(buckets["anti_resonant_delta"]) / len(buckets["anti_resonant_delta"])) if buckets["anti_resonant_delta"] else None,
            "peak_frequency_mean": (sum(buckets["peak_frequency"]) / len(buckets["peak_frequency"])) if buckets["peak_frequency"] else None,
        }

    baseline_profile = "uniform-random"
    baseline_means = profile_means.get(baseline_profile, {})
    baseline_q = baseline_means.get("q_factor_mean")
    baseline_slope = baseline_means.get("anti_resonant_slope_mean")
    baseline_delta = baseline_means.get("anti_resonant_delta_mean")
    baseline_peak_frequency = baseline_means.get("peak_frequency_mean")

    profile_impedance_ranking: List[Dict[str, Any]] = []
    for profile, means in sorted(profile_means.items()):
        q_mean = means.get("q_factor_mean")
        slope_mean = means.get("anti_resonant_slope_mean")
        delta_mean = means.get("anti_resonant_delta_mean")
        peak_freq_mean = means.get("peak_frequency_mean")

        q_ratio_to_uniform = None
        q_delta_from_uniform = None
        if q_mean is not None and baseline_q is not None and not math.isclose(baseline_q, 0.0):
            q_ratio_to_uniform = q_mean / baseline_q
            q_delta_from_uniform = q_mean - baseline_q

        anti_resonant_slope_ratio_to_uniform = None
        anti_resonant_slope_delta_from_uniform = None
        if slope_mean is not None and baseline_slope is not None and not math.isclose(baseline_slope, 0.0):
            anti_resonant_slope_ratio_to_uniform = slope_mean / baseline_slope
            anti_resonant_slope_delta_from_uniform = slope_mean - baseline_slope

        anti_resonant_delta_ratio_to_uniform = None
        anti_resonant_delta_delta_from_uniform = None
        if delta_mean is not None and baseline_delta is not None and not math.isclose(baseline_delta, 0.0):
            anti_resonant_delta_ratio_to_uniform = delta_mean / baseline_delta
            anti_resonant_delta_delta_from_uniform = delta_mean - baseline_delta

        peak_frequency_shift_from_uniform = None
        if peak_freq_mean is not None and baseline_peak_frequency is not None:
            peak_frequency_shift_from_uniform = peak_freq_mean - baseline_peak_frequency

        index_components = [
            value
            for value in [
                q_ratio_to_uniform,
                anti_resonant_slope_ratio_to_uniform,
                anti_resonant_delta_ratio_to_uniform,
            ]
            if value is not None
        ]
        profile_impedance_index = (sum(index_components) / len(index_components)) if index_components else None

        profile_impedance_ranking.append(
            {
                "state_model": profile,
                "profile_impedance_index": profile_impedance_index,
                "q_factor_mean": q_mean,
                "q_factor_delta_from_uniform": q_delta_from_uniform,
                "q_factor_ratio_to_uniform": q_ratio_to_uniform,
                "anti_resonant_slope_mean": slope_mean,
                "anti_resonant_slope_delta_from_uniform": anti_resonant_slope_delta_from_uniform,
                "anti_resonant_slope_ratio_to_uniform": anti_resonant_slope_ratio_to_uniform,
                "anti_resonant_delta_mean": delta_mean,
                "anti_resonant_delta_delta_from_uniform": anti_resonant_delta_delta_from_uniform,
                "anti_resonant_delta_ratio_to_uniform": anti_resonant_delta_ratio_to_uniform,
                "peak_frequency_mean": peak_freq_mean,
                "peak_frequency_shift_from_uniform": peak_frequency_shift_from_uniform,
            }
        )

    profile_impedance_ranking.sort(
        key=lambda item: (
            float(item["profile_impedance_index"]) if item.get("profile_impedance_index") is not None else float("-inf"),
            str(item["state_model"]),
        ),
        reverse=True,
    )
    for idx, item in enumerate(profile_impedance_ranking, start=1):
        item["rank"] = idx

    strongest_peak = None
    strongest_dip = None
    if resonance_entries:
        strongest_peak = max(resonance_entries, key=lambda entry: float(entry["peak_value"]))
        strongest_dip = max(
            resonance_entries,
            key=lambda entry: float(entry["anti_resonant_delta"]) if entry.get("anti_resonant_delta") is not None else -1.0,
        )

    return {
        "domain_metric": "volatility_index_mean",
        "entry_count": len(resonance_entries),
        "series": resonance_entries,
        "profile_q_variance": profile_q_stats,
        "impedance_baseline_profile": baseline_profile,
        "profile_impedance_ranking": profile_impedance_ranking,
        "strongest_peak": strongest_peak,
        "strongest_anti_resonance": strongest_dip,
    }


def quantile(values: Sequence[float], q: float) -> Optional[float]:
    if not values:
        return None
    ordered = sorted(float(value) for value in values)
    if len(ordered) == 1:
        return ordered[0]
    q_clamped = min(max(float(q), 0.0), 1.0)
    idx = q_clamped * (len(ordered) - 1)
    lo = int(math.floor(idx))
    hi = int(math.ceil(idx))
    if lo == hi:
        return ordered[lo]
    frac = idx - lo
    return ordered[lo] * (1.0 - frac) + ordered[hi] * frac


def augment_resonance_with_seed_bootstrap(
    resonance: Dict[str, Any],
    seed_rows: Sequence[Dict[str, Any]],
    iterations: int,
    ci_level: float,
    rng_seed: int,
) -> None:
    metadata: Dict[str, Any] = {
        "enabled": bool(seed_rows),
        "mode": "seed",
        "iterations": int(iterations),
        "ci_level": float(ci_level),
        "rng_seed": int(rng_seed),
        "seed_count": len({int(row["seed"]) for row in seed_rows if row.get("seed") is not None}),
        "status": "disabled",
    }

    ranking = resonance.get("profile_impedance_ranking") or []
    if not ranking or not seed_rows:
        metadata["status"] = "insufficient_seed_data"
        resonance["bootstrap_confidence"] = metadata
        return

    # Build per-seed resonance summaries by profile from single-seed sweeps.
    by_series_seed: Dict[tuple[str, int, float, int], List[tuple[float, float]]] = {}
    for row in seed_rows:
        value = row.get("volatility_index_mean")
        seed = row.get("seed")
        if value is None or seed is None:
            continue
        key = (
            str(row["state_model"]),
            int(row["n_qubits"]),
            float(row.get("perturbation_amplitude", 0.0)),
            int(seed),
        )
        by_series_seed.setdefault(key, []).append((float(row.get("perturbation_frequency", 0.0)), float(value)))

    per_seed_profile_metrics: Dict[int, Dict[str, Dict[str, Optional[float]]]] = {}
    for key, points in sorted(by_series_seed.items(), key=lambda item: (item[0][0], item[0][1], item[0][2], item[0][3])):
        if len(points) < 3:
            continue
        profile, _, _, seed = key
        ordered = sorted(points, key=lambda item: item[0])
        peak_idx, peak = max(enumerate(ordered), key=lambda item: item[1][1])
        peak_freq, peak_val = peak

        left_baseline = min(value for _, value in ordered[: peak_idx + 1])
        right_baseline = min(value for _, value in ordered[peak_idx:])
        baseline = min(left_baseline, right_baseline)
        half_max = baseline + (peak_val - baseline) * 0.5

        left_crossing = None
        for idx in range(peak_idx, 0, -1):
            left = ordered[idx - 1]
            right = ordered[idx]
            y_low, y_high = sorted((left[1], right[1]))
            if y_low - 1e-9 <= half_max <= y_high + 1e-9:
                left_crossing = interpolate_crossing(left[0], left[1], right[0], right[1], half_max)
                if left_crossing is not None:
                    break

        right_crossing = None
        for idx in range(peak_idx, len(ordered) - 1):
            left = ordered[idx]
            right = ordered[idx + 1]
            y_low, y_high = sorted((left[1], right[1]))
            if y_low - 1e-9 <= half_max <= y_high + 1e-9:
                right_crossing = interpolate_crossing(left[0], left[1], right[0], right[1], half_max)
                if right_crossing is not None:
                    break

        q_factor = None
        if left_crossing is not None and right_crossing is not None:
            width = right_crossing - left_crossing
            if width > 1e-9:
                q_factor = peak_freq / width

        right_side = ordered[peak_idx + 1 :]
        anti_slope = None
        anti_delta = None
        if right_side:
            trough_freq, trough_val = min(right_side, key=lambda item: item[1])
            anti_delta = peak_val - trough_val
            gap = trough_freq - peak_freq
            if not math.isclose(gap, 0.0):
                anti_slope = anti_delta / gap

        profile_bucket = per_seed_profile_metrics.setdefault(seed, {}).setdefault(
            profile,
            {
                "q_values": [],
                "slope_values": [],
                "delta_values": [],
            },
        )
        if q_factor is not None:
            profile_bucket["q_values"].append(float(q_factor))
        if anti_slope is not None:
            profile_bucket["slope_values"].append(float(anti_slope))
        if anti_delta is not None:
            profile_bucket["delta_values"].append(float(anti_delta))

    seed_profiles: Dict[int, Dict[str, Dict[str, Optional[float]]]] = {}
    for seed, profile_map in per_seed_profile_metrics.items():
        seed_profiles[seed] = {}
        for profile, buckets in profile_map.items():
            q_values = buckets.get("q_values", [])
            slope_values = buckets.get("slope_values", [])
            delta_values = buckets.get("delta_values", [])
            seed_profiles[seed][profile] = {
                "q_mean": (sum(q_values) / len(q_values)) if q_values else None,
                "slope_mean": (sum(slope_values) / len(slope_values)) if slope_values else None,
                "delta_mean": (sum(delta_values) / len(delta_values)) if delta_values else None,
            }

    available_seeds = sorted(seed_profiles.keys())
    if len(available_seeds) < 2:
        metadata["status"] = "insufficient_seed_data"
        resonance["bootstrap_confidence"] = metadata
        return

    alpha = (1.0 - float(ci_level)) / 2.0
    profiles = [str(item["state_model"]) for item in ranking]
    base_profile = str(resonance.get("impedance_baseline_profile", "uniform-random"))
    point_rank = {str(item["state_model"]): int(item.get("rank", 0)) for item in ranking}
    dist: Dict[str, Dict[str, List[float]]] = {
        profile: {"q_ratio": [], "slope_ratio": [], "rank": []}
        for profile in profiles
    }

    rng = random.Random(int(rng_seed))
    valid_iterations = 0
    for _ in range(max(1, int(iterations))):
        sampled = [rng.choice(available_seeds) for _ in range(len(available_seeds))]

        profile_means: Dict[str, Dict[str, Optional[float]]] = {}
        for profile in profiles:
            q_vals: List[float] = []
            slope_vals: List[float] = []
            delta_vals: List[float] = []
            for seed in sampled:
                metrics = seed_profiles.get(seed, {}).get(profile)
                if not metrics:
                    continue
                if metrics.get("q_mean") is not None:
                    q_vals.append(float(metrics["q_mean"]))
                if metrics.get("slope_mean") is not None:
                    slope_vals.append(float(metrics["slope_mean"]))
                if metrics.get("delta_mean") is not None:
                    delta_vals.append(float(metrics["delta_mean"]))
            profile_means[profile] = {
                "q_mean": (sum(q_vals) / len(q_vals)) if q_vals else None,
                "slope_mean": (sum(slope_vals) / len(slope_vals)) if slope_vals else None,
                "delta_mean": (sum(delta_vals) / len(delta_vals)) if delta_vals else None,
            }

        baseline = profile_means.get(base_profile, {})
        bq = baseline.get("q_mean")
        bs = baseline.get("slope_mean")
        bd = baseline.get("delta_mean")

        replicate_rows: List[Dict[str, Any]] = []
        for profile in profiles:
            means = profile_means.get(profile, {})
            q_ratio = None
            if means.get("q_mean") is not None and bq is not None and not math.isclose(bq, 0.0):
                q_ratio = float(means["q_mean"]) / float(bq)
            slope_ratio = None
            if means.get("slope_mean") is not None and bs is not None and not math.isclose(bs, 0.0):
                slope_ratio = float(means["slope_mean"]) / float(bs)
            delta_ratio = None
            if means.get("delta_mean") is not None and bd is not None and not math.isclose(bd, 0.0):
                delta_ratio = float(means["delta_mean"]) / float(bd)
            components = [value for value in [q_ratio, slope_ratio, delta_ratio] if value is not None]
            impedance_index = (sum(components) / len(components)) if components else None
            replicate_rows.append(
                {
                    "state_model": profile,
                    "q_ratio": q_ratio,
                    "slope_ratio": slope_ratio,
                    "impedance_index": impedance_index,
                }
            )

        ranked = sorted(
            [item for item in replicate_rows if item.get("impedance_index") is not None],
            key=lambda item: float(item["impedance_index"]),
            reverse=True,
        )
        if not ranked:
            continue
        valid_iterations += 1
        rank_map = {str(item["state_model"]): idx + 1 for idx, item in enumerate(ranked)}
        for item in replicate_rows:
            profile = str(item["state_model"])
            if item.get("q_ratio") is not None:
                dist[profile]["q_ratio"].append(float(item["q_ratio"]))
            if item.get("slope_ratio") is not None:
                dist[profile]["slope_ratio"].append(float(item["slope_ratio"]))
            if profile in rank_map:
                dist[profile]["rank"].append(float(rank_map[profile]))

    if valid_iterations == 0:
        metadata["status"] = "insufficient_seed_data"
        resonance["bootstrap_confidence"] = metadata
        return

    metadata["status"] = "ok"
    metadata["valid_iterations"] = valid_iterations
    resonance["bootstrap_confidence"] = metadata
    for item in ranking:
        profile = str(item["state_model"])
        q_dist = dist.get(profile, {}).get("q_ratio", [])
        slope_dist = dist.get(profile, {}).get("slope_ratio", [])
        rank_dist = dist.get(profile, {}).get("rank", [])
        q_ci_low = quantile(q_dist, alpha)
        q_ci_high = quantile(q_dist, 1.0 - alpha)
        slope_ci_low = quantile(slope_dist, alpha)
        slope_ci_high = quantile(slope_dist, 1.0 - alpha)
        item["q_factor_ratio_to_uniform_ci_low"] = q_ci_low
        item["q_factor_ratio_to_uniform_ci_high"] = q_ci_high
        item["anti_resonant_slope_ratio_to_uniform_ci_low"] = slope_ci_low
        item["anti_resonant_slope_ratio_to_uniform_ci_high"] = slope_ci_high
        item["bootstrap_rank_ci_low"] = quantile(rank_dist, alpha)
        item["bootstrap_rank_ci_high"] = quantile(rank_dist, 1.0 - alpha)

        if q_ci_low is None or q_ci_high is None:
            q_sig = "unknown"
            q_mark = "Q?"
        elif q_ci_low > 1.0:
            q_sig = "invariant_above"
            q_mark = "Q+"
        elif q_ci_high < 1.0:
            q_sig = "invariant_below"
            q_mark = "Q-"
        else:
            q_sig = "borderline"
            q_mark = "Q~"

        if slope_ci_low is None or slope_ci_high is None:
            slope_sig = "unknown"
            slope_mark = "S?"
        elif slope_ci_low > 1.0:
            slope_sig = "invariant_above"
            slope_mark = "S+"
        elif slope_ci_high < 1.0:
            slope_sig = "invariant_below"
            slope_mark = "S-"
        else:
            slope_sig = "borderline"
            slope_mark = "S~"

        item["q_ratio_significance"] = q_sig
        item["slope_ratio_significance"] = slope_sig
        item["ci_significance_compact"] = f"{q_mark}/{slope_mark}"
        point = point_rank.get(profile)
        if rank_dist and point is not None:
            flips = [1.0 if int(round(value)) != int(point) else 0.0 for value in rank_dist]
            item["bootstrap_rank_flip_rate"] = sum(flips) / len(flips)
        else:
            item["bootstrap_rank_flip_rate"] = None


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
    lines.append(f"- SVG plot domain: `{report['svg_domain']}`")
    lines.append(
        f"- Perturbation schedule sweep: amplitudes `{report['perturbation_amplitudes']}` at frequencies `{report['perturbation_frequencies']}`"
    )
    resonance = report.get("resonance_detector") or {}
    lines.append(f"- Resonance detector metric: `{resonance.get('domain_metric', 'volatility_index_mean')}`")
    lines.append(f"- Resonance detector series analyzed: `{resonance.get('entry_count', 0)}`")
    bootstrap_meta = resonance.get("bootstrap_confidence") or {}
    lines.append(
        f"- Seed-bootstrap confidence: status `{bootstrap_meta.get('status', 'disabled')}`, iterations `{bootstrap_meta.get('iterations', 0)}`, ci_level `{bootstrap_meta.get('ci_level', 0.95)}`, seed_count `{bootstrap_meta.get('seed_count', 0)}`"
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
    lines.append("## Resonance Detector (Volatility Domain)")
    lines.append("")
    series = resonance.get("series") or []
    if not series:
        lines.append("No resonance series were computed (insufficient frequency samples per profile/n/amplitude series).")
        lines.append("")
    else:
        lines.append("| profile | n_qubits | amplitude | peak_frequency | peak_value | fwhm | q_factor | trough_frequency_after_peak | trough_value_after_peak | anti_resonant_delta | peak_to_trough_frequency_gap | anti_resonant_slope |")
        lines.append("| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |")
        for item in series:
            lines.append(
                f"| {item['state_model']} | {item['n_qubits']} | {item['perturbation_amplitude']} | {item['peak_frequency']} | {item['peak_value']} | {item['fwhm']} | {item['q_factor']} | {item['trough_frequency_after_peak']} | {item['trough_value_after_peak']} | {item['anti_resonant_delta']} | {item['peak_to_trough_frequency_gap']} | {item['anti_resonant_slope']} |"
            )
        lines.append("")
        profile_variance = resonance.get("profile_q_variance") or []
        if profile_variance:
            lines.append("### Profile-Selective Q-Factor Variance")
            lines.append("")
            lines.append("| profile | q_factor_mean | q_factor_std | q_factor_min | q_factor_max | q_factor_count |")
            lines.append("| --- | ---: | ---: | ---: | ---: | ---: |")
            for item in profile_variance:
                lines.append(
                    f"| {item['state_model']} | {item['q_factor_mean']} | {item['q_factor_std']} | {item['q_factor_min']} | {item['q_factor_max']} | {item['q_factor_count']} |"
                )
            lines.append("")

        impedance_rows = resonance.get("profile_impedance_ranking") or []
        if impedance_rows:
            baseline_profile = resonance.get("impedance_baseline_profile", "uniform-random")
            lines.append(f"### Profile Impedance Ranking (Baseline: `{baseline_profile}`)")
            lines.append("")
            lines.append("CI significance compact: `Q+/Q-/Q~/Q?` for `q_ratio_ci` and `S+/S-/S~/S?` for `slope_ratio_ci` (`+` excludes above `1.0`, `-` excludes below `1.0`, `~` includes `1.0`, `?` unavailable).")
            lines.append("")
            lines.append("| rank | profile | ci_significance | impedance_index | q_factor_mean | q_ratio_to_uniform | q_ratio_ci | anti_resonant_slope_mean | slope_ratio_to_uniform | slope_ratio_ci | anti_resonant_delta_mean | delta_ratio_to_uniform | peak_frequency_mean | peak_frequency_shift_from_uniform | bootstrap_rank_ci | rank_flip_rate |")
            lines.append("| ---: | --- | --- | ---: | ---: | ---: | --- | ---: | ---: | --- | ---: | ---: | ---: | ---: | --- | ---: |")
            for item in impedance_rows:
                q_ci_low = item.get("q_factor_ratio_to_uniform_ci_low")
                q_ci_high = item.get("q_factor_ratio_to_uniform_ci_high")
                q_ci = "n/a" if q_ci_low is None or q_ci_high is None else f"[{q_ci_low:.4f}, {q_ci_high:.4f}]"
                slope_ci_low = item.get("anti_resonant_slope_ratio_to_uniform_ci_low")
                slope_ci_high = item.get("anti_resonant_slope_ratio_to_uniform_ci_high")
                slope_ci = "n/a" if slope_ci_low is None or slope_ci_high is None else f"[{slope_ci_low:.4f}, {slope_ci_high:.4f}]"
                rank_ci_low = item.get("bootstrap_rank_ci_low")
                rank_ci_high = item.get("bootstrap_rank_ci_high")
                rank_ci = "n/a" if rank_ci_low is None or rank_ci_high is None else f"[{rank_ci_low:.2f}, {rank_ci_high:.2f}]"
                lines.append(
                    f"| {item['rank']} | {item['state_model']} | {item.get('ci_significance_compact')} | {item['profile_impedance_index']} | {item['q_factor_mean']} | {item['q_factor_ratio_to_uniform']} | {q_ci} | {item['anti_resonant_slope_mean']} | {item['anti_resonant_slope_ratio_to_uniform']} | {slope_ci} | {item['anti_resonant_delta_mean']} | {item['anti_resonant_delta_ratio_to_uniform']} | {item['peak_frequency_mean']} | {item['peak_frequency_shift_from_uniform']} | {rank_ci} | {item.get('bootstrap_rank_flip_rate')} |"
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
    parser.add_argument("--svg-domain", default="profile", choices=("profile", "frequency"), help="SVG x-axis domain: profile categories or perturbation frequency.")
    parser.add_argument("--bootstrap-seed-ci", action="store_true", help="Run extra per-seed sweeps and compute seed-bootstrap confidence intervals for profile impedance ratios.")
    parser.add_argument("--bootstrap-iterations", type=int, default=1000, help="Bootstrap iterations when --bootstrap-seed-ci is enabled.")
    parser.add_argument("--bootstrap-ci-level", type=float, default=0.95, help="Bootstrap confidence interval level in (0,1).")
    parser.add_argument("--bootstrap-rng-seed", type=int, default=20260609, help="Deterministic RNG seed for bootstrap resampling.")
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
        f"--output-prefix {args.output_prefix} --svg-metric {args.svg_metric} --svg-domain {args.svg_domain}"
    )
    if args.bootstrap_seed_ci:
        invocation += (
            f" --bootstrap-seed-ci --bootstrap-iterations {args.bootstrap_iterations}"
            f" --bootstrap-ci-level {args.bootstrap_ci_level} --bootstrap-rng-seed {args.bootstrap_rng_seed}"
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

    seed_rows: List[Dict[str, Any]] = []
    if args.bootstrap_seed_ci:
        for profile in profiles:
            for n_qubits in n_values:
                for perturbation_frequency in perturbation_frequencies:
                    for perturbation_amplitude in perturbation_amplitudes:
                        for seed in seeds:
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
                                str(seed),
                                "--perturbation-amplitudes",
                                str(perturbation_amplitude),
                                "--perturbation-frequency",
                                str(perturbation_frequency),
                            ]
                            report = run_summary_command(cmd, repo_root)
                            summary = report["summary"]
                            seed_rows.append(
                                {
                                    "state_model": summary.get("state_model"),
                                    "n_qubits": n_qubits,
                                    "perturbation_amplitude": perturbation_amplitude,
                                    "perturbation_frequency": perturbation_frequency,
                                    "volatility_index_mean": summary.get("volatility_index_mean"),
                                    "seed": seed,
                                }
                            )

    computational_analog_label = raw_reports[0]["summary"].get("computational_analog_label") if raw_reports else None
    computational_analog_scope = raw_reports[0]["summary"].get("computational_analog_scope") if raw_reports else None

    resonance_detector = compute_resonance_analysis(rows)
    if args.bootstrap_seed_ci:
        augment_resonance_with_seed_bootstrap(
            resonance_detector,
            seed_rows,
            iterations=max(1, int(args.bootstrap_iterations)),
            ci_level=min(max(float(args.bootstrap_ci_level), 0.5), 0.999),
            rng_seed=int(args.bootstrap_rng_seed),
        )

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
        "svg_domain": args.svg_domain,
        "resonance_detector": resonance_detector,
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
    render_svg(rows, svg_path, args.svg_metric, args.svg_domain)

    print(json.dumps(aggregate, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())