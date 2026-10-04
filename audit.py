#!/usr/bin/env python3
"""Offline audit of the unsigned review derivative. Python 3.11+, stdlib only."""
from pathlib import Path
import hashlib, json, re, sys
sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parent
METHODS = ['B3-generic', 'RustCovenant-Verify']
POLICIES = ['strict', 'normalized']

def sha(data):
    return hashlib.sha256(data).hexdigest()

def get(path):
    return json.loads(path.read_text())

def tree(path):
    return {str(p.relative_to(path)): sha(p.read_bytes()) for p in sorted(path.rglob('*')) if p.is_file()}

def program_path(generation):
    """Both policies share strict.rs when strict parsing produced a program."""
    directory = ROOT / 'generation' / generation['task_id'] / generation['method']
    if generation['policy'] == 'strict' or (directory / 'strict.rs').is_file():
        return directory / 'strict.rs'
    return directory / 'normalized.rs'

def parse(text, policy):
    if policy == 'normalized':
        if len(text) > 24000:
            raise ValueError('original response cap')
        m = re.fullmatch(r'\s*```(?:json)?[ \t]*\r?\n(.*?)\r?\n```\s*', text, flags=re.DOTALL)
        if m:
            text = m.group(1)
    if text.strip() == 'ABSTAIN':
        return None
    if len(text) > 24000:
        raise ValueError('response cap')
    x = json.loads(text)
    assert set(x) == {'test_rs', 'rationale', 'coverage'}
    assert isinstance(x['test_rs'], str) and isinstance(x['coverage'], list)
    s = x['test_rs']
    assert 'fn main' in s and re.search(r'\bassert(?:_eq|_ne)?!', s)
    forbidden = [r'\bunsafe\b', r'std\s*::\s*(?:fs|net|process|env)\b',
                 r'\b(?:include|include_str|include_bytes|env|option_env|asm|global_asm)!',
                 r'\bextern\s*"', r'#\s*\[\s*(?:cfg|cfg_attr|test|path)', r'\bset_var\s*\(']
    assert not any(re.search(p, s) for p in forbidden)
    return s

def classify(code, text):
    if code is None:
        return 'TIMEOUT'
    if code == 0:
        return 'PASS'
    if 'error: Undefined Behavior' in text:
        return 'UB'
    if 'panicked at' in text:
        return 'ASSERT_OR_PANIC'
    return 'ERROR'

def verify_integrity():
    entries = {}
    for line in (ROOT / 'SHA256SUMS').read_text().splitlines():
        digest, rel = line.split('  ', 1)
        assert not Path(rel).is_absolute() and '..' not in Path(rel).parts
        p = ROOT / rel
        assert p.is_file() and not p.is_symlink(), rel
        assert sha(p.read_bytes()) == digest, rel
        assert rel not in entries
        entries[rel] = digest
    actual = {str(p.relative_to(ROOT)) for child in ROOT.iterdir() if child.name != '.git'
              for p in ([child] if child.is_file() else child.rglob('*'))
              if p.is_file() and p != ROOT / 'SHA256SUMS' and '__pycache__' not in p.parts}
    assert actual == set(entries), (actual - set(entries), set(entries) - actual)
    return len(entries)

