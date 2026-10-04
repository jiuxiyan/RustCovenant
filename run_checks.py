#!/usr/bin/env python3
"""Offline comparison audit plus finite protocol tests; writes only to a temporary directory."""
from pathlib import Path
import json, shutil, subprocess, sys, tempfile
ROOT = Path(__file__).resolve().parent

def main():
    completed = subprocess.run([sys.executable, '-B', str(ROOT / 'audit.py')], text=True, capture_output=True, check=True)
    comparison = json.loads(completed.stdout)
    with tempfile.TemporaryDirectory(prefix='rustcovenant-protocol-') as temp:
        target = Path(temp) / 'protocol'
        shutil.copytree(ROOT / 'supplements' / 'protocol', target, ignore=shutil.ignore_patterns('__pycache__', '*.pyc'))
        done = subprocess.run([sys.executable, '-B', str(target / 'test_protocol.py')], cwd=target, text=True, capture_output=True, check=True)
        protocol = json.loads(done.stdout)
        assert protocol['total_cases'] == 1368 and protocol['assertions_passed']
    print(json.dumps({'status': 'PASS', 'comparison': comparison, 'protocol': protocol,
                      'scope': 'Offline recomputation and protocol model; not independent semantic adjudication or full historical replay.'}, indent=2))

if __name__ == '__main__':
    main()
