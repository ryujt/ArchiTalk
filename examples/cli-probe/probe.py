#!/usr/bin/env python3
"""Read-only CLI protocol spike. Actual model calls require --real.

Evidence stays in a private OS temporary directory; credentials are never read.
The product adapter is authoritative; this probe records external CLI behavior.
"""
import argparse, json, os, pathlib, shutil, subprocess, tempfile, time, signal

parser = argparse.ArgumentParser()
parser.add_argument('--real', action='store_true')
parser.add_argument('--stop-on-new-system', action='store_true', help='Stop the owned CLI at the first previously unhandled system subtype')
parser.add_argument('--headers-only', action='store_true', help='Print only protocol type/subtype and tool-permission metadata; retain no raw output')
parser.add_argument('--prompt-file', type=pathlib.Path)
parser.add_argument('--effort', choices=['low','medium','high','xhigh','max'])
parser.add_argument('--backend', choices=['claude', 'codex', 'both'], default='both')
def safe_header(value):
    return value if isinstance(value,str) and 0<len(value)<=64 and all(c.isascii() and (c.isalnum() or c in '_') for c in value) else 'unknown'

def capture_headers(command, prompt, cwd, env):
    process=subprocess.Popen(command,cwd=cwd,env=env,stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,start_new_session=True,text=True)
    deadline=time.monotonic()+90
    first=True
    observed={}
    stopped=False
    while True:
        try:
            stdout,stderr=process.communicate(input=prompt if first else None,timeout=.5)
            complete=True
        except subprocess.TimeoutExpired as error:
            stdout=error.stdout or b''
            stdout=stdout.decode('utf-8',errors='replace') if isinstance(stdout,bytes) else stdout
            complete=False
        first=False
        for line in stdout.splitlines():
            try:event=json.loads(line)
            except ValueError:continue
            if event.get('type')!='system':continue
            subtype=safe_header(event.get('subtype'))
            header={'type':'system','subtype':subtype}
            if subtype=='init':
                header.update(tools_count=len(event.get('tools',[])),mcp_count=len(event.get('mcp_servers',[])),permission_mode=safe_header(event.get('permissionMode')))
            observed[subtype]=header
        (cwd/'protocol-headers.json').write_text(json.dumps(list(observed.values()),ensure_ascii=True))
        novel=next((key for key in observed if key not in ['init','status']),None)
        if complete:break
        if (args.stop_on_new_system and novel) or time.monotonic()>deadline:
            os.killpg(process.pid,signal.SIGTERM)
            try:process.communicate(timeout=3)
            except subprocess.TimeoutExpired:
                os.killpg(process.pid,signal.SIGKILL);process.communicate(timeout=3)
            stopped=True
            break
    for header in observed.values():print('HEADER',json.dumps(header),flush=True)
    print('HEADER_RESULT','exit=',process.returncode,'owned_probe_stopped=',stopped,flush=True)

args = parser.parse_args()
root = pathlib.Path(__file__).resolve().parents[2]
run = pathlib.Path(tempfile.mkdtemp(prefix='architalk-cli-probe-')).resolve()
profile = '(version 1)\n(allow default)\n(deny file-write* (subpath ' + json.dumps(str(root)) + '))\n'
(run / 'project-write-deny.sb').write_text(profile)
probe_target = root / '.architalk-write-probe-denied'
start = time.strftime('%Y-%m-%d %H:%M:%S %Z')
print('START', start, 'cwd_outside_project=', root not in run.parents, flush=True)
blocked = subprocess.run(['/usr/bin/sandbox-exec', '-f', str(run / 'project-write-deny.sb'), '/usr/bin/touch', str(probe_target)], cwd=run, capture_output=True, text=True)
print('OS_WRITE_DENY', blocked.returncode, 'target_exists=', probe_target.exists(), flush=True)
if blocked.returncode == 0 or probe_target.exists():
    raise SystemExit('OS boundary failed; no model call permitted')
