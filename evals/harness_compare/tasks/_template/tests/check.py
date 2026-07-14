#!/usr/bin/env python3
import json
import sys
from pathlib import Path

ws = Path(sys.argv[1])
ok = (ws / "DONE.txt").is_file()
print(json.dumps({"correct": ok, "score": 1.0 if ok else 0.0, "metrics": {}, "detail": "DONE.txt"}))
