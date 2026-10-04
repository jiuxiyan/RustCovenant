#!/usr/bin/env python3
"""Narrow protected evaluator for dependency-free library crates.
No Cargo/build scripts/proc macros; unsupported manifests fail closed.
Candidate compilation cannot see harness. Interpreter cannot see source tree.
This is a local research service, not a hardened multi-tenant service.
"""
from pathlib import Path
import datetime,hashlib,json,os,resource,secrets,signal,subprocess,tempfile,time,tomllib
from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey,Ed25519PublicKey
from cryptography.hazmat.primitives.serialization import Encoding,PublicFormat
ROOT=Path(__file__).resolve().parent
RUNTIME=ROOT.parent/'rustcovenant-v1/runtime'
TOOL=RUNTIME/'rustup/toolchains/nightly-2025-03-01-x86_64-unknown-linux-gnu'
SYSROOT=RUNTIME/'cache/miri'
BWRAP=ROOT/'tooling/usr/bin/bwrap'
def canonical(x):return json.dumps(x,sort_keys=True,separators=(',',':')).encode()
def sha(b):return hashlib.sha256(b).hexdigest()
def treehash(p):return {str(f.relative_to(p)):sha(f.read_bytes()) for f in sorted(p.rglob('*')) if f.is_file()}
def snapshot(src,dst):
 dst.mkdir(parents=True)
 for f in sorted(src.rglob('*')):
  if f.is_symlink():raise ValueError('Symlinks are not allowed')
  if f.is_file():
   p=dst/f.relative_to(src);p.parent.mkdir(parents=True,exist_ok=True);p.write_bytes(f.read_bytes());p.chmod(0o444)
def validate_manifest(p):
 m=tomllib.loads((p/'Cargo.toml').read_text());pkg=m['package'];lib=m.get('lib',{})
 if pkg.get('build') or (p/'build.rs').exists() or lib.get('proc-macro'):raise ValueError('Unsupported build script or procedural macro')
 if m.get('build-dependencies'):raise ValueError('Unsupported build dependencies')
 if m.get('target'):raise ValueError('Target-specific configurations require a separate evaluator')
 features=m.get('features',{});active={'default'} if 'default' in features else set();pending=list(features.get('default',[]))
 while pending:
  f=pending.pop()
  if '/' in f or f.startswith('dep:'):raise ValueError('Dependency features unsupported')
  if f not in active:active.add(f);pending.extend(features.get(f,[]))
 for name,dep in m.get('dependencies',{}).items():
  if isinstance(dep,str) or not dep.get('optional',False) or name in active:raise ValueError('Nonoptional external dependency unsupported: '+name)
 version=pkg.get('rust-version')
 if version and tuple(map(int,version.split('.')[:2]))>(1,87):raise ValueError('MSRV newer than pinned Rust 1.87')
 return {'crate_name':lib.get('name',pkg['name'].replace('-','_')),'edition':pkg.get('edition','2015'),'lib_path':lib.get('path','src/lib.rs'),'features':sorted(active),'package':pkg['name'],'version':pkg['version']}
def base_command(mounts):
 cmd=[str(BWRAP),'--unshare-all','--die-with-parent','--new-session','--cap-drop','ALL','--clearenv','--ro-bind','/usr','/usr','--ro-bind','/lib','/lib','--ro-bind','/lib64','/lib64','--ro-bind',str(TOOL),'/toolchain','--ro-bind',str(SYSROOT),'/sysroot','--tmpfs','/tmp','--dev','/dev','--chdir','/tmp']
 for src,dst,writable in mounts:cmd+=['--bind' if writable else '--ro-bind',str(src),dst]
 for k,v in {'PATH':'/toolchain/bin:/usr/bin:/bin','LD_LIBRARY_PATH':'/toolchain/lib','HOME':'/tmp','LANG':'C.UTF-8','LC_ALL':'C.UTF-8'}.items():cmd+=['--setenv',k,v]
 return cmd

