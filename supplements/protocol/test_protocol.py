from pathlib import Path
from dataclasses import replace
import csv,json,itertools,sys,collections
sys.path.insert(0,str(Path(__file__).parent))
from evidence import Context,TrustedRunner,Arbiter,FIELDS,REQUIRED,STATES,digest
root=Path(__file__).parent
ctx=Context(**{f:digest(f.encode()) for f in FIELDS})
runner=TrustedRunner(); arbiter=Arbiter(runner,ctx.contract)
base=[runner.issue(ctx,k,'PASS',('log:'+k).encode()) for k in REQUIRED]
rows=[]
for bits in itertools.product([False,True],repeat=len(FIELDS)):
 changes={f:digest(('changed:'+f).encode()) for f,b in zip(FIELDS,bits) if b}
 current=replace(ctx,**changes)
 actual=arbiter.decide(current,base)
 rows.append({'family':'context','case':'+'.join(changes) or 'unchanged','expected_accept':not bool(changes),'full_accept':actual=='ACCEPT_OBSERVED_SCOPE','source_only_accept':current.source==ctx.source,'exit_only_accept':True,'decision':actual})
 assert (actual=='ACCEPT_OBSERVED_SCOPE')== (not changes)
for states in itertools.product(STATES,repeat=len(REQUIRED)):
 records=[runner.issue(ctx,k,s,(k+':'+s).encode()) for k,s in zip(REQUIRED,states)]
 actual=arbiter.decide(ctx,records)
 expected=all(s=='PASS' for s in states)
 rows.append({'family':'status','case':','.join(states),'expected_accept':expected,'full_accept':actual=='ACCEPT_OBSERVED_SCOPE','source_only_accept':'','exit_only_accept':'','decision':actual})
 assert (actual=='ACCEPT_OBSERVED_SCOPE')==expected
negative={
 'missing':base[:-1],
 'duplicate':base+[base[0]],
 'forged_state':[replace(base[0],state='FAIL')]+base[1:],
 'forged_log':[replace(base[0],log_digest='0'*64)]+base[1:],
 'forged_context':[replace(base[0],context=replace(ctx,source='0'*64))]+base[1:],
 'forged_signature':[replace(base[0],signature='0'*64)]+base[1:],
 'foreign_runner':[TrustedRunner().issue(ctx,k,'PASS',b'foreign') for k in REQUIRED],
 'conflicting_authentic':base+[runner.issue(ctx,'semantics','FAIL',b'counterexample')]
}
for name,ev in negative.items():
 actual=arbiter.decide(ctx,ev);assert actual!='ACCEPT_OBSERVED_SCOPE'
 rows.append({'family':'negative','case':name,'expected_accept':False,'full_accept':False,'source_only_accept':'','exit_only_accept':'','decision':actual})
with (root/'protocol-results.csv').open('w',newline='') as f:
 w=csv.DictWriter(f,fieldnames=rows[0].keys());w.writeheader();w.writerows(rows)
summary={'context_cases':64,'invalid_contexts':63,'full_false_accepts':sum(r['full_accept'] and not r['expected_accept'] for r in rows if r['family']=='context'),'source_only_false_accepts':sum(r['source_only_accept'] and not r['expected_accept'] for r in rows if r['family']=='context'),'exit_only_false_accepts':63,'status_cases':len(STATES)**len(REQUIRED),'status_accepts':sum(r['full_accept'] for r in rows if r['family']=='status'),'negative_cases':len(negative),'negative_rejected':len(negative),'total_cases':len(rows),'assertions_passed':True,'interpretation':'finite constructed protocol tests, not benchmark repair accuracy or OS isolation'}
(root/'protocol-summary.json').write_text(json.dumps(summary,indent=2))
print(json.dumps(summary,indent=2))
