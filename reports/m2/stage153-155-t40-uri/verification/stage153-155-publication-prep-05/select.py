"""Select one finite current publication roster, preserving complete nonpasses.

No walks, subprocesses, archive bodies, toolchains or cache inspection. Root must
supply the reviewed closed plan and actual completed source audit identities.
"""
import argparse,json
from pathlib import Path
from common import *
OUT=ROOT/'target/m2/stage153-155-publication-selection-02'
rows={};omitted=[];documents={};checked={};excluded_roots=[]
FAILED_REGISTRY_DIGEST_DOCUMENT={'path': '/Users/cody.ij.hwang/Projects/pkl-rs.stage57/target/m2/stage153-155-t40-original-01/selected-regression-registry.json', 'bytes': 10494, 'sha256': '83511fa5fad7a85873f648be541596faaee6858297b4124d2c84aef08ace2bae', 'mode': 420}
FAILED_RESULT_DIGEST_DOCUMENTS={'/Users/cody.ij.hwang/Projects/pkl-rs.stage57/target/m2/stage153-155-t40-original-01/cases/51bbcfe4ca6096dcb961bd17/java/result.json': {'document': {'path': '/Users/cody.ij.hwang/Projects/pkl-rs.stage57/target/m2/stage153-155-t40-original-01/cases/51bbcfe4ca6096dcb961bd17/java/result.json', 'bytes': 294, 'sha256': '2fbdc69fc8e7c78bbcbf8159486d709e43f4827a6ee3e8c34bde7be11bf2dfa2', 'mode': 420}, 'stdout': {'path': '/Users/cody.ij.hwang/Projects/pkl-rs.stage57/target/m2/stage153-155-t40-original-01/cases/51bbcfe4ca6096dcb961bd17/java/stdout.bin', 'bytes': 17, 'sha256': '4d4d0946a537296a4ac9e2b9a1705e3cb4fa597fbcb4ba14b57b9275a71851ec', 'mode': 420}, 'stderr': {'path': '/Users/cody.ij.hwang/Projects/pkl-rs.stage57/target/m2/stage153-155-t40-original-01/cases/51bbcfe4ca6096dcb961bd17/java/stderr.bin', 'bytes': 0, 'sha256': 'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855', 'mode': 420}}, '/Users/cody.ij.hwang/Projects/pkl-rs.stage57/target/m2/stage153-155-t40-original-01/cases/51bbcfe4ca6096dcb961bd17/rust/result.json': {'document': {'path': '/Users/cody.ij.hwang/Projects/pkl-rs.stage57/target/m2/stage153-155-t40-original-01/cases/51bbcfe4ca6096dcb961bd17/rust/result.json', 'bytes': 294, 'sha256': '4dd96272576adef5f719f194d274bd5a1ae7c63477d9a984ee73af853bb29c82', 'mode': 420}, 'stdout': {'path': '/Users/cody.ij.hwang/Projects/pkl-rs.stage57/target/m2/stage153-155-t40-original-01/cases/51bbcfe4ca6096dcb961bd17/rust/stdout.bin', 'bytes': 17, 'sha256': '4d4d0946a537296a4ac9e2b9a1705e3cb4fa597fbcb4ba14b57b9275a71851ec', 'mode': 420}, 'stderr': {'path': '/Users/cody.ij.hwang/Projects/pkl-rs.stage57/target/m2/stage153-155-t40-original-01/cases/51bbcfe4ca6096dcb961bd17/rust/stderr.bin', 'bytes': 0, 'sha256': 'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855', 'mode': 420}}, '/Users/cody.ij.hwang/Projects/pkl-rs.stage57/target/m2/stage153-155-t40-original-01/cases/b01be896fe1b43f5c262dde8/java/result.json': {'document': {'path': '/Users/cody.ij.hwang/Projects/pkl-rs.stage57/target/m2/stage153-155-t40-original-01/cases/b01be896fe1b43f5c262dde8/java/result.json', 'bytes': 294, 'sha256': '591b0678b483f657920420a8cb38f4b6b4486991cb8682f93d34b1b13283de0b', 'mode': 420}, 'stdout': {'path': '/Users/cody.ij.hwang/Projects/pkl-rs.stage57/target/m2/stage153-155-t40-original-01/cases/b01be896fe1b43f5c262dde8/java/stdout.bin', 'bytes': 0, 'sha256': 'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855', 'mode': 420}, 'stderr': {'path': '/Users/cody.ij.hwang/Projects/pkl-rs.stage57/target/m2/stage153-155-t40-original-01/cases/b01be896fe1b43f5c262dde8/java/stderr.bin', 'bytes': 1824, 'sha256': '9b8edd057c55eaa3b49407c32ab111430acf2e90dbbbfc78fff85372e8b72311', 'mode': 420}}, '/Users/cody.ij.hwang/Projects/pkl-rs.stage57/target/m2/stage153-155-t40-original-01/cases/b01be896fe1b43f5c262dde8/rust/result.json': {'document': {'path': '/Users/cody.ij.hwang/Projects/pkl-rs.stage57/target/m2/stage153-155-t40-original-01/cases/b01be896fe1b43f5c262dde8/rust/result.json', 'bytes': 293, 'sha256': '58f9d72c4060514faaf1b7ed1768428c01b7ecfccd9c09c1a9953a877f6e0f30', 'mode': 420}, 'stdout': {'path': '/Users/cody.ij.hwang/Projects/pkl-rs.stage57/target/m2/stage153-155-t40-original-01/cases/b01be896fe1b43f5c262dde8/rust/stdout.bin', 'bytes': 0, 'sha256': 'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855', 'mode': 420}, 'stderr': {'path': '/Users/cody.ij.hwang/Projects/pkl-rs.stage57/target/m2/stage153-155-t40-original-01/cases/b01be896fe1b43f5c262dde8/rust/stderr.bin', 'bytes': 1824, 'sha256': '9b8edd057c55eaa3b49407c32ab111430acf2e90dbbbfc78fff85372e8b72311', 'mode': 420}}}
def add(expected,group,kind='raw'):
    assert isinstance(expected,dict) and {'path','sha256'}<=expected.keys()
    payload_path(expected['path'],kind,excluded_roots)
    assert isinstance(expected['sha256'],str) and re.fullmatch('[0-9a-f]{64}',expected['sha256'])
    if 'bytes' in expected:assert type(expected['bytes']) is int and expected['bytes']>=0
    if 'mode' in expected:assert type(expected['mode']) is int and 0<=expected['mode']<=0o7777
    if expected['path'] not in checked:checked[expected['path']]=pin(expected['path'])[0]
    actual=checked[expected['path']]
    assert {'path','sha256'}<=expected.keys()
    assert all(actual[k]==expected[k] for k in actual if k in expected)
    name=str(Path(actual['path']).relative_to(ROOT))
    if name in rows:
        assert rows[name]['source']==actual
        if group not in rows[name]['groups']:rows[name]['groups'].append(group)
    else:rows[name]=dict(member=name,source=actual,groups=[group])
    return actual

