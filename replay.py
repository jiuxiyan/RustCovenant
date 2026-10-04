#!/usr/bin/env python3
"""Replay one archived generated suite; never calls a model or changes study scores."""
from pathlib import Path
import argparse, json, os, sys
sys.dont_write_bytecode = True
from audit import program_path
ROOT = Path(__file__).resolve().parent
sys.path.insert(0, str(ROOT / 'code'))
import isolated_evaluator as E

def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--policy', choices=['strict', 'normalized'], required=True)
    p.add_argument('--task', choices=[f'T{i:02}' for i in range(1, 8)], required=True)
    p.add_argument('--method', choices=['B3-generic', 'RustCovenant-Verify'], required=True)
    p.add_argument('--variant', choices=['original', 'fixed', 'B1', 'B2'], required=True)
    p.add_argument('--out', type=Path, required=True)
    p.add_argument('--allow-runtime-drift', action='store_true', help='Diagnostic only; do not describe different-runtime output as a pinned replication.')
    a = p.parse_args()
    if ROOT == a.out.resolve() or ROOT in a.out.resolve().parents:
        p.error('--out must be outside this immutable artifact directory')
    for env, attr in [('RUSTCOV_TOOLCHAIN', 'TOOL'), ('RUSTCOV_MIRI_SYSROOT', 'SYSROOT'), ('RUSTCOV_BWRAP', 'BWRAP')]:
        if not os.environ.get(env):
            p.error('Set ' + env + ' to a local installed runtime path')
        setattr(E, attr, Path(os.environ[env]).resolve())
    study = json.loads((ROOT / 'study.json').read_text())
    task = next(t for t in study['tasks'] if t['id'] == a.task)
    g = next(g for g in study['generation'] if (g['policy'], g['task_id'], g['method']) == (a.policy, a.task, a.method))
    if g['status'] != 'TEST_GENERATED':
        p.error('This policy has no admitted-format program. Use normalized explicitly for the separate post-hoc program, not as an automatic fallback.')
    program = program_path(g)
    assert E.sha(program.read_bytes()) == g['test_sha256']
    source = ROOT / 'sources' / (a.task + '-' + a.variant) if a.variant in ['B1', 'B2'] else ROOT / 'sources' / a.task / a.variant
    expected_source = next(c['source_hashes'] for c in study['candidates'] if c['id'] == a.task + '-' + a.variant) if a.variant in ['B1', 'B2'] else task[a.variant + '_hashes']
    assert E.treehash(source) == expected_source
    contract = ROOT / 'materials' / a.task / 'contract.json'
    assert E.sha(contract.read_bytes()) == task['material_hashes']['contract.json']
    record_path = next((ROOT / 'runs' / 'strict').rglob('record.json'))
    expected = json.loads(record_path.read_text())['context']
    actual = {'miri_sha256': E.sha((E.TOOL / 'bin/miri').read_bytes()),
              'bwrap_sha256': E.sha(E.BWRAP.read_bytes()),
              'evaluator_sha256': E.sha(Path(E.__file__).read_bytes()),
              'sysroot_manifest_sha256': E.sha(E.canonical(E.treehash(E.SYSROOT)))}
    drift = {k: {'expected': expected[k], 'actual': v} for k, v in actual.items() if expected[k] != v}
    if drift and not a.allow_runtime_drift:
        raise SystemExit('Runtime hash mismatch; install pinned runtime or explicitly use --allow-runtime-drift for diagnostic output only: ' + json.dumps(drift))
    result = E.evaluate(source, {'hidden': program}, a.out, models=('SB', 'TB'), timeout=120,
                        ignore_leaks={'hidden': task['leak_policy']['hidden']}, contract_path=contract)
    result['replay_only_not_added_to_study'] = True
    result['runtime_drift'] = drift
    (a.out / 'replay-summary.json').write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps(result, indent=2))

if __name__ == '__main__':
    main()
