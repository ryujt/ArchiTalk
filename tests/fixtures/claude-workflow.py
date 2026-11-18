#!/usr/bin/python3
import sys,json,time
from pathlib import Path
if '--version' in sys.argv:
 print('2.1.326 (Claude Code)');sys.exit(0)
if '--help' in sys.argv:
 print('--print --output-format --include-partial-messages --safe-mode --restricted --strict-mcp-config --mcp-config --setting-sources --settings --tools --permission-mode --disable-slash-commands --no-session-persistence --effort');sys.exit(0)
prompt=sys.stdin.read()
print(json.dumps({'type':'system','subtype':'init','tools':[],'mcp_servers':[],'permissionMode':'dontAsk','model':'fixture-model'}),flush=True)
if 'SLOW_REQUEST' in prompt:time.sleep(30)
task=prompt.split('TASK:\n',1)[1].split('\n',1)[0]
target=json.JSONDecoder().raw_decode(prompt.split('TARGET_JSON:\n',1)[1])[0]['kind'] if 'TARGET_JSON:\n' in prompt else None
if target is not None:task='Create an implementable single document'
if task.startswith('Independently review'):
 result={'findings':[]}
elif task.startswith('Create an implementable'):
 package=Path(__file__).with_name('design-package.json')
 result=json.loads(package.read_text(encoding='utf-8')) if package.exists() else {'markdown':Path(__file__).with_name('architecture.smart.md').read_text(encoding='utf-8')}
 if target:
  control=Path(__file__).with_name('stage-control.json')
  deadline=time.monotonic()+20
  while control.exists():
   options=json.loads(control.read_text(encoding='utf-8'))
   if options.get('hold_kind')!=target:break
   if time.monotonic()>deadline:raise RuntimeError('test parent did not release stage')
   time.sleep(0.02)
  options=json.loads(control.read_text(encoding='utf-8')) if control.exists() else {}
  markdown=result['markdown'] if target=='architecture' or 'documents' not in result else next(v['markdown'] for v in result['documents'] if v['kind']==target)
  markdown=markdown.replace('아키텍처-설계.md#method-r-module-design','아키텍처-설계.md').replace('네비게이션.md#screen-verification','네비게이션.md')
  result={'kind':target,'markdown':'invalid synthetic stage' if options.get('fail_kind')==target else markdown}
else:
 context=json.loads(prompt.split('CONTEXT_JSON:\n',1)[1]);source=[m for m in context['messages'] if m['role']=='user'][-1]
 result={'reply':'현재 말씀하신 내용은 제안과 미정으로 나누어 기록했습니다.','questions':[{'target_ref':'scope','text':'첫 버전에서 꼭 필요한 기능은 무엇인가요?','reason':'초기 범위를 정하기 위해서입니다.','options':['가입 알림','예약 알림'],'priority_class':3}],'claims':[{'text':'사용자가 전달한 요구를 확인할 후보입니다.','kind':'interpretation','evidence_refs':[source['id']],'source_quote':source['content']}]}
text=json.dumps(result,ensure_ascii=False)
print(json.dumps({'type':'assistant','message':{'content':[{'type':'text','text':text}]}},ensure_ascii=False),flush=True)
print(json.dumps({'type':'result','subtype':'success','is_error':False,'result':text},ensure_ascii=False),flush=True)
