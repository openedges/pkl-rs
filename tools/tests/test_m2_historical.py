"""Historical admission preserves identity/accounting and full raw filesystem channels."""
import copy
import json
from pathlib import Path
import shutil
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0,str(Path(__file__).resolve().parents[1]))
import compat
import m2_historical as historical
import m2_language
import m2_manifest
import m2_module_eval


class HistoricalRegistryTests(unittest.TestCase):
    def setUp(self):
        self.root=m2_manifest.ROOT
        self.registry=historical.build_registry(self.root)

    def test_exact_eight_ids_contracts_and_ninth_resource_only(self):
        rows=historical.registry_cases(self.root,self.registry)
        self.assertEqual(len(rows),8)
        self.assertTrue(all(k.startswith('rev1-candidate:') for k in rows))
        self.assertTrue(any(x['path'].endswith('nestedLib.pkl') for x in self.registry['resources']))
        self.assertFalse(any(k.endswith('nestedLib.pkl') for k in rows))
        self.assertEqual({Path(x['path']).name for x in self.registry['resources'] if x['original']['path'] in ('LICENSE','NOTICE')},{'LICENSE','NOTICE'})
        self.assertTrue(all(x['contract']['channels']==['stdout','stderr','exit_code','files','workspace_changes'] for x in rows.values()))

    def test_cli_registry_contains_twenty_one_cases_and_two_support_libraries(self):
        cli_reg = historical.build_cli_registry(self.root)
        self.assertEqual(len(cli_reg['cases']), 21)
        self.assertEqual(len(historical.CLI_SELECTED), 21)
        self.assertTrue(all(k.startswith('rev1-candidate:') for k in [c['id'] for c in cli_reg['cases']]))
        self.assertEqual(len(cli_reg['resources']), 25)
        self.assertTrue(any(x['path'].endswith('t43_043_inherited_method_lib.pkl') for x in cli_reg['resources']))
        self.assertTrue(any(x['path'].endswith('t43_064_imported_function_lib.pkl') for x in cli_reg['resources']))
        self.assertFalse(any(c['id'].endswith('_lib.pkl') for c in cli_reg['cases']))
        self.assertEqual({Path(x['path']).name for x in cli_reg['resources'] if x['original']['path'] in ('LICENSE', 'NOTICE')}, {'LICENSE', 'NOTICE'})
        self.assertTrue(all(c['contract']['channels'] == ['stdout', 'stderr', 'exit_code', 'files', 'workspace_changes'] for c in cli_reg['cases']))

    def test_cli_isolated_t43_cases_stage_distinct_sibling_libs_without_changing_other_nineteen(self):
        cli_reg = historical.build_cli_registry(self.root)
        by_id = {c['id']: c for c in cli_reg['cases']}
        res_sha = {r['path']: r['sha256'] for r in cli_reg['resources']}
        self.assertEqual(len(cli_reg['staged_resources']), 4)
        expected_pairs = {
            'rev1-candidate:crates/pkl-cli/tests/fixtures/t43_043_inherited_method_main.pkl': (
                'staged/t43_043_inherited_method/t43_043_inherited_method_main.pkl',
                'staged/t43_043_inherited_method/lib.pkl',
                'tests/fixtures/rev1-cli/t43_043_inherited_method_main.pkl',
                'tests/fixtures/rev1-cli/t43_043_inherited_method_lib.pkl',
            ),
            'rev1-candidate:crates/pkl-cli/tests/fixtures/t43_064_imported_function_main.pkl': (
                'staged/t43_064_imported_function/t43_064_imported_function_main.pkl',
                'staged/t43_064_imported_function/lib.pkl',
                'tests/fixtures/rev1-cli/t43_064_imported_function_main.pkl',
                'tests/fixtures/rev1-cli/t43_064_imported_function_lib.pkl',
            ),
        }
        for case_id, entry in by_id.items():
            contract = entry['contract']
            if case_id in expected_pairs:
                staged_main, staged_lib, orig_main, orig_lib = expected_pairs[case_id]
                self.assertEqual(entry['input'], orig_main)
                self.assertEqual(contract['source']['path'], orig_main)
                self.assertEqual(contract['staged_input'], staged_main)
                self.assertEqual(contract['command'][-1], staged_main)
                case_res = {r['path']: r['sha256'] for r in contract['resources']}
                self.assertEqual(case_res[staged_main], res_sha[orig_main])
                self.assertEqual(case_res[staged_lib], res_sha[orig_lib])
            else:
                self.assertNotIn('staged_input', contract)
                self.assertEqual(contract['command'][-1], entry['input'])
                self.assertEqual(len(contract['resources']), 25)

    def test_resource_only_map_and_tracked_regressions_census(self):
        self.assertEqual(len(historical.RESOURCE_ONLY_MAP), 8)
        self.assertEqual(len(historical.ALL_RESOURCE_ONLY), 8)
        self.assertEqual(len(historical.TRACKED_REGRESSIONS), 3)
        self.assertEqual(len(historical.SELECTED) + len(historical.CLI_SELECTED) + len(historical.RESOURCE_ONLY_MAP) + len(historical.TRACKED_REGRESSIONS), 40)

    def test_missing_duplicate_extra_or_contract_tamper_rejected(self):
        for change in ('missing','duplicate','extra','argv','env','cwd','resources','output','source','watchdog'):
            registry=copy.deepcopy(self.registry)
            if change=='missing':registry['cases'].pop()
            elif change=='duplicate':registry['cases'].append(registry['cases'][0])
            elif change=='extra':registry['cases'][0]['id']='rev1-candidate:'+historical.ORIGINAL+historical.RESOURCE_ONLY
            elif change=='argv':registry['cases'][0]['contract']['command'].insert(1,'--color=never')
            elif change=='env':registry['cases'][0]['contract']['environment']['TZ']='Europe/Paris'
            elif change=='cwd':registry['cases'][0]['contract']['cwd']='other'
            elif change=='resources':registry['cases'][0]['contract']['resources']=[]
            elif change=='output':registry['cases'][0]['contract']['output']={'kind':'multiple','path':'{output_dir}'}
            elif change=='source':registry['cases'][0]['input_sha256']='0'*64
            else:registry['watchdog_seconds']=1
            with self.subTest(change=change),self.assertRaises(ValueError):historical.registry_cases(self.root,registry)

    def test_missing_changed_extra_or_symlink_fixture_rejected(self):
        temp=tempfile.TemporaryDirectory();self.addCleanup(temp.cleanup)
        root=Path(temp.name);(root/'compatibility').mkdir()
        shutil.copyfile(self.root/'compatibility/rev1-inventory.json',root/'compatibility/rev1-inventory.json')
        shutil.copytree(self.root/historical.FIXTURES,root/historical.FIXTURES)
        path=root/historical.FIXTURES/historical.SELECTED[0];raw=path.read_bytes()
        for change in ('missing','changed','extra','symlink'):
            if change=='missing':path.unlink()
            elif change=='changed':path.write_bytes(raw+b'changed')
            elif change=='extra':(path.parent/'extra.pkl').write_bytes(b'')
            else:path.unlink();path.symlink_to(self.root/historical.FIXTURES/historical.SELECTED[0])
            with self.subTest(change=change),self.assertRaises((OSError,ValueError)):historical.build_registry(root)
            if path.is_symlink():path.unlink()
            path.write_bytes(raw)
            (path.parent/'extra.pkl').unlink(missing_ok=True)

    def test_total_ids_and_other_original_contracts_unchanged(self):
        manifest=m2_manifest.current_manifest()
        originals=json.loads((self.root/'compatibility/regressions.json').read_bytes())['cases']
        self.assertEqual(len(manifest['cases']),4586)
        token_ids={f'token-stage97-{token}-{boundary}'
            for token in ('true','false','null')
            for boundary in ('identifier-classes','delimiter-classes','literal-at-EOF',
                             'comment-newline-success','supplementary-identifier-boundary',
                             'reserved-bare-declaration')}
        tokens=[row for row in manifest['cases'] if row['id'].startswith('token-stage97-')]
        self.assertEqual(len(tokens),18)
        self.assertEqual({row['id'] for row in tokens},token_ids)
        for row in tokens:
            self.assertEqual(row['adapter'],'language-api-string')
            self.assertEqual(row['execution']['state'],'ready')
            self.assertTrue(row['required'])
            self.assertNotIn('upstream_case',row)
        candidates=[x for x in manifest['cases'] if x['id'].startswith('rev1-candidate:')]
        self.assertEqual(len(candidates),40)
        self.assertEqual(sum(x['execution']['state']=='ready' for x in candidates),29)
        self.assertEqual(sum(x['execution']['state']=='unmapped' for x in candidates),11)
        self.assertEqual(manifest['counts']['cases_by_execution'],{'ready':4575,'unmapped':11})
        admitted={'rev1:t36-foreign-nominal-identity','rev1:t40-imported-declaration-authority','rev1:t40-foreign-namesake'}
        for original in originals:
            case=next(x for x in manifest['cases'] if x['id']=='m2:'+original['id'])
            self.assertEqual(case['regression_case'],original)
            self.assertEqual(case['execution']['state'],'ready' if original['id'] in admitted else 'unimplemented')
            self.assertEqual(case['adapter'],'selected-regression-module-eval' if original['id'] in admitted else None)
            self.assertIn('--color=never',case['regression_case']['command'])
        selected=m2_language.selected_cases(manifest,[historical.ADAPTER])
        self.assertEqual([x['id'] for x in selected],[x['id'] for x in self.registry['cases']])
        cli_selected=m2_language.selected_cases(manifest,[historical.CLI_ADAPTER])
        cli_registry=historical.cli_registry_cases(self.root)
        self.assertEqual(len(cli_selected),21)
        self.assertEqual([x['id'] for x in cli_selected],list(cli_registry.keys()))
        self.assertEqual(len(m2_language.selected_cases(manifest)),4575)
        self.assertTrue(any(selected[0]['id'] in error for error in m2_manifest.full_gate_failures(manifest,[])))

    def test_unselected_promotion_and_missing_id_rejected(self):
        manifest=m2_manifest.current_manifest()
        for change in ('promote','drop'):
            broken=copy.deepcopy(manifest)
            case=next(x for x in broken['cases'] if x['id'].startswith('rev1-candidate:') and x['execution']['state']=='unmapped')
            if change=='promote':case['adapter']=historical.ADAPTER;case['execution']={'state':'ready','reason':None}
            else:broken['cases'].remove(case)
            with self.subTest(change=change),self.assertRaises(ValueError):m2_manifest.validate_manifest(broken)