def limits():
 resource.setrlimit(resource.RLIMIT_CORE,(0,0));resource.setrlimit(resource.RLIMIT_AS,(6*1024**3,6*1024**3));resource.setrlimit(resource.RLIMIT_FSIZE,(64*1024**2,64*1024**2));resource.setrlimit(resource.RLIMIT_NOFILE,(256,256));resource.setrlimit(resource.RLIMIT_CPU,(100,100))

def execute(cmd,log,timeout):
 start=time.monotonic()
 with log.open('wb') as f:
  p=subprocess.Popen(cmd,stdin=subprocess.DEVNULL,stdout=f,stderr=subprocess.STDOUT,close_fds=True,start_new_session=True,preexec_fn=limits)
  try:rc=p.wait(timeout=timeout)
  except subprocess.TimeoutExpired:
   os.killpg(p.pid,signal.SIGKILL);p.wait();rc=None
 text=log.read_text(errors='replace')
 status='TIMEOUT' if rc is None else 'PASS' if rc==0 else 'UB' if 'error: Undefined Behavior' in text else 'ASSERT_OR_PANIC' if 'panicked at' in text else 'ERROR'
 return {'status':status,'returncode':rc,'elapsed_s':round(time.monotonic()-start,3),'log_sha256':sha(log.read_bytes()),'command':cmd}

def verify_record(record,public_hex,expected_context,logroot):
 try:
  payload=record['payload'];Ed25519PublicKey.from_public_bytes(bytes.fromhex(public_hex)).verify(bytes.fromhex(record['signature']),canonical(payload))
  if payload['context_hash']!=expected_context:return False
  for name,r in payload['processes'].items():
   if sha((Path(logroot)/(name+'.log')).read_bytes())!=r['log_sha256']:return False
  return True
 except Exception:return False