def channel_projection(document_path,trail,item):
    # Exact existing document/field schemas only. A recognized process channel
    # anywhere else must satisfy the ordinary process-pin contract.
    if document_path==FAILED_REGISTRY_DIGEST_DOCUMENT['path']and len(trail)==6 and trail[0]=='cases'and type(trail[1])is int and trail[1]in (0,1)and trail[2:5]==('contract','oracle_observation','sha256')and trail[5]in ('stdout','stderr'):
        assert isinstance(item,str)and re.fullmatch('[0-9a-f]{64}',item)
        return []  # Historical contract digest metadata, not a current process pin.
    if document_path in FAILED_RESULT_DIGEST_DOCUMENTS and len(trail)==2 and trail[0]=='sha256'and trail[1]in ('stdout','stderr'):
        assert isinstance(item,str)and re.fullmatch('[0-9a-f]{64}',item)
        stream=process_channel_pin(FAILED_RESULT_DIGEST_DOCUMENTS[document_path][trail[1]])
        assert item==stream['sha256'],'Failed result digest differs from exact selected sibling stream.'
        return [stream]
    if document_path==str(ROOT/'compatibility/regressions.json') and len(trail)==5 and trail[0]=='cases' and type(trail[1])is int and trail[2:4]==('oracle_observation','sha256'):
        assert trail[-1] in ('stdout','stderr') and isinstance(item,str) and re.fullmatch('[0-9a-f]{64}',item)
        return []  # Exact digest-only inventory metadata, retained verbatim.
    semantic=str(ROOT/'target/m2/stage145-protobuf-semantic-review-03')
    if document_path==semantic+'/report.json' and len(trail)==5 and trail[0]=='sorting_controls' and type(trail[1])is int and trail[2]=='modes' and trail[3] in ('on','off') and trail[4]=='stdout':
        assert isinstance(item,dict) and set(item)=={'text','utf16_be_hex','utf8','utf16'}
        assert isinstance(item['text'],str) and isinstance(item['utf16_be_hex'],str) and re.fullmatch('(?:[0-9a-f]{4})*',item['utf16_be_hex'])
        return [process_channel_pin(item['utf8']),process_channel_pin(item['utf16'])]
    if document_path==semantic+'/extracted.json' and len(trail)==6 and trail[0]=='cases' and type(trail[1])is int and trail[2]=='modes' and trail[3]in ('on','off') and trail[4]=='process_full_utf8' and trail[5]in ('stdout','stderr'):
        assert isinstance(item,str)
        return []  # Exact derived text projection, not a file-body request.
    return [process_channel_pin(item)]

