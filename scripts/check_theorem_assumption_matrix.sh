#!/usr/bin/env bash
set -euo pipefail

registry_file="theorem-registry.json"
assumption_ledger_file="security/assumption_ledger.json"

if [ ! -f "$registry_file" ]; then
  echo "Missing theorem registry file: $registry_file"
  exit 1
fi

if [ ! -f "$assumption_ledger_file" ]; then
  echo "Missing assumption ledger file: $assumption_ledger_file"
  exit 1
fi

python3 -c '
import json
import sys
from collections import defaultdict

registry_path = "theorem-registry.json"
assumption_path = "security/assumption_ledger.json"


def load_json(path):
    try:
        with open(path, "r", encoding="utf-8") as f:
            return json.load(f)
    except json.JSONDecodeError as e:
        print(f"Invalid JSON in {path}: {e}")
        sys.exit(1)
    except OSError as e:
        print(f"Failed to read {path}: {e}")
        sys.exit(1)


def read_theorems(payload):
    if isinstance(payload, dict):
        entries = payload.get("entries")
    elif isinstance(payload, list):
        entries = payload
    else:
        print("Theorem registry must be an object with an entries array or a top-level array.")
        sys.exit(1)

    if not isinstance(entries, list):
        print("Theorem registry entries must be an array.")
        sys.exit(1)
    return entries


def read_assumptions(payload):
    if isinstance(payload, dict):
        assumptions = payload.get("assumptions")
    elif isinstance(payload, list):
        assumptions = payload
    else:
        print("Assumption ledger must be an object with an assumptions array or a top-level array.")
        sys.exit(1)

    if not isinstance(assumptions, list):
        print("Assumption ledger assumptions must be an array.")
        sys.exit(1)
    return assumptions


def normalize_status(value):
    if value is None:
        return ""
    return str(value).strip().lower()


def format_ids(values):
    if not values:
        return "(none)"
    return ", ".join(values)


theorems = read_theorems(load_json(registry_path))
assumptions = read_assumptions(load_json(assumption_path))

assumption_by_id = {}
missing_anchor = []
duplicate_assumptions = []

for idx, assumption in enumerate(assumptions, start=1):
    if not isinstance(assumption, dict):
        continue
    assumption_id = str(assumption.get("id", "")).strip()
    if not assumption_id:
        continue

    if assumption_id in assumption_by_id:
        duplicate_assumptions.append(assumption_id)
    assumption_by_id[assumption_id] = assumption

    anchor = assumption.get("threat_model_anchor")
    if not isinstance(anchor, str) or not anchor.strip():
        missing_anchor.append(assumption_id)

referenced_assumptions = set()
violated_assumptions_referenced_by = defaultdict(list)

matrix_lines = []
theorem_total = 0
theorem_covered = 0
theorem_uncovered = 0

unknown_ref_count = 0
violated_ref_count = 0
duplicate_ref_count = 0