class RawCaptureTests(unittest.TestCase):
    def setUp(self):
        temp=tempfile.TemporaryDirectory();self.addCleanup(temp.cleanup)
        self.root=Path(temp.name);self.workspace=self.root/'target/workspace';self.workspace.mkdir(parents=True)
        (self.root/'input.pkl').write_bytes(b'v=1\n');(self.workspace/'input.pkl').write_bytes(b'v=1\n')
        self.case={'id':'synthetic-historical','adapter':historical.ADAPTER,'input':'input.pkl',
            'input_sha256':compat.digest(b'v=1\n'),'upstream_case':{'id':'synthetic-historical','command':[],'cwd':'.','environment':{},'resources':[],'expected_exit':[0]}}

    def capture(self,script,name):
        return m2_module_eval.capture_process(self.root,self.workspace,self.case,[sys.executable,'-c',script],
            {'PATH':'/bin:/usr/bin','LANG':'C','LC_ALL':'C','TZ':'UTC','TERM':'dumb','NO_COLOR':'1'},self.root/'target'/name)

    def test_full_stdout_stderr_exit_generated_files_and_tamper(self):
        directory=compat.case_directory(self.case['id'])
        script="import pathlib,sys;sys.stdout.buffer.write(b'raw\\x00\\xff');sys.stderr.buffer.write(b'err\\n');pathlib.Path('generated/"+directory+"/file.bin').write_bytes(b'file\\x00\\xff')"
        process,captured=self.capture(script,'raw')
        self.assertEqual(captured['stdout'],b'raw\0\xff')
        self.assertEqual(captured['stderr'],b'err\n')
        self.assertEqual(captured['exit_code'],0)
        self.assertEqual(captured['files'],{'generated/file.bin':b'file\0\xff'})
        self.assertEqual(captured['workspace_changes'],{})
        self.assertEqual(compat.load_capture(self.root/'target/raw')['files'],captured['files'])
        (self.root/'target/raw/files/generated/file.bin').write_bytes(b'changed')
        with self.assertRaises(ValueError):compat.load_capture(self.root/'target/raw')

    def test_unexpected_write_retained_restored_and_never_pass(self):
        process,captured=self.capture("from pathlib import Path;Path('unexpected.bin').write_bytes(b'unexpected')",'unexpected')
        self.assertIn('unexpected.bin',captured['workspace_changes'])
        self.assertEqual(captured['files']['workspace/unexpected.bin'],b'unexpected')
        self.assertFalse((self.workspace/'unexpected.bin').exists())
        self.assertEqual(m2_module_eval.status(process,process,captured,captured,[0]),'oracle-error')