def raw_channels(value,group,document_path='',trail=()):
    if isinstance(value,dict):
        for key,item in value.items():
            current=trail+(key,)
            if key in ('stdout','stderr'):
                for row in channel_projection(document_path,current,item):add(row,group+'-process-raw')
            else:raw_channels(item,group,document_path,current)
    elif isinstance(value,list):
        for index,item in enumerate(value):raw_channels(item,group,document_path,trail+(index,))

def record(expected,group):
    canonical(expected);actual=add(expected,group,'authority')
    if actual['path'] not in documents:documents[actual['path']]=read_pin(actual)
    if actual['path']==FAILED_REGISTRY_DIGEST_DOCUMENT['path']:assert actual==FAILED_REGISTRY_DIGEST_DOCUMENT
    if actual['path']in FAILED_RESULT_DIGEST_DOCUMENTS:assert actual==FAILED_RESULT_DIGEST_DOCUMENTS[actual['path']]['document']
    doc=documents[actual['path']];raw_channels(doc,group,actual['path'])
    # Current flat guards retain full inline before/after arrays in this body.
    # Retain any explicitly encoded stored snapshot too, without assuming gzip.
    if isinstance(doc,dict):
        for key in ('inputs_before','inputs_after'):
            value=doc.get(key)
            if isinstance(value,dict) and value.get('encoding')=='gzip':add(value['stored'],group+'-source-bookends')
    return doc

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--plan-pin',nargs=4,required=True)
    parser.add_argument('--audit-pin',nargs=4,required=True)
    args=parser.parse_args()
    def supplied(parts):
        path,size,sha,mode=parts;return canonical(dict(path=path,bytes=int(size),sha256=sha,mode=int(mode)))
    plan_pin=supplied(args.plan_pin);plan=read_pin(plan_pin);validate_plan(plan)
    excluded_roots.extend(plan['excluded_body_roots'])
    source_plan=record(plan['source_plan'],'verified-source-plan');coverage=source_coverage(plan,source_plan)
    audit_pin=supplied(args.audit_pin);audit=record(audit_pin,'final-source-audit')
    assert audit['complete'] is True and audit['errors']==[] and audit['plan']==plan_pin
    assert audit['verified_source_plan']==plan['source_plan'] and audit['source_plan_coverage']==coverage
    assert audit['external_source_inputs']==plan['external_source_inputs'] and audit['external_sourceplan_bodies_selected']is False
    assert audit['identity_only_source_inputs']==plan['identity_only_source_inputs']
    assert audit['source_plan_input_count']==source_plan['file_count']==len(coverage)+len(plan['identity_only_source_inputs'])
    assert audit['documentation_transitions']==plan['documentation_transitions']and audit['source_epoch_transition']==plan['source_epoch_transition']
    source_paths={r['current']['path']for r in plan['source_rows']}
    assert audit['source_plan_payloads']==[r['current']for r in coverage if r['current']['path']not in source_paths]
    assert audit['source_epoch']==HEAD and audit['source_dependency_closure_established'] is False
    assert audit['documentation_manifest']==plan['docs_manifest']
    assert audit['source_file_count']==len(audit['files'])==len(plan['source_rows'])
    assert audit['files']==[dict(destination=r['destination'],actual=r['current']) for r in plan['source_rows']]
    assert audit['documentation']==[dict(role=k,actual=v) for k,v in plan['docs'].items()]
    assert audit['explicit_absences']==[dict(path=p,absent=True) for p in plan['explicit_absences']]
    for row in plan['identity_only_source_inputs']:
        authority=record(row['acquisition_authority'],'identity-only-acquisition-authority')
        configured=record(authority['config'],'identity-only-acquisition-config');identity_acquisition(row,authority,configured)
        omitted.append(dict(row['pin'],category=row['category'],reason=row['reason'],body_selected=False,acquisition_authority=row['acquisition_authority']))
    role_docs={};role_pins={}
    for role in plan['roles']:
        doc=record(role['actual_pin'],role['role']);json_assertions(doc,role['assertions'])
        role_docs[role['role']]=doc;role_pins[role['role']]=role['actual_pin']
    epoch_verification=verify_source_epoch(plan,source_plan,lambda value:record(value,'source07-transition-authority'))
    assert audit['source_epoch_verification']==epoch_verification
    for change in (plan['source_epoch_transition']or {}).get('changes',[]):
        add(change['before_alias'],'source06-before-alias','source');add(change['current'],'source07-current','source')
    if plan['source_epoch_transition']is not None:
        failed=record(FAILED_T40_SELECTION,'preserved-failures')
        assert failed['file_count']==len(failed['files'])==37 and all(r['qualified']is False for r in failed['observations'])
        for value in input_catalogue(failed['files']):
            if Path(value['path']).suffix=='.json':record(value,'preserved-failures')
            else:add(value,'preserved-failures','source'if value in FAILED_T40_LICENSE_PINS else'raw')
    # Exact process membership is checked against each current API capture.
    for name,count in (('file_uri48_boundary_raw_and_independent_readback',24),('package_uri74_boundary_raw_and_independent_readback',37)):
        api=role_docs[name];capture=record(api['capture_guard'],name+'-canonical-stream-coverage')
        paired_channels(api,capture,count)
    for row in plan['records']:
        doc=record(row['pin'],row['group']);json_assertions(doc,row['assertions'])
    for row in plan['payloads']:add(row['pin'],row['group'],row['kind'])
    for row in plan['source_rows']:add(row['current'],'current-code-fixtures-and-tools','source')
    record(plan['docs_manifest'],'frozen-final-docs')
    for value in plan['docs'].values():add(value,'frozen-final-docs','source')
    record(plan_pin,'reviewed-closed-selection-plan')
    for transition in plan['documentation_transitions']:
        add(transition['before_alias'],'documentation-before-alias','source')
        add(transition['current'],'documentation-current','source')
        doc=record(transition['receipt'],'documentation-transition');json_assertions(doc,transition['assertions'])
    # The independently verified finite t40 catalogues retain originals and
    # separate Java cache controls without discovering any filesystem entries.
    seen_t40=False;seen_raw266=False;seen_m1=False
    for spec in plan['raw_maps']:
        root=lexical(spec['root']);group=spec['group'];doc=record(spec['report'],group)
        if spec['kind']=='t40_inputs':
            assert not seen_t40 and spec['report']==role_pins['t40_original2_raw_and_separate_cache_controls'];seen_t40=True
            cache=record(spec['cache_report'],group+'-separate-cache-reader')
            original=record(doc['original_report'],group+'-canonical-original-report')
            controls=record(cache['control_report'],group+'-cache-control-report')
            requested,exclusions=t40_catalogue(doc,cache,original,controls,spec)
            for value,kind in requested:add(value,group+'-raw-and-source',kind)
            omitted.extend(exclusions)
        elif spec['kind']=='raw266':
            assert not seen_raw266 and spec['report']==role_pins['same_workspace_raw266_channels_and_reader'];seen_raw266=True
            assert len(doc['observations'])==266
            for value in doc['raw_ledgers']:add(value,group+'-raw-ledgers')
            for observation in doc['observations']:
                assert observation['actual_channels'], 'Each attempted slot needs unchanged raw channels.'
                for value in observation['actual_channels']:add(value,group+'-actual-channels')
        else:
            assert not seen_m1 and spec['report']==role_pins['m1_clone_capture_raw_channels_and_reader'];seen_m1=True
            exclusions=spec['compiled_products']
            assert len(exclusions)==3 and len(set(exclusions))==3
            assert all(Path(p).is_relative_to(root) for p in exclusions)
            captured_products=set()
            for catalogue_row in input_catalogue(doc['inputs']):
                filename=catalogue_row['path'];identity={k:v for k,v in catalogue_row.items()if k!='path'}
                # Only this explicit capture-root catalogue is selected. Other
                # runtime/browser/JDK/cache inputs remain recorded dependencies.
                p=Path(filename)
                if not p.is_relative_to(root):continue
                assert p.suffix not in BINARY or filename in exclusions, 'Unexpected compiled capture input needs explicit review.'
                reason='compiled-M1-product' if filename in exclusions else None
                if reason:
                    assert '..' not in p.parts and not any('stage86' in s.lower() or 'private8' in s.lower() or s=='stage65-b-native-regressions-evidence.tar.gz.partial' for s in p.parts)
                    captured_products.add(filename)
                    omitted.append(dict(path=filename,**identity,reason=reason));continue
                add(dict(path=filename,**identity),group+'-raw-and-source',m1_payload_kind(filename,spec))
            assert captured_products==set(exclusions), 'All three omitted M1 product identities must be recorded.'
    assert seen_t40 and seen_raw266 and seen_m1
    # Re-read every selected pin; source/audit authorities must still agree.
    for value in checked.values():expected_pin(value)
    for value in [plan_pin,audit_pin]:expected_pin(value)
    groups={g for row in rows.values() for g in row['groups']}
    assert {'preserved-failures','reproduction-source','applied-source-authorities','historical-java-authorities'}<=groups
    required=[dict(role=r['role'],required=True,actual_pin=r['actual_pin']) for r in plan['roles']]
    required.append(dict(role='final_docs_and_source_audit',required=True,actual_pin=audit_pin))
    assert {r['role'] for r in required}==ROLE_NAMES
    files=sorted(rows.values(),key=lambda row:row['member'])
    roster=dict(complete=True,draft=False,mutable_scratch_pending_m1=False,head=HEAD,
      files=files,file_count=len(files),logical_payload_bytes=sum(r['source']['bytes'] for r in files),
      required_future_inputs=required,source_audit=audit_pin,closed_plan=plan_pin,
      source_dependency_closure_established=False,full_m2_claim=False,
      historical_dependencies=plan['historical_dependencies'],omissions=omitted,
      documentation_transitions=plan['documentation_transitions'],source_epoch_transition=plan['source_epoch_transition'],source_epoch_verification=epoch_verification,verified_source_plan=plan['source_plan'],source_plan_input_count=source_plan['file_count'],external_source_inputs=plan['external_source_inputs'],external_sourceplan_bodies_selected=False,identity_only_source_inputs=plan['identity_only_source_inputs'],source_plan_coverage=coverage,
      outcomes={r['role']:dict(actual_pin=r['actual_pin'],asserted_fields=r['assertions'])for r in plan['roles']},
      scope='Complete finite evidence selection, not compatibility success or hermetic restoration. All nonpasses retained; final archive integrity/publication/Git are separate after-pack receipts.')
    chain(OUT.parent);OUT.mkdir(exist_ok=False)
    (OUT/'payload-roster.json').write_text(json.dumps(roster,indent=2)+'\n')
    (OUT/'omissions.json').write_text(json.dumps(dict(omissions=omitted,historical_dependencies=plan['historical_dependencies']),indent=2)+'\n')
    print(json.dumps(pin(OUT/'payload-roster.json')[0]))
if __name__=='__main__':main()