for idx, theorem in enumerate(theorems, start=1):
    theorem_total += 1
    theorem_id = f"THM-UNKNOWN-{idx}"
    tags = []

    if isinstance(theorem, dict):
        raw_id = theorem.get("id")
        if isinstance(raw_id, str) and raw_id.strip():
            theorem_id = raw_id.strip()
        raw_tags = theorem.get("tags")
        if isinstance(raw_tags, list):
            tags = raw_tags

    refs = []
    for tag in tags:
        if isinstance(tag, str) and tag.startswith("assumption:"):
            assumption_id = tag.split(":", 1)[1].strip()
            if assumption_id:
                refs.append(assumption_id)

    refs_unique = []
    seen = set()
    for ref in refs:
        if ref in seen:
            duplicate_ref_count += 1
            continue
        refs_unique.append(ref)
        seen.add(ref)

    if not refs_unique:
        theorem_uncovered += 1
        matrix_lines.append(f"{theorem_id}: (none) [FAIL: no assumptions referenced]")
        continue

    unknown_refs = [ref for ref in refs_unique if ref not in assumption_by_id]
    for ref in unknown_refs:
        unknown_ref_count += 1

    violated_refs = []
    deprecated = []
    valid_refs = []

    for ref in refs_unique:
        assumption = assumption_by_id.get(ref)
        if assumption is None:
            continue

        referenced_assumptions.add(ref)
        status = normalize_status(assumption.get("status"))

        if status == "violated":
            violated_refs.append(ref)
            violated_assumptions_referenced_by[ref].append(theorem_id)
            violated_ref_count += 1
            continue

        if status == "deprecated":
            deprecated.append(ref)

        valid_refs.append(ref)

    has_valid_reference = len(valid_refs) > 0
    if has_valid_reference:
        theorem_covered += 1
    else:
        theorem_uncovered += 1

    line_status = "OK"
    reasons = []
    if unknown_refs:
        reasons.append(f"unknown assumptions: {format_ids(unknown_refs)}")
    if violated_refs:
        reasons.append(f"references violated assumptions: {format_ids(violated_refs)}")
    if not has_valid_reference:
        reasons.append("no valid assumptions referenced")

    if reasons:
        line_status = "FAIL: " + "; ".join(reasons)
    elif deprecated:
        line_status = f"WARN: deprecated assumptions referenced: {format_ids(deprecated)}"

    matrix_lines.append(f"{theorem_id}: {format_ids(refs_unique)} [{line_status}]")

orphaned = []
for assumption_id in assumption_by_id:
    if assumption_id not in referenced_assumptions:
        orphaned.append(assumption_id)

orphaned.sort()
missing_anchor.sort()

print("=== Theorem-to-Assumption Coverage Matrix ===")
for line in matrix_lines:
    print(line)

print("")
print("=== Orphaned Assumptions ===")
if orphaned:
    for assumption_id in orphaned:
        print(f"{assumption_id}: not referenced by any theorem [WARN]")
else:
    print("(none)")

print("")
print("=== Violated Assumptions ===")
if violated_assumptions_referenced_by:
    for assumption_id in sorted(violated_assumptions_referenced_by):
        refs = ", ".join(sorted(set(violated_assumptions_referenced_by[assumption_id])))
        print(f"{assumption_id}: referenced by {refs} [FAIL]")
else:
    print("(none)")

if missing_anchor:
    print("")
    print("=== Assumptions Missing Threat Model Anchors ===")
    for assumption_id in missing_anchor:
        print(f"{assumption_id}: threat_model_anchor is missing [WARN]")

print("")
print("=== Summary ===")
print(f"Theorems: {theorem_total} total, {theorem_covered} covered, {theorem_uncovered} uncovered")
print(f"Assumptions: {len(assumption_by_id)} total, {len(referenced_assumptions)} referenced, {len(orphaned)} orphaned")
if missing_anchor:
    print(f"Threat anchors: {len(missing_anchor)} missing [WARN]")
if duplicate_assumptions:
    unique_dups = sorted(set(duplicate_assumptions))
    print(f"Assumption IDs: {len(unique_dups)} duplicated [FAIL]")
if duplicate_ref_count:
    print(f"Theorem assumption tags: {duplicate_ref_count} duplicate references ignored [WARN]")

should_fail = False
fail_reasons = []

if theorem_uncovered > 0:
    should_fail = True
    fail_reasons.append(f"{theorem_uncovered} theorem(s) have no valid assumption coverage")
if violated_ref_count > 0:
    should_fail = True
    fail_reasons.append(f"{violated_ref_count} violated assumption reference(s)")
if unknown_ref_count > 0:
    should_fail = True
    fail_reasons.append(f"{unknown_ref_count} unknown assumption reference(s)")
if duplicate_assumptions:
    should_fail = True
    fail_reasons.append(f"{len(set(duplicate_assumptions))} duplicate assumption id(s)")

if should_fail:
    reason_text = "; ".join(fail_reasons)
    print(f"Status: FAIL ({reason_text})")
    sys.exit(1)

print("Status: PASS")
'