def main():
    checked_files = verify_integrity()
    study = get(ROOT / 'study.json')
    assert study['no_task_exclusions'] and study['new_model_calls'] == 0
    assert study['post_hoc_sensitivity'] and not study['signed_records_included']
    tasks = {t['id']: t for t in study['tasks']}
    candidates = {c['id']: c for c in study['candidates']}
    assert set(tasks) == {f'T{i:02}' for i in range(1, 8)}
    assert set(candidates) == {t + '-' + b for t in tasks for b in ['B1', 'B2']}
    for tid, t in tasks.items():
        for variant in ['original', 'fixed']:
            assert tree(ROOT / 'sources' / tid / variant) == t[variant + '_hashes']
        assert tree(ROOT / 'materials' / tid) == t['material_hashes']
    for cid, c in candidates.items():
        assert tree(ROOT / 'sources' / cid) == c['source_hashes']
    generations = {}
    for g in study['generation']:
        key = (g['policy'], g['task_id'], g['method'])
        assert key not in generations
        d = ROOT / 'generation' / g['task_id'] / g['method']
        assert sha((d / 'prompt.txt').read_bytes()) == g['prompt_sha256']
        assert sha((d / 'response.txt').read_bytes()) == g['response_sha256']
        try:
            s = parse((d / 'response.txt').read_text(), g['policy'])
            status = 'ABSTAIN' if s is None else 'TEST_GENERATED'
        except (ValueError, AssertionError, TypeError):
            status, s = 'GENERATION_ERROR', None
        assert status == g['status'], (key, status, g['status'])
        if s is not None:
            assert s.encode() == program_path(g).read_bytes()
            assert sha(s.encode()) == g['test_sha256']
        generations[key] = g
    expected_keys = {(p, t, m) for p in POLICIES for t in tasks for m in METHODS}
    assert set(generations) == expected_keys
    records, status_maps = {}, {}
    total_processes = 0
    for rp in sorted((ROOT / 'runs').rglob('record.json')):
        rel = str(rp.parent.relative_to(ROOT))
        rec = get(rp)
        assert rec['scope'].startswith('Unsigned review derivative')
        assert all(rec['execution_flags'].values()), rel
        assert sha((rp.parent / 'contract.json').read_bytes()) == rec['context']['contract_sha256']
        expected_processes = {'compile'} | {Path(h).stem + '-' + m for h in rec['context']['harness'] for m in rec['context']['models']}
        assert set(rec['processes']) == expected_processes
        statuses = {}
        for name, proc in rec['processes'].items():
            data = (rp.parent / (name + '.log')).read_bytes()
            assert sha(data) == proc['log_sha256']
            status = classify(proc['returncode'], data.decode(errors='replace'))
            assert status == proc['status'], (rel, name)
            statuses[name] = status
            total_processes += 1
        records[rel], status_maps[rel] = rec, statuses
    assert len(records) == 46 and total_processes == 194
    derivation = get(ROOT / 'derivation.json')
    changed = {x['file']: x for x in derivation['changed_logs']}
    found = set()
    for rel, rec in records.items():
        for name, p in rec['processes'].items():
            key = rel + '/' + name + '.log'
            if p['path_normalization_applied']:
                x = changed[key]
                assert x['original_sha256'] == p['original_log_sha256']
                assert x['review_sha256'] == p['log_sha256']
                found.add(key)
            else:
                assert p['original_log_sha256'] == p['log_sha256']
    assert found == set(changed) and len(found) == derivation['changed_log_count'] == 11
    def check_binding(run, source_hashes, harness_hashes, tid):
        c = records[run]['context']
        assert c['source'] == source_hashes, run
        assert c['harness'] == harness_hashes, run
        assert c['contract_sha256'] == tasks[tid]['material_hashes']['contract.json']
        for name in harness_hashes:
            assert c['leak_policy_by_suite'][Path(name).stem] == tasks[tid]['leak_policy'][Path(name).stem]
    labels, gates = {}, {}
    for cid, c in candidates.items():
        tid = c['task_id']; run = c['reference_run']
        check_binding(run, c['source_hashes'], {k: tasks[tid]['material_hashes'][k] for k in ['witness.rs', 'ordinary.rs', 'hidden.rs']}, tid)
        statuses = status_maps[run]
        label = 'SUPPORTED' if all(v == 'PASS' for v in statuses.values()) else 'VIOLATION'
        # Oracle uncertainty: an empty-state alignment assertion exceeds C0.
        # This is not silently relabeled supported or a confirmed violation.
        if cid == 'T06-B1':
            label = 'ORACLE_UNCERTAIN'
        assert label == c['reference_label'], cid
        labels[cid] = label
        gates[cid] = {
            'B1-witness': all(statuses[k] == 'PASS' for k in ['compile', 'witness-SB', 'witness-TB']),
            'B2-ordinary': all(statuses[k] == 'PASS' for k in ['compile', 'witness-SB', 'witness-TB', 'ordinary-SB', 'ordinary-TB'])}
    def summarize(policy, method, decisions, g):
        row = dict(policy=policy, method=method, N=len(decisions), A=0, S=0, V=0, U=0, R=0, Ab=0, G=g)
        for cid, decision in decisions.items():
            row[{'ACCEPT': 'A', 'REJECT': 'R', 'ABSTAIN': 'Ab'}[decision]] += 1
            if decision == 'ACCEPT':
                row[{'SUPPORTED': 'S', 'VIOLATION': 'V', 'ORACLE_UNCERTAIN': 'U'}[labels[cid]]] += 1
        assert row['N'] == 14 and row['A'] + row['R'] + row['Ab'] == 14
        assert row['S'] + row['V'] + row['U'] == row['A']
        return row
    all_decisions = {(p, m): {} for p in POLICIES for m in METHODS}
    admitted = {(p, m): 0 for p in POLICIES for m in METHODS}
    used_runs = {c['reference_run'] for c in candidates.values()}
    result_keys = set()
    for result in study['results']:
        p, tid, m = result['policy'], result['task_id'], result['method']
        key = (p, tid, m)
        assert key not in result_keys
        result_keys.add(key)
        g = generations[key]; test = g.get('test_sha256'); fixed = result['fixed_run']
        assert result['generation_status'] == g['status']
        assert bool(fixed) == (g['status'] == 'TEST_GENERATED')
        ok = False
        if fixed:
            used_runs.add(fixed)
            check_binding(fixed, tasks[tid]['fixed_hashes'], {'hidden.rs': test}, tid)
            ok = all(v == 'PASS' for v in status_maps[fixed].values())
        admitted[(p, m)] += int(ok)
        assert {c['id'] for c in result['candidates']} == {tid + '-B1', tid + '-B2'}
        for candidate in result['candidates']:
            cid, run = candidate['id'], candidate['run']
            assert bool(run) == ok
            if not ok:
                decision = 'ABSTAIN'
            else:
                used_runs.add(run)
                check_binding(run, candidates[cid]['source_hashes'], {'hidden.rs': test}, tid)
                values = list(status_maps[run].values())
                if gates[cid]['B2-ordinary'] and all(v == 'PASS' for v in values):
                    decision = 'ACCEPT'
                elif any(v in ['TIMEOUT', 'UNKNOWN', 'UNSUPPORTED', 'ERROR'] for v in values):
                    decision = 'ABSTAIN'
                else:
                    decision = 'REJECT'
            assert decision == candidate['stored_decision'], (key, cid, decision)
            assert cid not in all_decisions[(p, m)]
            all_decisions[(p, m)][cid] = decision
    assert result_keys == expected_keys and used_runs == set(records)
    rows = [summarize(p, m, all_decisions[(p, m)], admitted[(p, m)]) for p in POLICIES for m in METHODS]
    assert rows == get(ROOT / 'expected-comparison.json')
    base_rows = [summarize('strict', m, {c: 'ACCEPT' if gates[c][m] else 'REJECT' for c in candidates}, None) for m in ['B1-witness', 'B2-ordinary']]
    assert all((r['A'], r['S'], r['V'], r['U'], r['R'], r['Ab']) == (13, 10, 2, 1, 1, 0) for r in base_rows)
    runtime_fields = ['rust_commit', 'miri_sha256', 'bwrap_sha256', 'evaluator_sha256', 'sysroot_manifest_sha256']
    runtimes = [{k: r['context'][k] for k in runtime_fields} for key, r in records.items() if '/reference/' not in key]
    assert all(x == runtimes[0] for x in runtimes)
    print(json.dumps({'status': 'PASS', 'integrity_files': checked_files, 'tasks': 7, 'candidates': 14,
                      'derivative_runs': len(records), 'processes_reclassified': total_processes,
                      'decisions_recomputed': 56, 'baseline_decisions_recomputed': 28,
                      'base_rows': base_rows, 'rows': rows,
                      'scope': 'Integrity and deterministic recomputation of unsigned derivative; no independent authentication, semantic proof, or independent-machine replication.'}, indent=2))

if __name__ == '__main__':
    main()
