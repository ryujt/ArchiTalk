#!/usr/bin/python3
import sys,json,time
if '--version' in sys.argv:
 print('2.1.326 (Claude Code)');sys.exit(0)
if '--help' in sys.argv:
 print('--print --output-format --include-partial-messages --safe-mode --restricted --strict-mcp-config --mcp-config --setting-sources --settings --tools --permission-mode --disable-slash-commands --no-session-persistence --effort');sys.exit(0)
prompt=sys.stdin.read()
print(json.dumps({'type':'system','subtype':'init','tools':[],'mcp_servers':[],'permissionMode':'dontAsk','model':'fixture-model'}),flush=True)
if 'SLOW_REQUEST' in prompt:time.sleep(30)
if 'Independently review the latest architecture' in prompt:
 result={'findings':[]}
elif 'Create an implementable software architecture document' in prompt:
 result={'markdown':'# 알림 서비스 아키텍처\n\n## 목적과 범위\n회원에게 가입 안내를 보내는 작은 서비스입니다. 사용자가 확정하지 않은 부하와 전달 시간은 미정입니다.\n\n## 구조\n하나의 애플리케이션이 요청과 발송 이력을 관리하고 외부 발송사와 통신합니다. 발송 상태의 유일한 소유자는 알림 모듈입니다.\n\n## 대안과 실패\n직접 발송과 작업 큐를 비교하고 초기에는 단순한 작업 큐를 제안합니다. 외부 요청 실패는 중복 키와 결과 조회로 해결합니다. 인증과 보관 기간은 확인이 필요합니다.\n\n## 검증 계획\n중복 요청, 외부 실패, 재시작, 권한 거부를 검증합니다.'}
else:
 context=json.loads(prompt.split('CONTEXT_JSON:\n',1)[1]);source=[m for m in context['messages'] if m['role']=='user'][-1]
 result={'reply':'현재 말씀하신 내용은 제안과 미정으로 나누어 기록했습니다.','questions':[{'target_ref':'scope','text':'첫 버전에서 꼭 필요한 기능은 무엇인가요?','reason':'초기 범위를 정하기 위해서입니다.','options':['가입 알림','예약 알림'],'priority_class':3}],'claims':[{'text':'사용자가 전달한 요구를 확인할 후보입니다.','kind':'interpretation','evidence_refs':[source['id']],'source_quote':source['content']}]}
text=json.dumps(result,ensure_ascii=False)
print(json.dumps({'type':'assistant','message':{'content':[{'type':'text','text':text}]}},ensure_ascii=False),flush=True)
print(json.dumps({'type':'result','subtype':'success','is_error':False,'result':text},ensure_ascii=False),flush=True)
