"""Reference protocol model, not an OS sandbox or a Rust soundness prover.
All signing must be done by a trusted runner outside candidate control.
"""
from dataclasses import dataclass, asdict
from typing import Tuple
import hashlib, hmac, json, secrets

FIELDS = ('source', 'contract', 'harness', 'environment', 'configuration', 'scope')
REQUIRED = ('compile', 'miri_default', 'miri_tree', 'semantics')
STATES = ('PASS', 'FAIL', 'UNKNOWN', 'TIMEOUT', 'UNSUPPORTED', 'ERROR')

def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(',', ':')).encode()

def digest(value):
    return hashlib.sha256(value if isinstance(value, bytes) else canonical(value)).hexdigest()

@dataclass(frozen=True)
class Context:
    source: str
    contract: str
    harness: str
    environment: str
    configuration: str
    scope: str

@dataclass(frozen=True)
class Evidence:
    context: Context
    obligation: str
    state: str
    log_digest: str
    signature: str

class TrustedRunner:
    def __init__(self):
        self.__key = secrets.token_bytes(32)
        self.__logs = {}

    def issue(self, context, obligation, state, log_bytes):
        if obligation not in REQUIRED or state not in STATES:
            raise ValueError('unknown obligation/state')
        log_hash = digest(log_bytes)
        self.__logs[log_hash] = log_bytes
        payload = dict(context=asdict(context), obligation=obligation,
                       state=state, log_digest=log_hash)
        sig = hmac.new(self.__key, canonical(payload), hashlib.sha256).hexdigest()
        return Evidence(context, obligation, state, log_hash, sig)

    def authentic(self, evidence):
        data = asdict(evidence)
        signature = data.pop('signature')
        actual = hmac.new(self.__key, canonical(data), hashlib.sha256).hexdigest()
        log = self.__logs.get(evidence.log_digest)
        return hmac.compare_digest(signature, actual) and log is not None and digest(log) == evidence.log_digest

class Arbiter:
    def __init__(self, runner, frozen_contract):
        self.runner = runner
        self.frozen_contract = frozen_contract

    def decide(self, current, evidence):
        if current.contract != self.frozen_contract:
            return 'REJECT_CONTRACT_CHANGE'
        if any(not self.runner.authentic(e) for e in evidence):
            return 'REJECT_UNAUTHENTIC'
        if any(e.context != current for e in evidence):
            return 'REJECT_STALE'
        if any(e.state == 'FAIL' for e in evidence):
            return 'REJECT_COUNTEREXAMPLE'
        if any(e.state != 'PASS' for e in evidence):
            return 'ABSTAIN_INCONCLUSIVE'
        for required in REQUIRED:
            matching = [e for e in evidence if e.obligation == required]
            if len(matching) != 1:
                return 'ABSTAIN_INCOMPLETE'
        return 'ACCEPT_OBSERVED_SCOPE'
