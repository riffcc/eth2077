#!/usr/bin/env bash
set -euo pipefail

registry_file="theorem-registry.json"

if [ ! -f "$registry_file" ]; then
  echo "Missing registry file: $registry_file"
  exit 1
fi

python3 -c '
import collections
import json
import sys

path = "theorem-registry.json"
required = {"id", "name", "tier", "status"}

try:
    with open(path, "r", encoding="utf-8") as f:
        data = json.load(f)
except json.JSONDecodeError as e:
    print(f"Invalid JSON in {path}: {e}")
    sys.exit(1)
except OSError as e:
    print(f"Failed to read {path}: {e}")
    sys.exit(1)

if isinstance(data, dict):
    entries = data.get("entries")
elif isinstance(data, list):
    entries = data
else:
    print("Registry must be a JSON object with an entries array or a top-level array.")
    sys.exit(1)

if not isinstance(entries, list):
    print("Registry entries must be an array.")
    sys.exit(1)

errors = []
tier_counts = collections.Counter()

def is_tier1(value):
    if isinstance(value, int):
        return value == 1
    if isinstance(value, str):
        normalized = value.strip().lower().replace("-", "").replace(" ", "")
        return normalized in {"1", "tier1"}
    return False

for idx, entry in enumerate(entries, start=1):
    if not isinstance(entry, dict):
        errors.append(f"Entry #{idx} is not an object.")
        continue

    missing = sorted(required - set(entry.keys()))
    if missing:
        errors.append("Entry #{} is missing required fields: {}".format(idx, ", ".join(missing)))

    tier = entry.get("tier")
    status = entry.get("status")
    tier_counts[str(tier)] += 1

    if is_tier1(tier) and isinstance(status, str) and status.strip().lower() == "stub":
        entry_id = entry.get("id", f"#{idx}")
        errors.append(f"Tier-1 theorem cannot have status stub: {entry_id}")

if errors:
    print("Theorem registry lint failed:")
    for err in errors:
        print(f"- {err}")
    print("Tier summary:")
    for tier in sorted(tier_counts):
        print(f"- {tier}: {tier_counts[tier]}")
    sys.exit(1)

print("Theorem registry lint passed.")
print("Tier summary:")
for tier in sorted(tier_counts):
    print(f"- {tier}: {tier_counts[tier]}")
'