def evaluate(source,harnesses,out,models=('SB','TB'),timeout=120,ignore_leaks=False,hook=None,contract_path=None):
 source=Path(source).resolve();harnesses={k:Path(v).resolve() for k,v in harnesses.items()};out=Path(out).resolve();out.mkdir(parents=True,exist_ok=False)
 key=Ed25519PrivateKey.generate();pub=key.public_key().public_bytes(Encoding.Raw,PublicFormat.Raw).hex()
 (out/'public-key.txt').write_text(pub+'\n')
 original=treehash(source);before_h={k:sha(v.read_bytes()) for k,v in harnesses.items()};processes={};context={}
 contract_bytes=Path(contract_path).read_bytes() if contract_path else None
 contract_hash=sha(contract_bytes) if contract_bytes is not None else None
 leak_policy={s:bool(ignore_leaks.get(s,False)) if isinstance(ignore_leaks,dict) else bool(ignore_leaks) for s in harnesses}
 if contract_bytes is not None:(out/'contract.json').write_bytes(contract_bytes)
 with tempfile.TemporaryDirectory(prefix='rc-protected-v3-') as td:
  w=Path(td);src=w/'source';snapshot(source,src);meta=validate_manifest(src)
  hs=w/'harness';hs.mkdir()
  for name,p in harnesses.items():
   if name not in ('witness','ordinary','hidden'):raise ValueError('Invalid suite')
   (hs/(name+'.rs')).write_bytes(p.read_bytes())
  build=w/'build';build.mkdir()
  context={'protocol':'direct-miri-v3','run_id':secrets.token_hex(16),'utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'source':treehash(src),'harness':treehash(hs),'metadata':meta,'models':list(models),'timeout_s':timeout,'ignore_leaks':ignore_leaks,'rust_commit':'287487624357c19b22d27aa3ed584b8ccd080b4d','bwrap_sha256':sha(BWRAP.read_bytes()),'miri_sha256':sha((TOOL/'bin/miri').read_bytes()),'sysroot':treehash(SYSROOT),'evaluator_sha256':sha(Path(__file__).read_bytes()),'mount_policy':'no procfs; no host home/network; compile source-only then execute harness+metadata-only; read-only protected inputs','limits':{'address_space_bytes':6*1024**3,'cpu_seconds':100,'file_bytes':64*1024**2,'open_files':256},'public_key':pub}
  context.update({'protocol':'direct-miri-v3-r2','contract_sha256':contract_hash,'leak_policy_by_suite':leak_policy})
  ch=sha(canonical(context));(out/'context.json').write_text(json.dumps(context,indent=2))
  if hook:hook('snapshot-created',{'source':src,'harness':hs,'build':build,'context':context})
  c=base_command([(src,'/source',False),(build,'/build',True)])+['--','/usr/bin/env','MIRI_BE_RUSTC=target','/toolchain/bin/miri','--sysroot','/sysroot','--crate-name',meta['crate_name'],'--crate-type','lib','--edition='+meta['edition'],'--emit=metadata','--cap-lints=allow','/source/'+meta['lib_path'],'-o','/build/libcandidate.rmeta']
  for f in meta['features']:c+=['--cfg','feature="'+f+'"']
  processes['compile']=execute(c,out/'compile.log',timeout)
  if processes['compile']['status']=='PASS':
   for suite in harnesses:
    for model in models:
     if model not in ('SB','TB'):raise ValueError('Invalid memory model')
     name=suite+'-'+model;args=['/toolchain/bin/miri','--sysroot','/sysroot','--edition=2021','--cap-lints=allow','--extern',meta['crate_name']+'=/build/libcandidate.rmeta','-Zmiri-seed=0']
     if model=='TB':args+=['-Zmiri-tree-borrows']
     if leak_policy[suite]:args+=['-Zmiri-ignore-leaks']
     args+=['/harness/'+suite+'.rs']
     processes[name]=execute(base_command([(hs,'/harness',False),(build,'/build',False)])+['--']+args,out/(name+'.log'),timeout)
  snapshot_unchanged=treehash(src)==context['source'] and treehash(hs)==context['harness']
  contract_unchanged=contract_bytes is None or (Path(contract_path).read_bytes()==contract_bytes and (out/'contract.json').read_bytes()==contract_bytes)
  snapshot_unchanged=snapshot_unchanged and contract_unchanged
  expected={'compile'}|{s+'-'+m for s in harnesses for m in models}
  complete=set(processes)==expected
  outcome='OBSERVED_SCOPE_PASS' if complete and snapshot_unchanged and all(r['status']=='PASS' for r in processes.values()) else 'REJECT_OR_UNKNOWN'
  payload={'context_hash':ch,'run_id':context['run_id'],'processes':processes,'complete':complete,'snapshot_unchanged':snapshot_unchanged,'original_inputs_unchanged':treehash(source)==original and {k:sha(v.read_bytes()) for k,v in harnesses.items()}==before_h,'artifact_sha256':treehash(build),'outcome':outcome}
  record={'payload':payload,'signature':key.sign(canonical(payload)).hex()};(out/'evidence.json').write_text(json.dumps(record,indent=2));verified=verify_record(record,pub,ch,out);assert verified
  summary={'out':str(out),'outcome':outcome,'verified':verified,'processes':{n:r['status'] for n,r in processes.items()}};(out/'summary.json').write_text(json.dumps(summary,indent=2));return summary
if __name__=='__main__':
 import argparse
 p=argparse.ArgumentParser();p.add_argument('--source',required=True);p.add_argument('--harness-dir',required=True);p.add_argument('--out',required=True);p.add_argument('--suites',default='witness,ordinary,hidden');p.add_argument('--ignore-leaks',action='store_true');a=p.parse_args()
 print(json.dumps(evaluate(a.source,{s:Path(a.harness_dir)/(s+'.rs') for s in a.suites.split(',')},a.out,ignore_leaks=a.ignore_leaks)))
