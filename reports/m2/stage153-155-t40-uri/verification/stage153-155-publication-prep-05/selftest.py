"""Pure exact-schema controls; no payload hashing, producer or archive access."""
import ast,copy,unittest
from pathlib import Path
from common import ROOT,process_channel_pin,re
P=Path(__file__).absolute().parent
T=ast.parse((P/'select.py').read_text());ns=dict(ROOT=ROOT,process_channel_pin=process_channel_pin,re=re)
nodes=[n for n in T.body if(isinstance(n,ast.Assign)and any(isinstance(v,ast.Name)and v.id in ('FAILED_REGISTRY_DIGEST_DOCUMENT','FAILED_RESULT_DIGEST_DOCUMENTS')for v in n.targets))or(isinstance(n,ast.FunctionDef)and n.name in ('channel_projection','raw_channels'))]
exec(compile(ast.Module(nodes,[]),'<pure-failed-digests>','exec'),ns)
class SchemaControls(unittest.TestCase):
 def test_exact_four_historical_contract_digest_fields(self):
  path=ns['FAILED_REGISTRY_DIGEST_DOCUMENT']['path']
  for i in (0,1):
   for channel in ('stdout','stderr'):self.assertEqual(ns['channel_projection'](path,('cases',i,'contract','oracle_observation','sha256',channel),'a'*64),[])
 def test_eight_results_join_exact_sibling_stream_pins(self):
  for path,rows in ns['FAILED_RESULT_DIGEST_DOCUMENTS'].items():
   for channel in ('stdout','stderr'):self.assertEqual(ns['channel_projection'](path,('sha256',channel),rows[channel]['sha256']),[rows[channel]])
 def test_result_digest_mismatch_and_malformed_values_reject(self):
  path,rows=next(iter(ns['FAILED_RESULT_DIGEST_DOCUMENTS'].items()))
  for bad in ('BAD','f'*63,0,True,{},'f'*64):
   with self.assertRaises(AssertionError):ns['channel_projection'](path,('sha256','stdout'),bad)
 def test_unknown_document_or_field_never_gets_digest_waiver(self):
  path=ns['FAILED_REGISTRY_DIGEST_DOCUMENT']['path']
  for doc,trail in [(str(ROOT/'target/m2/unrelated.json'),('sha256','stdout')),(path,('sha256','stdout')),(path,('cases',2,'contract','oracle_observation','sha256','stdout')),(path,('cases',True,'contract','oracle_observation','sha256','stdout'))]:
   with self.assertRaises(AssertionError):ns['channel_projection'](doc,trail,'a'*64)
 def test_malformed_real_process_channel_remains_rejected(self):
  with self.assertRaises(AssertionError):ns['channel_projection'](str(ROOT/'target/m2/guard.json'),('commands',0,'stdout'),dict(path=str(ROOT/'target/m2/0.stdout.bin'),bytes=0))
 def test_nested_result_adds_both_exact_stream_requests(self):
  path,rows=next(iter(ns['FAILED_RESULT_DIGEST_DOCUMENTS'].items()));requests=[];ns['add']=lambda row,group:requests.append(row)
  ns['raw_channels'](dict(sha256={key:rows[key]['sha256']for key in ('stdout','stderr')}),'preserved-failures',path)
  self.assertEqual(requests,[rows['stdout'],rows['stderr']])
if __name__=='__main__':unittest.main(verbosity=2)
