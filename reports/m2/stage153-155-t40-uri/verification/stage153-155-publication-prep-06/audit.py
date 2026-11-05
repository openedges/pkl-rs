"""Audit explicit current source/document pins only after every workload closes."""
import argparse,json
from pathlib import Path
from common import *
OUT=ROOT/'target/m2/stage153-155-source-verification-02'
def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--plan-pin',nargs=4,required=True)
    args=parser.parse_args();path,size,sha,mode=args.plan_pin
    supplied=dict(path=path,bytes=int(size),sha256=sha,mode=int(mode))
    plan=read_pin(supplied);validate_plan(plan)
    # Check nine actual receipts before current body reads; assertions do not
    # require compatibility passes and may explicitly preserve failed outcomes.
    source_plan=read_pin(plan['source_plan']);coverage=source_coverage(plan,source_plan)
    roles=[]
    for row in plan['roles']:
        doc=read_pin(row['actual_pin']);json_assertions(doc,row['assertions'])
        roles.append(dict(role=row['role'],actual_pin=row['actual_pin']))
    epoch_pins={}
    def epoch_load(value):
        canonical(value)
        if value['path']not in epoch_pins:epoch_pins[value['path']]=(value,read_pin(value))
        assert epoch_pins[value['path']][0]==value
        return epoch_pins[value['path']][1]
    epoch_verification=verify_source_epoch(plan,source_plan,epoch_load)
    for change in (plan['source_epoch_transition']or {}).get('changes',[]):expected_pin(change['before_alias']);expected_pin(change['current'])
    identity_authorities={};identity_configs={}
    for row in plan['identity_only_source_inputs']:
        key=row['acquisition_authority']['path']
        if key not in identity_authorities:identity_authorities[key]=read_pin(row['acquisition_authority'])
        authority=identity_authorities[key];config_pin=authority['config'];config_key=tuple(config_pin[k]for k in ('path','bytes','sha256','mode'))
        if config_key not in identity_configs:identity_configs[config_key]=read_pin(config_pin)
        identity_acquisition(row,authority,identity_configs[config_key])
    transitions=[]
    for transition in plan['documentation_transitions']:
        expected_pin(transition['before_alias']);expected_pin(transition['current'])
        receipt=read_pin(transition['receipt']);json_assertions(receipt,transition['assertions'])
        transitions.append(transition)
    source_paths={r['current']['path']for r in plan['source_rows']}
    covered_payloads=[r['current']for r in coverage if r['current']['path']not in source_paths]
    for row in covered_payloads:expected_pin(row)
    checked=[]
    for row in plan['source_rows']:
        actual=expected_pin(row['current'])
        checked.append(dict(destination=row['destination'],actual=actual))
    docs=read_pin(plan['docs_manifest']);assert docs['head']==HEAD and docs['frozen'] is True
    declared={r['path']:r for r in docs['files']}
    documentation=[]
    for role,expected in plan['docs'].items():
        assert expected['path'] in declared
        assert all(declared[expected['path']][k]==expected[k] for k in expected)
        documentation.append(dict(role=role,actual=expected_pin(expected)))
    assert len({x['actual']['path'] for x in documentation})==2
    absence=[]
    for value in plan['explicit_absences']:
        p=lexical(value);chain(p.parent)
        try:p.lstat()
        except FileNotFoundError:absence.append(dict(path=str(p),absent=True))
        else:raise AssertionError('Expected absent destination exists: '+str(p))
    # Recheck finite source/document/receipt bookends; not an atomic tree snapshot.
    for row in checked:expected_pin(row['actual'])
    for row in covered_payloads:expected_pin(row)
    for value,_ in epoch_pins.values():expected_pin(value)
    for change in (plan['source_epoch_transition']or {}).get('changes',[]):expected_pin(change['before_alias']);expected_pin(change['current'])
    for item in [supplied,plan['docs_manifest'],*plan['docs'].values(),*[r['actual_pin'] for r in roles],plan['source_plan'],*[p for t in transitions for p in (t['before_alias'],t['current'],t['receipt'])],*[r['acquisition_authority']for r in plan['identity_only_source_inputs']],*[a['config']for a in identity_authorities.values()],*GUARD_HELPER_PINS]:expected_pin(item)
    for value in plan['explicit_absences']:
        p=lexical(value);chain(p.parent)
        try:p.lstat()
        except FileNotFoundError:pass
        else:raise AssertionError('Expected absent destination reappeared: '+str(p))
    report=dict(complete=True,errors=[],source_epoch=HEAD,actual_git_head_checked=False,
      preparation=pin(Path(__file__).absolute())[0],plan=supplied,
      verified_source_plan=plan['source_plan'],source_epoch_transition=plan['source_epoch_transition'],source_epoch_verification=epoch_verification,source_plan_input_count=source_plan['file_count'],external_source_inputs=plan['external_source_inputs'],external_sourceplan_bodies_selected=False,identity_only_source_inputs=plan['identity_only_source_inputs'],source_plan_coverage=coverage,source_plan_payloads=covered_payloads,documentation_transitions=transitions,source_file_count=len(checked),files=checked,documentation_manifest=plan['docs_manifest'],
      documentation=documentation,explicit_absences=absence,closed_role_authorities=roles,
      source_dependency_closure_established=False,production_mutated=False,
      limits=['Finite supplied source/document roster only; no whole-tree or Git-membership audit.',
              'Sequential stable-file reads and pre/post absence observations are not continuous enforcement.',
              'Receipt identity and explicit assertions do not replace independent raw readers or semantic review.',
              'No compatibility, platform equivalence, performance or full M2 promotion.'])
    chain(OUT.parent);OUT.mkdir(exist_ok=False)
    (OUT/'source-verification.json').write_text(json.dumps(report,indent=2)+'\n')
    print(json.dumps(pin(OUT/'source-verification.json')[0]))
if __name__=='__main__':main()