if args.real:
    common_disable = ['shell_tool', 'unified_exec', 'hooks', 'plugins', 'apps', 'multi_agent', 'multi_agent_v2', 'browser_use', 'computer_use', 'image_generation', 'code_mode', 'code_mode_host', 'in_app_browser', 'shell_snapshot', 'memories', 'skill_search', 'skill_mcp_dependency_install', 'remote_plugin', 'workspace_dependencies']
    commands = {
      'claude': ['claude','-p','--output-format','stream-json','--verbose','--include-partial-messages','--no-session-persistence','--safe-mode','--restricted','--tools','','--strict-mcp-config','--mcp-config','{"mcpServers":{}}','--disable-slash-commands','--permission-mode','dontAsk','--setting-sources','','--settings','{"disableAllHooks":true}','--max-turns','1'],
      'codex': ['codex','--no-daemon','-a','never','-s','read-only','exec','--ignore-user-config','--ignore-rules','--skip-git-repo-check','--ephemeral','--json','--color','never','-c','project_doc_max_bytes=0','-c','web_search="disabled"','-c','developer_instructions=""'] + sum((['--disable', x] for x in common_disable), []) + ['-'],
    }
    for backend in (['claude','codex'] if args.backend == 'both' else [args.backend]):
        cmd = commands[backend]
        cmd[0] = shutil.which(cmd[0])
        if args.effort:
            if backend == 'claude': cmd.extend(['--effort',args.effort])
            else: cmd[-1:-1]=['-c','model_reasoning_effort='+json.dumps(args.effort)]
        print('CALL', backend, flush=True)
        if args.headers_only:
            allowed={'HOME','USER','LOGNAME','PATH','SHELL','TMPDIR','LANG','LC_ALL','SSL_CERT_FILE','SSL_CERT_DIR','HTTP_PROXY','HTTPS_PROXY','ALL_PROXY','NO_PROXY','http_proxy','https_proxy','all_proxy','no_proxy','CODEX_HOME','ANTHROPIC_API_KEY','ANTHROPIC_AUTH_TOKEN','CLAUDE_CODE_OAUTH_TOKEN','OPENAI_API_KEY','OPENAI_BASE_URL'}
            env={key:value for key,value in os.environ.items() if key in allowed}
            capture_headers(['/usr/bin/sandbox-exec','-f',str(run/'project-write-deny.sb')]+cmd,args.prompt_file.read_text() if args.prompt_file else 'Reply ARCHITALK_OK.',run,env)
            continue
        try:
            result = subprocess.run(['/usr/bin/sandbox-exec','-f',str(run / 'project-write-deny.sb')] + cmd, cwd=run, input=args.prompt_file.read_text() if args.prompt_file else 'Reply exactly ARCHITALK_OK. Do not use tools.\n', capture_output=True, text=True, timeout=90, env={key:value for key,value in os.environ.items() if key in {'HOME','USER','LOGNAME','PATH','SHELL','TMPDIR','LANG','LC_ALL','SSL_CERT_FILE','SSL_CERT_DIR','HTTP_PROXY','HTTPS_PROXY','ALL_PROXY','NO_PROXY','http_proxy','https_proxy','all_proxy','no_proxy','CODEX_HOME','ANTHROPIC_API_KEY','ANTHROPIC_AUTH_TOKEN','CLAUDE_CODE_OAUTH_TOKEN','OPENAI_API_KEY','OPENAI_BASE_URL'}})
        except subprocess.TimeoutExpired:
            print('TIMEOUT', backend, flush=True)
            continue
        if not args.headers_only:
            (run / (backend + '.jsonl')).write_text(result.stdout)
            (run / (backend + '.stderr')).write_text(result.stderr)
        events=[]
        for line in result.stdout.splitlines():
            try: events.append(json.loads(line))
            except ValueError: print('NON_JSON', backend, '<content omitted>' if args.headers_only else line[:200], flush=True)
        print('RESULT', backend, 'exit=', result.returncode, 'types=', [e.get('type') for e in events], flush=True)
        for event in events:
            if args.headers_only:
                safe=lambda value: value if isinstance(value,str) and len(value)<=64 and all(c.isascii() and (c.isalnum() or c in '_.') for c in value) else '<unknown>'
                print('HEADER',backend,{'type':safe(event.get('type')),'subtype':safe(event.get('subtype'))},flush=True)
                if event.get('type')=='system' and event.get('subtype')=='init':
                    print('PERMISSIONS',backend, {key:event.get(key) for key in ['tools','mcp_servers','permissionMode']},flush=True)
                continue
            if event.get('type') in ['result','turn.completed','turn.failed','error']:
                print('TERMINAL', backend, json.dumps(event, ensure_ascii=False)[:3000], flush=True)
            if event.get('type') == 'system':
                print('INIT', backend, {key:event.get(key) for key in ['subtype','model','tools','mcp_servers','permissionMode']}, flush=True)
            if event.get('type') == 'item.completed':
                print('ITEM', backend, json.dumps(event.get('item'),ensure_ascii=False)[:1500], flush=True)
        if result.returncode:
            print('STDERR', backend, '<content omitted>' if args.headers_only else result.stderr[:1500], flush=True)
print('END',time.strftime('%Y-%m-%d %H:%M:%S %Z'),flush=True)
print('private_evidence=', str(run), flush=True)
