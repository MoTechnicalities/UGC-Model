#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
default_spec="$repo_root/docs/specifications/ugc-symbolic-benchmark-v1.json"

echo_usage() {
  cat <<'USAGE'
Usage:
  scripts/score-ugc-symbolic-benchmark.sh [options]

Options:
  --endpoint URL        Math endpoint URL (default: http://127.0.0.1:8080/v1/csif/math)
  --spec PATH           Benchmark spec path (default: docs/specifications/ugc-symbolic-benchmark-v1.json)
  --mode MODE           Request mode sent to /v1/csif/math (default: geometric)
  --angle-unit UNIT     Request angle unit (default: radians)
  --timeout SECONDS     Per-request timeout in seconds (default: 20)
  --dry-run             Validate spec and list cases without calling endpoint
  --help                Show this help

Environment overrides:
  UGC_BENCH_ENDPOINT
  UGC_BENCH_SPEC
USAGE
}

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "error: required command not found: $1" >&2
    exit 2
  fi
}

endpoint="${UGC_BENCH_ENDPOINT:-http://127.0.0.1:8080/v1/csif/math}"
spec_path="${UGC_BENCH_SPEC:-$default_spec}"
mode="geometric"
angle_unit="radians"
timeout_sec="20"
dry_run="false"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --endpoint)
      endpoint="$2"
      shift 2
      ;;
    --spec)
      spec_path="$2"
      shift 2
      ;;
    --mode)
      mode="$2"
      shift 2
      ;;
    --angle-unit)
      angle_unit="$2"
      shift 2
      ;;
    --timeout)
      timeout_sec="$2"
      shift 2
      ;;
    --dry-run)
      dry_run="true"
      shift
      ;;
    --help)
      echo_usage
      exit 0
      ;;
    *)
      echo "error: unknown option: $1" >&2
      echo_usage
      exit 2
      ;;
  esac
done

require_cmd jq
require_cmd curl

if [[ ! -f "$spec_path" ]]; then
  echo "error: spec file not found: $spec_path" >&2
  exit 2
fi

jq empty "$spec_path" >/dev/null