class CliIsolatedStagingTests(unittest.TestCase):
    def _synthetic_root(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        root = Path(temp.name).resolve(strict=True)
        (root / 'compatibility').mkdir()
        shutil.copyfile(m2_manifest.ROOT / 'compatibility/rev1-inventory.json', root / 'compatibility/rev1-inventory.json')
        shutil.copytree(m2_manifest.ROOT / historical.CLI_FIXTURES, root / historical.CLI_FIXTURES)
        return root

    def test_acquire_and_verify_cli_isolated_staging_and_tamper_rejection(self):
        root = self._synthetic_root()
        output = root / 'target/cli-evidence'
        output.mkdir(parents=True)
        workspace = root / 'target/cli-workspace'
        registry = historical.build_cli_registry(root)
        cases = []
        for entry in registry['cases']:
            cases.append({
                'id': entry['id'],
                'adapter': historical.CLI_ADAPTER,
                'input': entry['input'],
                'input_sha256': entry['input_sha256'],
                'upstream_case': entry['contract'],
                'execution': {'state': 'ready', 'reason': None},
            })
        probe = root / 'synthetic-cli-probe.py'
        probe.write_text(
            "#!/usr/bin/env python3\n"
            "import hashlib, pathlib, sys\n"
            "target = pathlib.Path(sys.argv[-1])\n"
            "sib = target.parent / 'lib.pkl'\n"
            "sib_sha = hashlib.sha256(sib.read_bytes()).hexdigest() if sib.is_file() else 'MISSING'\n"
            "sys.stdout.write(f'{target.as_posix()}|{sib_sha}\\n')\n",
            encoding='utf-8',
        )
        probe.chmod(0o755)
        metadata = {
            'command': [sys.executable, str(probe)],
            'environment': {'PATH': '/bin:/usr/bin', 'LANG': 'C', 'LC_ALL': 'C', 'TZ': 'UTC', 'TERM': 'dumb', 'NO_COLOR': '1'},
        }
        rows, context = historical.acquire_cli(root, output, cases, metadata, probe, workspace)
        self.assertEqual(len(rows), 21)
        self.assertTrue(all(r['status'] == 'pass' for r in rows))
        baseline = historical.verify_cli_context(root, output, context)
        by_id = {r['id']: r for r in rows}
        for case in cases:
            historical.verify_cli_case(root, output, context, case, by_id[case['id']], metadata, probe, baseline)

        # Verify both t43 cases actually observed their distinct sibling lib.pkl bytes.
        t43_043_id = 'rev1-candidate:crates/pkl-cli/tests/fixtures/t43_043_inherited_method_main.pkl'
        t43_064_id = 'rev1-candidate:crates/pkl-cli/tests/fixtures/t43_064_imported_function_main.pkl'
        sha_043_lib = compat.digest((root / historical.CLI_FIXTURES / 't43_043_inherited_method_lib.pkl').read_bytes())
        sha_064_lib = compat.digest((root / historical.CLI_FIXTURES / 't43_064_imported_function_lib.pkl').read_bytes())
        out_043 = (output / 'cases' / by_id[t43_043_id]['directory'] / 'rust/stdout.bin').read_text(encoding='utf-8').strip()
        out_064 = (output / 'cases' / by_id[t43_064_id]['directory'] / 'rust/stdout.bin').read_text(encoding='utf-8').strip()
        self.assertEqual(out_043, f'staged/t43_043_inherited_method/t43_043_inherited_method_main.pkl|{sha_043_lib}')
        self.assertEqual(out_064, f'staged/t43_064_imported_function/t43_064_imported_function_main.pkl|{sha_064_lib}')

        # Verify tamper rejection on staged_input, staged lib.pkl baseline hash, registry, and context.
        t43_043_case = next(c for c in cases if c['id'] == t43_043_id)
        bad_staged = copy.deepcopy(t43_043_case)
        bad_staged['upstream_case']['staged_input'] = 'staged/other/main.pkl'
        with self.assertRaisesRegex(ValueError, 'staged input disagrees'):
            historical.verify_cli_case(root, output, context, bad_staged, by_id[t43_043_id], metadata, probe, baseline)

        bad_baseline = copy.deepcopy(baseline)
        bad_baseline['staged/t43_043_inherited_method/lib.pkl']['sha256'] = '0' * 64
        with self.assertRaisesRegex(ValueError, 'resource baseline differs'):
            historical.verify_cli_case(root, output, context, t43_043_case, by_id[t43_043_id], metadata, probe, bad_baseline)

        bad_context = dict(context, baseline_sha256='0' * 64)
        with self.assertRaisesRegex(ValueError, 'context/profile changed'):
            historical.verify_cli_context(root, output, bad_context)

        bad_reg_path = output / 'historical-cli-registry.json'
        orig_reg_bytes = bad_reg_path.read_bytes()
        tampered_reg = json.loads(orig_reg_bytes)
        tampered_reg['staged_resources'][1]['sha256'] = '0' * 64
        bad_reg_path.write_bytes(compat.encoded(tampered_reg))
        with self.assertRaisesRegex(ValueError, 'CLI contract differs'):
            historical.verify_cli_context(root, output, context)
        bad_reg_path.write_bytes(orig_reg_bytes)

    def test_mutated_staged_lib_is_restored_and_never_passes(self):
        root = self._synthetic_root()
        output = root / 'target/cli-mutate-evidence'
        output.mkdir(parents=True)
        workspace = root / 'target/cli-mutate-workspace'
        registry = historical.build_cli_registry(root)
        baseline = historical.prepare_cli_workspace(root, workspace, output, registry)
        t43_043_entry = next(c for c in registry['cases'] if c['id'].endswith('t43_043_inherited_method_main.pkl'))
        case = {
            'id': t43_043_entry['id'],
            'adapter': historical.CLI_ADAPTER,
            'input': t43_043_entry['input'],
            'input_sha256': t43_043_entry['input_sha256'],
            'upstream_case': t43_043_entry['contract'],
        }
        exec_case = historical._cli_execution_case(case)
        mutator = "from pathlib import Path; Path('staged/t43_043_inherited_method/lib.pkl').write_bytes(b'mutated')"
        with tempfile.TemporaryDirectory() as ref_dir:
            ref_root = Path(ref_dir).resolve(strict=True)
            historical._materialize_cli_tree(root, ref_root, registry)
            process, captured = m2_module_eval.capture_process(
                ref_root, workspace, exec_case, [sys.executable, '-c', mutator],
                {'PATH': '/bin:/usr/bin', 'LANG': 'C', 'LC_ALL': 'C', 'TZ': 'UTC', 'TERM': 'dumb', 'NO_COLOR': '1'},
                output / 'mutated',
            )
        self.assertIn('staged/t43_043_inherited_method/lib.pkl', captured['workspace_changes'])
        self.assertEqual(captured['files']['workspace/staged/t43_043_inherited_method/lib.pkl'], b'mutated')
        self.assertEqual(compat.snapshot(workspace), baseline)
        self.assertEqual(m2_module_eval.status(process, process, captured, captured, [0, 1]), 'oracle-error')


class MainRunnerDispatchTests(unittest.TestCase):
    def test_focused_main_runner_and_offline_verifier_dispatch_all_eight_without_build(self):
        manifest=m2_manifest.current_manifest()
        cases=m2_language.selected_cases(manifest,[historical.ADAPTER])
        rows=[{'id':c['id'],'adapter':historical.ADAPTER,'status':'pass'} for c in cases]
        metadata={'command':['synthetic-java'],'environment':{},'artifact':'synthetic-oracle'}
        with tempfile.TemporaryDirectory() as folder:
            base=Path(folder);binary=base/'synthetic-module-probe';binary.write_bytes(b'synthetic artifact, never executed')
            output=base/'target/evidence'
            with patch.object(m2_language.contract,'current_manifest',return_value=manifest), \
                 patch.object(m2_language.oracle,'ensure_oracle',return_value=metadata), \
                 patch.object(m2_language,'source_paths',return_value=['compatibility/m2-language.json']), \
                 patch.object(m2_language,'capture',return_value={'exit_code':0}) as capture, \
                 patch.object(historical,'acquire',return_value=(rows,{'profile':historical.PROFILE})) as acquire, \
                 patch.object(m2_language,'verify_oracle'), \
                 patch.object(historical,'verify_context',return_value={}) as context, \
                 patch.object(historical,'verify_case') as verifier:
                report=m2_language.execute(output,None,adapters=[historical.ADAPTER],module_candidate=binary)
                self.assertEqual(report['schema_version'],6)
                self.assertEqual(report['selection']['case_ids'],[c['id'] for c in cases])
                self.assertEqual(report['selected_count'],8)
                self.assertEqual(report['inventory_counts']['cases'],4586)
                self.assertEqual(report['module_candidate']['source_binding'],'unbound-caller-supplied')
                self.assertEqual(acquire.call_count,1)
                self.assertEqual(acquire.call_args.args[2],cases)
                self.assertEqual(context.call_count,1)
                self.assertEqual(verifier.call_count,8)
                self.assertEqual([c.args[4]['id'] for c in verifier.call_args_list],[c['id'] for c in cases])
                self.assertEqual(capture.call_count,1)
                self.assertEqual(capture.call_args.args[0],['rustc','--version','--verbose'])
                self.assertTrue(report['full_gate_failures'])

    def test_focused_main_runner_and_offline_verifier_dispatch_all_twenty_one_cli_without_build(self):
        manifest=m2_manifest.current_manifest()
        cases=m2_language.selected_cases(manifest,[historical.CLI_ADAPTER])
        rows=[{'id':c['id'],'adapter':historical.CLI_ADAPTER,'status':'pass'} for c in cases]
        metadata={'command':['synthetic-java'],'environment':{},'artifact':'synthetic-oracle'}
        with tempfile.TemporaryDirectory() as folder:
            base=Path(folder);binary=base/'synthetic-module-probe';binary.write_bytes(b'synthetic artifact, never executed')
            output=base/'target/evidence'
            with patch.object(m2_language.contract,'current_manifest',return_value=manifest), \
                 patch.object(m2_language.oracle,'ensure_oracle',return_value=metadata), \
                 patch.object(m2_language,'source_paths',return_value=['compatibility/m2-language.json']), \
                 patch.object(m2_language,'capture',return_value={'exit_code':0}) as capture, \
                 patch.object(historical,'acquire_cli',return_value=(rows,{'profile':historical.CLI_PROFILE})) as acquire, \
                 patch.object(m2_language,'verify_oracle'), \
                 patch.object(historical,'verify_cli_context',return_value={}) as context, \
                 patch.object(historical,'verify_cli_case') as verifier:
                report=m2_language.execute(output,None,adapters=[historical.CLI_ADAPTER],module_candidate=binary)
                self.assertEqual(report['schema_version'],6)
                self.assertEqual(report['selection']['case_ids'],[c['id'] for c in cases])
                self.assertEqual(report['selected_count'],21)
                self.assertEqual(report['inventory_counts']['cases'],4586)
                self.assertEqual(report['module_candidate']['source_binding'],'unbound-caller-supplied')
                self.assertEqual(acquire.call_count,1)
                self.assertEqual(acquire.call_args.args[2],cases)
                self.assertEqual(context.call_count,1)
                self.assertEqual(verifier.call_count,21)
                self.assertEqual([c.args[4]['id'] for c in verifier.call_args_list],[c['id'] for c in cases])
                self.assertEqual(capture.call_count,1)
                self.assertEqual(capture.call_args.args[0],['rustc','--version','--verbose'])
                self.assertTrue(report['full_gate_failures'])



# These controls exercise real report/context validators with tiny synthetic
# resource inventories. Only runtime acquisition and per-case engine readback
# are mocked; they are never Java compatibility observations.
import contextlib
import m2_workspace
import m2_jvm_classpath


class MixedProfileFixture:
    def __init__(self, directory, *, schema=6, profile=m2_workspace.ORIGINAL_PROFILE, historical_selected=True, historical_cli_selected=False, classpath=False):
        self.directory=directory
        directory.mkdir(parents=True)
        self.payloads={'external/pkl/input.pkl':b'value = 1\n','external/pkl-pantry/resource.pkl':b'value = 2\n'}
        sources=[]
        for repository,path,resource in [('pkl','external/pkl','external/pkl/input.pkl'),('pkl-pantry','external/pkl-pantry','external/pkl-pantry/resource.pkl')]:
            resources=[{'path':resource,'kind':'file','sha256':compat.digest(self.payloads[resource])}]
            sources.append({'repository':repository,'path':path,'revision':'synthetic-pinned-'+repository,'url':'https://invalid.example/synthetic/'+repository,
                'resources':resources,'resource_set_sha256':compat.digest(compat.encoded(resources))})
        self.original={'id':'synthetic-upstream','suite':'language-snippet','source':{'repository':'pkl','revision':'synthetic-pinned-pkl','path':'input.pkl','sha256':compat.digest(self.payloads['external/pkl/input.pkl'])},
            'execution':{'status':'ready','reason':None},'command':['eval','--settings','pkl:settings','external/pkl/input.pkl'],
            'cwd':'.','environment':{},'resources':[],'expected_exit':[0],'output':None}
        self.corpus={'sources':sources,'cases':[self.original]}
        module_case={'id':self.original['id'],'adapter':m2_module_eval.ADAPTER,'upstream_case':self.original,
            'input':'external/pkl/input.pkl','input_sha256':self.original['source']['sha256'],
            'required':True,'requirements':[],'execution':{'state':'ready','reason':None}}
        cases=[module_case]
        if historical_selected:cases.append({'id':'synthetic-historical','adapter':historical.ADAPTER,'required':True,'requirements':[],'execution':{'state':'ready','reason':None}})
        if historical_cli_selected:cases.append({'id':'synthetic-historical-cli','adapter':historical.CLI_ADAPTER,'required':True,'requirements':[],'execution':{'state':'ready','reason':None}})
        if classpath:cases.append({'id':m2_jvm_classpath.CASE_ID,'adapter':m2_jvm_classpath.ADAPTER,'required':True,'requirements':[],'execution':{'state':'ready','reason':None}})
        self.manifest={'cases':cases,'requirements':[],'comparison_boundary':'synthetic infrastructure control',
            'module_comparison_boundary':'synthetic exact original module control',
            'sources':{'compatibility/corpus.json':compat.digest(compat.encoded(self.corpus))},
            'counts':{'cases':len(cases)}}
        self.profile=m2_workspace.profile_record(profile,self.corpus,[module_case])
        baseline={}
        for path,payload in self.payloads.items():
            baseline[path]={'kind':'file','sha256':compat.digest(payload)}
            for parent in Path(path).parents:
                if parent!=Path('.'):baseline[str(parent)]={'kind':'directory','sha256':None}
        self.baseline=baseline
        self.declared={path:compat.digest(payload) for path,payload in self.payloads.items()}
        self.rows=[{'id':c['id'],'adapter':c['adapter'],'directory':'synthetic-'+str(i),'status':'pass'} for i,c in enumerate(cases)]
        self.context={'workspace':str(directory/'workspace'),'workers':1,'watchdog_seconds':60,
            'workspace_profile':profile}
        self.report={'schema_version':schema,'oracle':{},'source_stability':True,'artifact_stability':True,
            'selection':{'adapters':[c['adapter'] for c in cases],'case_ids':[c['id'] for c in cases]},
            'selected_count':len(cases),'candidate':None,'classpath_candidate':None,
            'module_candidate':{'sha256':compat.digest(b'synthetic-module-artifact'),'source_binding':'unbound-caller-supplied','build':None},
            'module_eval':self.context,
            'historical_eval':{'profile':historical.PROFILE} if historical_selected else None,
            'historical_cli_eval':{'profile':historical.CLI_PROFILE} if historical_cli_selected else None,
            'classpath_eval':{'directory':'jvm-classpath'} if classpath else None,
            'results':self.rows,'metamorphic_results':[],'status_counts':{'pass':len(cases)},
            'full_gate_failures':m2_manifest.full_gate_failures(self.manifest,self.rows)}
        (directory/'artifacts').mkdir()
        (directory/'artifacts/module_eval_probe').write_bytes(b'synthetic-module-artifact')
        if classpath:
            (directory/'artifacts/jvm_snippet_probe').write_bytes(b'synthetic-classpath-artifact')
            self.report['classpath_candidate']={'sha256':compat.digest(b'synthetic-classpath-artifact'),'source_binding':'unbound-caller-supplied','build':None}
        self.write_payloads()

    def write_payloads(self):
        self.source_payloads={'compatibility/m2-language.json':compat.encoded(self.manifest),
            'compatibility/corpus.json':compat.encoded(self.corpus),
            'tools/m2_workspace.py':(m2_manifest.ROOT/'tools/m2_workspace.py').read_bytes()}
        for name,data in self.source_payloads.items():
            path=self.directory/'sources'/name;path.parent.mkdir(parents=True,exist_ok=True);path.write_bytes(data)
        (self.directory/'module-workspace.json').write_bytes(compat.encoded(self.baseline))
        (self.directory/'module-inputs.json').write_bytes(compat.encoded({'files':len(self.declared),'sha256':compat.digest(compat.encoded(self.declared))}))
        (self.directory/'module-workspace-profile.json').write_bytes(compat.encoded(self.profile))
        self.context.update(baseline_sha256=compat.digest((self.directory/'module-workspace.json').read_bytes()),
            input_validation_sha256=compat.digest((self.directory/'module-inputs.json').read_bytes()),
            workspace_profile_sha256=compat.digest((self.directory/'module-workspace-profile.json').read_bytes()))
        self.save_report()

    def save_report(self):
        self.report['source_sha256']={name:compat.digest(data) for name,data in self.source_payloads.items()}
        (self.directory/'selection.json').write_bytes(compat.encoded(self.report['selection']))
        self.report['evidence_sha256']={str(p.relative_to(self.directory)):compat.digest(p.read_bytes()) for p in m2_language.evidence_paths(self.directory)}
        (self.directory/'report.json').write_bytes(compat.encoded(self.report))

    def check(self, *, reject_child=False):
        with contextlib.ExitStack() as stack:
            stack.enter_context(patch.object(m2_manifest,'current_manifest',return_value=self.manifest))
            stack.enter_context(patch.object(m2_language,'source_paths',return_value=list(self.source_payloads)))
            stack.enter_context(patch.object(m2_language,'verify_oracle'))
            module_read=stack.enter_context(patch.object(m2_module_eval,'verify_case'))
            history_context=stack.enter_context(patch.object(historical,'verify_context',return_value={}))
            history_read=stack.enter_context(patch.object(historical,'verify_case'))
            history_cli_context=stack.enter_context(patch.object(historical,'verify_cli_context',return_value={}))
            history_cli_read=stack.enter_context(patch.object(historical,'verify_cli_case'))
            baseline=stack.enter_context(patch.object(m2_workspace,'validate_baseline',wraps=m2_workspace.validate_baseline))
            child=stack.enter_context(patch.object(m2_language,'verify_classpath_child',return_value={'synthetic':'child'},
                side_effect=ValueError('synthetic child integrity rejection') if reject_child else None))
            child_row=next((r for r in self.rows if r['adapter']==m2_jvm_classpath.ADAPTER),None)
            stack.enter_context(patch.object(m2_language,'classpath_result',return_value=child_row))
            result=m2_language.check_report(self.directory)
            return result,{'module':module_read.call_count,'history':history_read.call_count,
                'history_context':history_context.call_count,'history_cli':history_cli_read.call_count,
                'history_cli_context':history_cli_context.call_count,'baseline':baseline.call_count,'child':child.call_count}


class MixedProfileTests(unittest.TestCase):
    def fixture(self, **options):
        temporary=tempfile.TemporaryDirectory();self.addCleanup(temporary.cleanup)
        return MixedProfileFixture(Path(temporary.name)/'capture',**options)

    def test_original_schema5_profile_boundary_remains_valid(self):
        fixture=self.fixture(schema=5,historical_selected=False)
        result,calls=fixture.check()
        self.assertEqual(calls,{'module':1,'history':0,'history_context':0,'history_cli':0,'history_cli_context':0,'baseline':1,'child':0})

    def test_schema6_upstream_and_historical_inherit_exact_original_profile(self):
        fixture=self.fixture()
        result,calls=fixture.check()
        self.assertEqual(calls,{'module':1,'history':1,'history_context':1,'history_cli':0,'history_cli_context':0,'baseline':1,'child':0})

    def test_schema6_upstream_historical_and_cli_inherit_exact_original_profile(self):
        fixture=self.fixture(historical_cli_selected=True)
        result,calls=fixture.check()
        self.assertEqual(calls,{'module':1,'history':1,'history_context':1,'history_cli':1,'history_cli_context':1,'baseline':1,'child':0})

    def test_schema6_rejects_unselected_historical_cli_context(self):
        fixture=self.fixture()
        fixture.report['historical_cli_eval']={'profile':'not-selected'};fixture.save_report()
        with self.assertRaisesRegex(ValueError,'unselected historical CLI context'):fixture.check()

    def test_schema6_upstream_and_historical_preserve_legacy_profile(self):
        fixture=self.fixture(profile=m2_workspace.DEFAULT_PROFILE)
        result,calls=fixture.check()
        self.assertEqual(calls['baseline'],1)
        self.assertEqual(result['module_eval']['workspace_profile'],m2_workspace.DEFAULT_PROFILE)

    def test_schema6_module_historical_and_jvm_keep_child_dispatch_and_rejection(self):
        fixture=self.fixture(classpath=True)
        result,calls=fixture.check()
        self.assertEqual(calls,{'module':1,'history':1,'history_context':1,'history_cli':0,'history_cli_context':0,'baseline':1,'child':1})
        with self.assertRaisesRegex(ValueError,'synthetic child integrity rejection'):
            fixture.check(reject_child=True)

    def test_schema6_rejects_inherited_corpus_profile_hash_baseline_and_omission_tamper(self):
        mutations={'corpus-pin':'module-eval corpus differs','profile-bytes':'workspace profile provenance differs',
            'profile-hash':'workspace profile provenance differs','baseline-extra':'original module workspace baseline differs',
            'missing-profile':'unsupported module workspace profile','missing-profile-hash':'workspace profile provenance differs',
            'workers':'module-eval staged input provenance differs','watchdog':'module-eval staged input provenance differs'}
        for change,reason in mutations.items():
            fixture=self.fixture()
            if change=='corpus-pin':
                fixture.manifest['sources']['compatibility/corpus.json']='0'*64
                fixture.source_payloads['compatibility/m2-language.json']=compat.encoded(fixture.manifest)
                (fixture.directory/'sources/compatibility/m2-language.json').write_bytes(fixture.source_payloads['compatibility/m2-language.json'])
            elif change=='profile-bytes':(fixture.directory/'module-workspace-profile.json').write_bytes(b'{}\n')
            elif change=='profile-hash':fixture.context['workspace_profile_sha256']='0'*64
            elif change=='baseline-extra':
                fixture.baseline['unexpected.pkl']={'kind':'file','sha256':compat.digest(b'unexpected')}
                (fixture.directory/'module-workspace.json').write_bytes(compat.encoded(fixture.baseline))
                fixture.context['baseline_sha256']=compat.digest((fixture.directory/'module-workspace.json').read_bytes())
            elif change=='missing-profile':fixture.context.pop('workspace_profile')
            elif change=='missing-profile-hash':fixture.context.pop('workspace_profile_sha256')
            elif change=='workers':fixture.context['workers']=2
            elif change=='watchdog':fixture.context['watchdog_seconds']=61
            fixture.save_report()
            with self.subTest(change=change),self.assertRaisesRegex(ValueError,reason):fixture.check()

    def test_schema6_rejects_unselected_classpath_context(self):
        fixture=self.fixture()
        fixture.report['classpath_eval']={'directory':'not-selected'};fixture.save_report()
        with self.assertRaisesRegex(ValueError,'unselected JVM-classpath child context'):fixture.check()

    def test_main_runner_emits_and_verifies_mixed_module_and_historical_profiles(self):
        fixture=self.fixture(profile=m2_workspace.DEFAULT_PROFILE)
        with tempfile.TemporaryDirectory() as folder:
            # macOS TemporaryDirectory may spell its existing parent through
            # /var -> /private/var. Use the physical fixture root; production
            # rejection of symlinks inside managed paths remains unchanged.
            root=Path(folder).resolve(strict=True);output=root/'target/evidence'
            compat.no_symlinks(root)
            for name,data in fixture.source_payloads.items():
                path=root/name;path.parent.mkdir(parents=True,exist_ok=True);path.write_bytes(data)
            for name,data in fixture.payloads.items():
                path=root/name;path.parent.mkdir(parents=True,exist_ok=True);path.write_bytes(data)
            lock = ['baseline.pkl=0.32.0']
            for source in fixture.corpus['sources']:
                lock += [source['repository']+'.'+key+'='+source[key] for key in ('path','revision','url')]
            (root/'references.lock').write_text('\n'.join(lock)+'\n')
            def tracked(checkout):
                return [str(Path(r['path']).relative_to(source['path'])) for source in fixture.corpus['sources'] if root/source['path']==checkout for r in source['resources']]
            def git(command):
                if command[-2:] == ['rev-parse','HEAD']:
                    source=next(s for s in fixture.corpus['sources'] if str(root/s['path'])==command[2])
                    return (source['revision']+'\n').encode()
                if command[3:5] == ['ls-tree','HEAD']:
                    source=next(s for s in fixture.corpus['sources'] if s['path']==command[-1])
                    return ('160000 commit '+source['revision']+'\t'+source['path']+'\n').encode()
                raise AssertionError(command)
            binary=root/'synthetic-native';binary.write_bytes(b'never executed')
            metadata={'command':['synthetic-java'],'environment':{},'artifact':'synthetic-oracle'}
            def prepare(root,workspace,profile):
                workspace.mkdir(parents=True)
                compat.write_json(workspace/compat.MARKER,m2_workspace.workspace_identity(root,workspace,profile))
                for name,data in fixture.payloads.items():
                    path=workspace/name;path.parent.mkdir(parents=True,exist_ok=True);path.write_bytes(data)
            module_row=next(r for r in fixture.rows if r['adapter']==m2_module_eval.ADAPTER)
            history_rows=[r for r in fixture.rows if r['adapter']==historical.ADAPTER]
            with contextlib.ExitStack() as stack:
                stack.enter_context(patch.object(m2_language,'ROOT',root))
                stack.enter_context(patch.object(m2_manifest,'current_manifest',return_value=fixture.manifest))
                stack.enter_context(patch.object(m2_manifest,'load',return_value=fixture.corpus))
                stack.enter_context(patch.object(m2_language,'source_paths',return_value=list(fixture.source_payloads)))
                stack.enter_context(patch.object(m2_language.oracle,'ensure_oracle',return_value=metadata))
                capture=stack.enter_context(patch.object(m2_language,'capture',return_value={'exit_code':0}))
                stack.enter_context(patch.object(m2_workspace,'prepare',side_effect=prepare))
                stack.enter_context(patch.object(compat,'tracked_paths',side_effect=tracked))
                stack.enter_context(patch.object(compat.subprocess,'check_output',side_effect=git))
                stack.enter_context(patch.object(m2_module_eval,'acquire_case',return_value=module_row))
                stack.enter_context(patch.object(historical,'acquire',return_value=(history_rows,{'profile':historical.PROFILE})))
                stack.enter_context(patch.object(m2_language,'verify_oracle'))
                stack.enter_context(patch.object(m2_module_eval,'verify_case'))
                stack.enter_context(patch.object(historical,'verify_context',return_value={}))
                stack.enter_context(patch.object(historical,'verify_case'))
                baseline=stack.enter_context(patch.object(m2_workspace,'validate_baseline',wraps=m2_workspace.validate_baseline))
                report=m2_language.execute(output,None,adapters=[m2_module_eval.ADAPTER,historical.ADAPTER],module_candidate=binary)
                self.assertEqual(report['schema_version'],6)
                self.assertEqual(report['selected_count'],2)
                self.assertEqual(baseline.call_count,2)
                self.assertEqual(report['module_eval']['workspace_profile'],m2_workspace.DEFAULT_PROFILE)
                self.assertEqual(capture.call_count,1)
                self.assertEqual(capture.call_args.args[0],['rustc','--version','--verbose'])
                self.assertTrue(report['full_gate_failures'])


if __name__=='__main__':unittest.main()
