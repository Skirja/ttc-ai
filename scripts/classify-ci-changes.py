#!/usr/bin/env python3
"""Write the full_gate workflow output, defaulting to the full CI gate."""
import os
from pathlib import Path
import sys

sys.dont_write_bytecode = True
from ci_change_policy import full_gate_for_event


event = os.environ.get("EVENT_NAME", "")
full_gate = full_gate_for_event(event, os.environ)
value = "true" if full_gate else "false"
output = os.environ.get("GITHUB_OUTPUT", "")
if not output:
    raise SystemExit("GITHUB_OUTPUT is required")
with Path(output).open("a", encoding="utf-8") as stream:
    stream.write(f"full_gate={value}\n")
print(f"full_gate={value}")