mapfile -t cases < <(jq -c '.cases[]' "$spec_path")
if [[ ${#cases[@]} -eq 0 ]]; then
  echo "error: no benchmark cases found in spec: $spec_path" >&2
  exit 2
fi

w_family="$(jq -r '.scoring.family_id' "$spec_path")"
w_canonical="$(jq -r '.scoring.canonical_form' "$spec_path")"
w_tags="$(jq -r '.scoring.ontology_tags' "$spec_path")"
w_block="$(jq -r '.scoring.benchmark_output_block' "$spec_path")"
pass_threshold="$(jq -r '.scoring.pass_threshold' "$spec_path")"

printf 'UGC symbolic benchmark\n'
printf '  spec: %s\n' "$spec_path"
printf '  endpoint: %s\n' "$endpoint"
printf '  mode: %s | angle_unit: %s | timeout: %ss\n' "$mode" "$angle_unit" "$timeout_sec"
printf '  cases: %d\n' "${#cases[@]}"

if [[ "$dry_run" == "true" ]]; then
  printf '\nDry run case list:\n'
  for case_json in "${cases[@]}"; do
    case_id="$(jq -r '.case_id' <<<"$case_json")"
    input_expr="$(jq -r '.input' <<<"$case_json")"
    printf '  - %s :: %s\n' "$case_id" "$input_expr"
  done
  exit 0
fi

pass_count=0
fail_count=0
sum_score="0"

for case_json in "${cases[@]}"; do
  case_id="$(jq -r '.case_id' <<<"$case_json")"
  input_expr="$(jq -r '.input' <<<"$case_json")"
  expected_family="$(jq -r '.required.family_id' <<<"$case_json")"
  expected_canonical="$(jq -r '.required.canonical_form' <<<"$case_json")"
  expected_block_pattern="$(jq -r '.required.benchmark_output_block_pattern' <<<"$case_json")"

  req_body="$(jq -nc --arg expression "$input_expr" --arg mode "$mode" --arg angle "$angle_unit" '{expression: $expression, mode: $mode, angle_unit: $angle}')"

  set +e
  response="$(curl -sS -X POST "$endpoint" -H 'content-type: application/json' --max-time "$timeout_sec" --data "$req_body")"
  curl_rc=$?
  set -e

  if [[ $curl_rc -ne 0 ]]; then
    printf '\n[FAIL] %s\n' "$case_id"
    printf '  reason: request failed (curl exit %d)\n' "$curl_rc"
    fail_count=$((fail_count + 1))
    continue
  fi

  if ! jq empty <<<"$response" >/dev/null 2>&1; then
    printf '\n[FAIL] %s\n' "$case_id"
    printf '  reason: endpoint response is not valid JSON\n'
    fail_count=$((fail_count + 1))
    continue
  fi

  actual_family="$(jq -r '.symbolic_orchestration.family_id // "__MISSING__"' <<<"$response")"
  actual_canonical="$(jq -r '.symbolic_orchestration.canonical_form // "__MISSING__"' <<<"$response")"

  family_pass="false"
  canonical_pass="false"
  tags_pass="true"
  block_pass="false"

  if [[ "$actual_family" == "$expected_family" ]]; then
    family_pass="true"
  fi
  if [[ "$actual_canonical" == "$expected_canonical" ]]; then
    canonical_pass="true"
  fi

  mapfile -t expected_tags < <(jq -r '.required.ontology_tags_all[]' <<<"$case_json")
  for tag in "${expected_tags[@]}"; do
    if ! jq -e --arg tag "$tag" '.symbolic_orchestration.ontology_tags // [] | index($tag) != null' <<<"$response" >/dev/null; then
      tags_pass="false"
      break
    fi
  done

  if jq -e --arg p "$expected_block_pattern" '.symbolic_orchestration.benchmark_output_block // "" | test($p)' <<<"$response" >/dev/null; then
    block_pass="true"
  fi

  case_score="0"
  if [[ "$family_pass" == "true" ]]; then
    case_score="$(awk -v a="$case_score" -v b="$w_family" 'BEGIN{printf "%.6f", a+b}')"
  fi
  if [[ "$canonical_pass" == "true" ]]; then
    case_score="$(awk -v a="$case_score" -v b="$w_canonical" 'BEGIN{printf "%.6f", a+b}')"
  fi
  if [[ "$tags_pass" == "true" ]]; then
    case_score="$(awk -v a="$case_score" -v b="$w_tags" 'BEGIN{printf "%.6f", a+b}')"
  fi
  if [[ "$block_pass" == "true" ]]; then
    case_score="$(awk -v a="$case_score" -v b="$w_block" 'BEGIN{printf "%.6f", a+b}')"
  fi

  sum_score="$(awk -v a="$sum_score" -v b="$case_score" 'BEGIN{printf "%.6f", a+b}')"

  case_pass="$(awk -v s="$case_score" -v t="$pass_threshold" 'BEGIN{ if (s+1e-12 >= t) print "true"; else print "false" }')"

  if [[ "$case_pass" == "true" ]]; then
    printf '\n[PASS] %s score=%s\n' "$case_id" "$case_score"
    pass_count=$((pass_count + 1))
  else
    printf '\n[FAIL] %s score=%s\n' "$case_id" "$case_score"
    fail_count=$((fail_count + 1))
  fi

  printf '  family_id: %s\n' "$family_pass"
  printf '  canonical_form: %s\n' "$canonical_pass"
  printf '  ontology_tags: %s\n' "$tags_pass"
  printf '  benchmark_output_block: %s\n' "$block_pass"
done

total_cases="${#cases[@]}"
avg_score="$(awk -v s="$sum_score" -v n="$total_cases" 'BEGIN{ if (n>0) printf "%.6f", s/n; else print "0" }')"

printf '\nSummary\n'
printf '  passed: %d\n' "$pass_count"
printf '  failed: %d\n' "$fail_count"
printf '  average_score: %s\n' "$avg_score"

if [[ "$fail_count" -gt 0 ]]; then
  exit 1
fi
