import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {webcrypto} from 'node:crypto';

// Load the same browser module without adding a Node package manifest to the
// Rust application. These are protocol/state tests, not browser rendering QA.
const source=await readFile(new URL('../web/app.js',import.meta.url),'utf8');
const {takeToken,createApi,ApiError,createSSEParser,readSSE,draftIsCurrent,
  approvalBlockers,safeRelativeDirectory,settingsFromValues,startWorkspace}=await import(`data:text/javascript;base64,${Buffer.from(source).toString('base64')}`);
const encode=new TextEncoder();
const settings={ai:{default_backend:'claude',timeout_seconds:180,
  backends:{claude:{executable:'/trusted/claude',model:null,effort:null,profile:null},codex:{executable:'/trusted/codex',model:'chosen-model',effort:'high',profile:'trusted-profile'}},
  roles:{conversation:{backend:null,model:null,effort:null},design:{backend:null,model:null,effort:null},critic:{backend:null,model:null,effort:null}}},
  documents:{directory:'docs/architecture',language:'ko'}};

test('capability fragment is removed before storage or requests, with in-memory fallback',()=>{
  const order=[],map=new Map();
  const token=takeToken({hash:'#token=secret%2Bvalue',pathname:'/',search:'?view=local'},
    {replaceState(...args){order.push(['replace',...args]);}},
    {setItem(key,value){order.push(['store',key,value]);map.set(key,value);},getItem(key){return map.get(key);}});
  assert.equal(token,'secret+value');
  assert.equal(order[0][0],'replace');
  assert.equal(order[0][3],'/?view=local');
  assert.equal(takeToken({hash:'#token=memory',pathname:'/',search:''},{replaceState(){}},{setItem(){throw new Error('disabled');}}),'memory');
  const expired=new Map([['architalk.authorization','old']]);
  assert.equal(takeToken({hash:'#token=',pathname:'/',search:''},{replaceState(){}},{getItem:key=>expired.get(key),removeItem:key=>expired.delete(key)}),'');
  assert.equal(expired.has('architalk.authorization'),false);
  assert.equal(takeToken({hash:'',pathname:'/',search:''},{replaceState(){assert.fail('no token to remove');}},{getItem(){return 'remembered';}}),'remembered');
});

test('every API call uses the authorization header and same-origin, no-redirect fetch',async()=>{
  const calls=[];
  const api=createApi('private',(path,options)=>{
    calls.push({path,options});
    return Promise.resolve(new Response(JSON.stringify({ok:true}),{headers:{'content-type':'application/json'}}));
  });
  assert.deepEqual(await api('/api/v1/settings',{method:'PATCH',body:{command_id:'id',expected_revision:4,payload:{settings}}}),{ok:true});
  const {path,options}=calls[0];
  assert.equal(path,'/api/v1/settings');
  assert.equal(options.headers.get('Authorization'),'Bearer private');
  assert.equal(options.mode,'same-origin');
  assert.equal(options.redirect,'error');
  assert.equal(options.cache,'no-store');
  assert.equal(JSON.parse(options.body).payload.settings.ai.backends.codex.profile,'trusted-profile');
  await assert.rejects(api('https://foreign.invalid/api/v1/settings'));
  await assert.rejects(api('//foreign.invalid/api/v1/settings'));
  assert.equal(calls.length,1);
});

test('HTTP errors preserve machine code and safe server message; stream MIME is checked',async()=>{
  const api=createApi('token',async()=>new Response(JSON.stringify({error:{code:'revision_conflict',message:'최신 기록을 확인해 주세요.'},snapshot:{revision:7}}),{status:409}));
  await assert.rejects(api('/api/v1/sessions/a'),error=>error instanceof ApiError&&error.code==='revision_conflict'&&error.snapshot.revision===7);
  const html=createApi('token',async()=>new Response('<html>redirect</html>',{headers:{'content-type':'text/html'}}));
  await assert.rejects(html('/api/v1/sessions/a/events',{stream:true}),/응답 형식/);
});

test('SSE handles each byte boundary, UTF-8, CRLF, BOM, comments and multiple data lines',()=>{
  const content='\uFEFF: keepalive\r\nid: 42\r\nevent: state\r\ndata: {"text":\r\ndata: "한글 😀"}\r\n\r\ndata: second\n\n';
  for(let split=0;split<=encode.encode(content).length;split++){
    const events=[],parser=createSSEParser(frame=>events.push(frame)),bytes=encode.encode(content);
    parser.push(bytes.slice(0,split));parser.push(bytes.slice(split));parser.end();
    assert.deepEqual(events,[{id:'42',event:'state',data:'{"text":\n"한글 😀"}'},{event:'message',data:'second'}],`split ${split}`);
  }
  const events=[],parser=createSSEParser(frame=>events.push(frame));
  for(const byte of encode.encode(content))parser.push(Uint8Array.of(byte));
  parser.end();assert.equal(events[0].id,'42');assert.match(events[0].data,/한글 😀/);
});

test('SSE discards incomplete frames, ignores NUL IDs, and bounds untrusted buffers',()=>{
  const events=[],parser=createSSEParser(frame=>events.push(frame));
  parser.push(encode.encode('id: invalid\0id\ndata: full\n\ndata: incomplete\n'));parser.end();
  assert.deepEqual(events,[{event:'message',data:'full'}]);
  const limited=createSSEParser(()=>{}, {maxBuffer:20});
  assert.throws(()=>limited.push(encode.encode('data: '+ 'x'.repeat(30))),/크기 제한/);
  const frameLimited=createSSEParser(()=>{}, {maxBuffer:20});
  assert.throws(()=>frameLimited.push(encode.encode('data: 1234567890\ndata: 1234567890\n\n')),/너무 커서/);
});

test('the fetch SSE reader joins chunks and supports abort without another request',async()=>{
  const events=[],controller=new AbortController();let cancelled=false;
  const body=new ReadableStream({start(stream){stream.enqueue(encode.encode('data: {"value":"실제"}\r'));stream.enqueue(encode.encode('\n\r\n'));},cancel(){cancelled=true;}});
  const reading=readSSE(new Response(body),event=>{events.push(event);controller.abort();},controller.signal);
  await reading;
  assert.equal(events[0].data,'{"value":"실제"}');assert.equal(cancelled,true);
});

test('semantic draft revision is separate from mutable session metadata; review blockers stay explicit',()=>{
  const draft={id:'draft-1',input_revision:3,reviewed:true};
  const session={revision:90,design_revision:3,coverage:[],findings:[],requirements:[]};
  assert.equal(draftIsCurrent(session,draft),true);
  assert.deepEqual(approvalBlockers(session,draft),[]);
  session.design_revision=4;assert.equal(draftIsCurrent(session,draft),false);
  session.coverage=[{id:'risk',title:'복구',required:true,status:'deferred',blocker:true,owner:'담당',follow_up:'출시 전'}];
  session.findings=[{draft_id:'draft-1',severity:'major',text:'중복 실행',disposition:'accepted'}];
  assert.equal(approvalBlockers(session,draft).length,3);
  session.design_revision=3;session.coverage[0].blocker=false;session.findings[0].disposition='resolved';
  assert.deepEqual(approvalBlockers(session,draft),[]);
  assert.deepEqual(approvalBlockers({...session,readiness_issues:[]},{...draft,reviewed:false}),['독립 비판 검토가 아직 완료되지 않았어요.']);
});

test('settings forms preserve trusted executables/profiles and set per-backend/role overrides',()=>{
  const changed=settingsFromValues(settings,{'default-backend':'codex','claude-model':' custom-claude ','claude-effort':'medium','codex-model':'','codex-effort':'','design-backend':'claude','design-model':'design-model','design-effort':'high'});
  assert.equal(changed.ai.default_backend,'codex');
  assert.equal(changed.ai.backends.claude.model,'custom-claude');
  assert.equal(changed.ai.backends.codex.model,null);
  assert.equal(changed.ai.backends.codex.profile,'trusted-profile');
  assert.equal(changed.ai.backends.claude.executable,'/trusted/claude');
  assert.deepEqual(changed.ai.roles.design,{backend:'claude',model:'design-model',effort:'high'});
  assert.equal(changed.ai.timeout_seconds,180);assert.equal(changed.documents.language,'ko');
  assert.equal(settings.ai.default_backend,'claude');
});

test('request time limits accept whole seconds within bounds and preserve omitted legacy form values',()=>{
  const original=structuredClone(settings);
  for(const seconds of ['10','300','601','1800','3600']) {
    const changed=settingsFromValues(settings,{'default-backend':'claude','timeout-seconds':seconds});
    assert.equal(changed.ai.timeout_seconds,Number(seconds));
    assert.equal(changed.ai.backends.codex.executable,settings.ai.backends.codex.executable);
    assert.equal(changed.ai.backends.codex.profile,settings.ai.backends.codex.profile);
  }
  assert.equal(settingsFromValues(settings,{'default-backend':'claude'}).ai.timeout_seconds,180);
  for(const seconds of ['', ' ', '9', '3601', '-10', '10.5', 'NaN', 'Infinity', 'not-a-number']) {
    assert.throws(()=>settingsFromValues(settings,{'timeout-seconds':seconds}),error=>error instanceof ApiError&&error.code==='INVALID_SETTINGS'&&/10~3600.*정수/.test(error.message),seconds);
  }
  assert.deepEqual(settings,original,'settings changes must not alter an active run or its source settings');
});

test('output directory client validation rejects traversal and absolute paths',()=>{
  for(const path of ['/tmp/result','../result','docs/../result','C:/result','\\server\\result','docs\\result','~/result','docs//result','docs/./result','docs/'])assert.equal(safeRelativeDirectory(path),false,path);
  assert.equal(safeRelativeDirectory('docs/architecture/서비스-개정-2'),true);
});

class Element {
  constructor(id,doc){this.id=id;this.doc=doc;this.innerHTML='';this.textContent='';this.value='';this.dataset={};this.open=false;this.disabled=false;this.attributes={};this.lastElementChild={textContent:''};this.listeners=new Map();this.classList={toggle(){return true;}};this.selectionStart=0;this.selectionEnd=0;}
  setAttribute(key,value){this.attributes[key]=value;}
  addEventListener(name,fn){this.listeners.set(name,fn);}
  querySelectorAll(){return [];}
  showModal(){this.open=true;}
  close(){this.open=false;}
  focus(){this.doc.activeElement=this;}
  setSelectionRange(start,end){this.selectionStart=start;this.selectionEnd=end;}
  reportValidity(){return true;}
  setCustomValidity(message){this.validationMessage=message;}
  closest(){return this;}
  matches(){return false;}
}
function harness(fixture,{storageEntries=[]}={}){
  const elements=new Map(),listeners=new Map(),winListeners=new Map(),requests=[],streams=[],reloads=[];
  const doc={activeElement:null,body:{classList:{toggle(){return true;}}},getElementById(id){if(!elements.has(id))elements.set(id,new Element(id,doc));return elements.get(id);},addEventListener(name,fn){listeners.set(name,fn);}};
  const storage=new Map(storageEntries);
  const win={location:{hash:'#token=private-test-capability',pathname:'/',search:'',reload(){reloads.push(true);}},history:{replaceState(){}},sessionStorage:{setItem(k,v){storage.set(k,v);},getItem(k){return storage.get(k)||null;},removeItem(k){storage.delete(k);}},crypto:webcrypto,setTimeout,clearTimeout,addEventListener(name,fn){winListeners.set(name,fn);},fetch:async(path,options)=>{
    requests.push({path,options,body:options.body?JSON.parse(options.body):null});
    if(path.includes('/events'))return new Response(new ReadableStream({start(controller){streams.push(controller);}}),{headers:{'content-type':'text/event-stream'}});
    const body=await fixture(path,options,options.body?JSON.parse(options.body):null);
    if(body instanceof Response)return body;
    return new Response(JSON.stringify(body),{headers:{'content-type':'application/json'}});
  }};
  const app=startWorkspace(doc,win);
  const emit=(name,target)=>listeners.get(name)?.({target,preventDefault(){}});
  const click=(action,data={})=>{const el=new Element('button',doc);el.dataset={action,...data};emit('click',el);};
  return {app,doc,win,requests,streams,reloads,emit,emitWindow:name=>winListeners.get(name)?.(),click,dispose(){winListeners.get('pagehide')?.();}};
}
async function eventually(predicate){for(let n=0;n<100;n++){if(predicate())return;await new Promise(resolve=>setTimeout(resolve,5));}assert.fail('expected UI state was not reached');}
function sessionFixture(){return {id:'session-1',title:'실제 프로젝트',revision:4,design_revision:2,messages:[{id:'m1',role:'user',content:'<img src=x onerror=alert(1)>',created_at:'2026-11-17T11:00:00Z'},{id:'m2',role:'assistant',content:'실제 CLI에서 확정된 질문',created_at:'2026-11-17T11:00:01Z'}],drafts:[],claims:[],coverage:[],requirements:[],decisions:[],findings:[],questions:[],content_status:'draft',active_run_id:null,cursor:7};}

test('live workspace uses server snapshots and safe text, without a generated example draft',async()=>{
  const session=sessionFixture();
  const run={id:'run-1',session_id:session.id,kind:'conversation',status:'completed',process_termination:'confirmed',effective_settings:{}};
  const workspace={project_name:'<script>project</script>',root_display:'/project',sessions:[session],runs:[run],active_run:null,exports:[],workspace_revision:1,context_files:[]};
  const h=harness(async path=>path==='/api/v1/workspace'?workspace:path.includes('/runs/')?run:session);
  try{
    await eventually(()=>h.app.state.scene==='recovery');
    h.click('interview');
    const html=h.doc.getElementById('workspace-content').innerHTML;
    assert.match(html,/실제 CLI에서 확정된 질문/);assert.match(html,/&lt;img src=x onerror=alert\(1\)&gt;/);assert.doesNotMatch(html,/<img/);
    assert.match(h.doc.getElementById('aux-content').innerHTML,/아직 정리된 메모/);
    h.emit('click',Object.assign(new Element('documents-tab',h.doc),{dataset:{panel:'documents'}}));
    assert.match(h.doc.getElementById('aux-content').innerHTML,/미생성/);
    assert.doesNotMatch(html,/회원 알림 서비스|2천 명|2\.2초/);
    assert.ok(h.requests.every(call=>call.options.headers.get('Authorization')==='Bearer private-test-capability'));
  }finally{h.dispose();}
});

test('create/generate/cancel commands use UUID/revisions; cancellation waits for confirmed server state',async()=>{
  let session=null,run=null;
  const workspace=()=>({project_name:'프로젝트',root_display:'/project',sessions:session?[session]:[],runs:run?[run]:[],active_run:run&&['running','cancelling'].includes(run.status)?run:null,exports:[],workspace_revision:3,context_files:[]});
  const h=harness(async(path,options,body)=>{
    if(path==='/api/v1/workspace')return workspace();
    if(path==='/api/v1/sessions'&&options.method==='POST'){
      assert.equal(body.expected_revision,3);assert.deepEqual(body.payload.context_paths,[]);
      session=sessionFixture();session.active_run_id='run-1';
      run={id:'run-1',session_id:session.id,kind:'conversation',status:'running',process_termination:'pending',effective_settings:{model:'actual-model'}};
      return {session,run};
    }
    if(path.endsWith('/cancel')){run={...run,status:'cancelling'};return {session,run};}
    if(path.includes('/runs/'))return run;
    return session;
  });
  try{
    await eventually(()=>h.app.state.scene==='welcome');
    h.doc.getElementById('project-intro').value='사용자 프로젝트 소개';
    h.emit('submit',h.doc.getElementById('start-form'));
    await eventually(()=>h.app.state.scene==='generating'&&!h.app.state.busy);
    const create=h.requests.find(call=>call.path==='/api/v1/sessions');
    assert.match(create.body.command_id,/^[\da-f-]{36}$/);assert.equal(create.body.payload.intro,'사용자 프로젝트 소개');
    h.click('cancel');await eventually(()=>h.app.state.run.status==='cancelling'&&!h.app.state.busy);
    assert.equal(h.app.state.scene,'generating');assert.match(h.doc.getElementById('workspace-content').innerHTML,/종료를 확인/);
    const cancel=h.requests.find(call=>call.path.endsWith('/cancel'));assert.equal(cancel.body.expected_revision,4);
    run={...run,status:'cancelled',process_termination:'confirmed'};session.active_run_id=null;
    await h.app.refresh({keep:false});assert.equal(h.app.state.scene,'paused');
    assert.equal(h.requests.filter(call=>call.path==='/api/v1/sessions').length,1);
  }finally{h.dispose();}
});

test('stale drafts keep immutable text and cannot be approved as the current version',async()=>{
  const session=sessionFixture();session.design_revision=8;
  session.drafts=[{id:'d1',input_revision:7,markdown:'# 이전 초안\n사용자 2000명\n<script>alert(1)</script>',reviewed:true,content_status:'draft'}];
  session.readiness_issues=['이전 설계 기록입니다.'];
  const workspace={project_name:'프로젝트',sessions:[session],runs:[],exports:[],active_run:null,context_files:[]};
  const h=harness(async path=>path==='/api/v1/workspace'?workspace:session);
  try{
    await eventually(()=>h.app.state.scene==='recovery');h.click('review');
    const html=h.doc.getElementById('workspace-content').innerHTML;
    assert.match(html,/이전 대화 기준 초안/);assert.match(html,/사용자 2000명/);assert.match(html,/&lt;script&gt;alert\(1\)&lt;\/script&gt;/);assert.doesNotMatch(html,/<script>/);
    assert.match(html,/data-action="approve" disabled/);
    assert.match(html,/data-action="save-preview"/);
    assert.match(html,/이전 설계 기록입니다/);
  }finally{h.dispose();}
});

test('export conflict is never shown as saved; confirmed paths are displayed only on completed result',async()=>{
  const session=sessionFixture();session.drafts=[{id:'d1',input_revision:2,markdown:'# 실제 초안',reviewed:true,content_status:'draft'}];session.readiness_issues=[];
  const plan={id:'export-1',document_revision:2,relative_directory:'docs/architecture/프로젝트',files:[{relative_path:'아키텍처-설계.md',content:'# 실제 초안',hash:'hash'}]};
  let outcome='conflict';
  const h=harness(async(path,options,body)=>{
    if(path==='/api/v1/workspace')return {project_name:'프로젝트',sessions:[session],runs:[],exports:[],context_files:[],active_run:null};
    if(path.endsWith('/exports')&&body.payload.mode==='preview')return {export:{id:plan.id,status:'preview'},plan};
    if(path.endsWith('/exports')&&body.payload.mode==='apply')return {session,export:{id:plan.id,status:outcome,result:{status:outcome,error:outcome==='conflict'?'외부에서 변경된 파일이 있어요.':null,paths:outcome==='completed'?['docs/architecture/프로젝트/아키텍처-설계.md']:[]}}};
    return session;
  });
  try{
    await eventually(()=>h.app.state.scene==='recovery');h.click('review');h.click('save-preview');
    await eventually(()=>h.app.state.plan&&!h.app.state.busy);
    assert.match(h.doc.getElementById('scene-dialog').innerHTML,/docs\/architecture\/프로젝트\/아키텍처-설계.md/);
    h.click('export-apply');await eventually(()=>!h.app.state.busy&&h.app.state.modalError);
    assert.equal(h.app.state.scene,'review');assert.equal(h.app.state.saved,null);assert.match(h.app.state.modalError,/외부에서 변경/);
    outcome='completed';h.app.state.plan=plan;h.click('export-apply');
    await eventually(()=>h.app.state.scene==='saved'&&!h.app.state.busy);
    assert.match(h.doc.getElementById('workspace-content').innerHTML,/docs\/architecture\/프로젝트\/아키텍처-설계.md/);
  }finally{h.dispose();}
});

test('durable SSE cursor reconciles snapshots; disconnect recovers state without starting another run',async()=>{
  let session=sessionFixture();
  const h=harness(async path=>path==='/api/v1/workspace'?{project_name:'프로젝트',sessions:[session],runs:[],exports:[],context_files:[],active_run:null}:session);
  try{
    await eventually(()=>h.app.state.scene==='recovery'&&h.streams.length>0);
    assert.match(h.requests.find(call=>call.path.includes('/events')).path,/after=7$/);
    h.click('interview');
    session={...session,revision:5,cursor:8,messages:[...session.messages,{id:'m3',role:'assistant',content:'서버에 새로 확정된 답변',created_at:'2026-11-17T12:00:00Z'}]};
    h.streams[0].enqueue(encode.encode('id: 8\r\nevent: state\r\ndata: {"event_id":8,"session_id":"session-1","type":"state","payload":{"kind":"run_completed"}}\r\n\r\n'));
    await eventually(()=>h.app.state.session.revision===5);
    assert.equal(h.app.state.cursor,'8');
    assert.match(h.doc.getElementById('workspace-content').innerHTML,/새로 확정된 답변/);
    session={...session,revision:6,cursor:9};h.streams[0].close();
    await eventually(()=>h.app.state.session.revision===6&&h.app.state.connection==='polling');
    assert.equal(h.requests.filter(call=>call.options.method!=='GET').length,0);
    assert.equal(h.app.state.scene,'interview');
  }finally{h.dispose();}
});

test('unconfirmed process termination blocks new AI work and presents explicit manual recovery',async()=>{
  const session=sessionFixture();session.active_run_id='run-unknown';
  const run={id:'run-unknown',session_id:session.id,kind:'design',status:'interrupted',process_termination:'unknown',effective_settings:{}};
  const h=harness(async path=>path==='/api/v1/workspace'?{project_name:'프로젝트',sessions:[session],runs:[run],active_run:run,exports:[],context_files:[]}:path.includes('/runs/')?run:session);
  try{
    await eventually(()=>h.app.state.scene==='recovery');
    assert.match(h.doc.getElementById('workspace-content').innerHTML,/이전 작업의 로컬 도구를 직접 종료/);
    h.click('interview');
    assert.match(h.doc.getElementById('workspace-content').innerHTML,/data-action="generate" disabled/);
    h.click('new-session');
    assert.match(h.doc.getElementById('workspace-content').innerHTML,/<button type="submit" class="primary" disabled>/);
    assert.equal(h.requests.filter(call=>call.options.method!=='GET').length,0);
  }finally{h.dispose();}
});

test('lost apply response checks the exact export record before reporting confirmed save',async()=>{
  const session=sessionFixture();session.drafts=[{id:'d1',input_revision:2,markdown:'# 실제 초안',reviewed:true,content_status:'draft'}];session.readiness_issues=[];
  const plan={id:'export-lost',document_revision:2,relative_directory:'docs/architecture/프로젝트',files:[{relative_path:'아키텍처-설계.md',content:'# 실제 초안',hash:'hash'}]};
  const entry={id:plan.id,session_id:session.id,status:'completed',result:{status:'completed',paths:['docs/architecture/프로젝트/아키텍처-설계.md']}};
  let applied=false;
  const h=harness(async(path,options,body)=>{
    if(path==='/api/v1/workspace')return {project_name:'프로젝트',sessions:[session],runs:[],exports:applied?[entry]:[],context_files:[],active_run:null};
    if(path.endsWith('/exports')&&body.payload.mode==='preview')return {export:{id:plan.id,status:'preview'},plan};
    if(path.endsWith('/exports')&&body.payload.mode==='apply'){applied=true;throw new TypeError('network response lost');}
    if(path.endsWith('/exports/export-lost'))return entry;
    return session;
  });
  try{
    await eventually(()=>h.app.state.scene==='recovery');h.click('review');h.click('save-preview');
    await eventually(()=>h.app.state.plan&&!h.app.state.busy);h.click('export-apply');
    await eventually(()=>h.app.state.scene==='saved'&&!h.app.state.busy);
    assert.equal(h.requests.filter(call=>call.body?.payload.mode==='apply').length,1);
    assert.equal(h.requests.filter(call=>call.path.endsWith('/exports/export-lost')).length,1);
  }finally{h.dispose();}
});

test('a diagnostic with no session can be manually recovered using the workspace revision',async()=>{
  let active={id:'diagnostic-1',session_id:'',kind:'diagnostic',status:'interrupted',process_termination:'unknown',effective_settings:{kind:'help_only'}};
  let revision=11;
  const h=harness(async(path,options,body)=>{
    if(path==='/api/v1/workspace')return {project_name:'빈 프로젝트',sessions:[],runs:[active],active_run:active,exports:[],context_files:[],workspace_revision:revision};
    if(path.endsWith('/recover')){
      assert.equal(body.expected_revision,11);assert.deepEqual(body.payload,{confirmed:true});
      active=null;revision++;return {run:{id:'diagnostic-1',kind:'diagnostic',status:'interrupted',process_termination:'manually_confirmed'},session:null};
    }
    assert.fail(`unexpected request ${path}`);
  });
  try{
    await eventually(()=>h.app.state.scene==='diagnostic');
    assert.match(h.doc.getElementById('workspace-content').innerHTML,/진단에서 시작한 로컬 도구를 직접 종료/);
    h.click('confirm-diagnostic-termination');
    assert.equal(h.requests.filter(call=>call.path.endsWith('/recover')).length,0);
    h.doc.getElementById('diagnostic-termination-confirm').checked=true;
    h.click('confirm-diagnostic-termination');
    await eventually(()=>h.app.state.scene==='welcome'&&!h.app.state.busy);
    assert.equal(h.requests.filter(call=>call.path.endsWith('/recover')).length,1);
    assert.equal(h.requests.filter(call=>call.path==='/api/v1/sessions').length,0);
  }finally{h.dispose();}
});

test('a running diagnostic with no session blocks new AI work and polls only snapshots',async()=>{
  const active={id:'diagnostic-1',session_id:'',kind:'diagnostic',status:'running',process_termination:'pending'};
  const h=harness(async()=>({project_name:'빈 프로젝트',sessions:[],runs:[active],active_run:active,exports:[],context_files:[],workspace_revision:1}));
  try{
    await eventually(()=>h.app.state.scene==='diagnostic');
    assert.match(h.doc.getElementById('workspace-content').innerHTML,/모델을 호출하지 않아요/);
    assert.doesNotMatch(h.doc.getElementById('workspace-content').innerHTML,/data-action="cancel"/);
    assert.ok(h.app.state.diagnosticTimer);
    h.click('new-session');
    assert.match(h.doc.getElementById('workspace-content').innerHTML,/<button type="submit" class="primary" disabled>/);
    assert.match(h.doc.getElementById('notice').innerHTML,/data-action="diagnostic-status"/);
    h.click('diagnostic-status');assert.equal(h.app.state.scene,'diagnostic');
    assert.equal(h.requests.filter(call=>call.options.method!=='GET').length,0);
  }finally{h.dispose();}
});

test('diagnostics application errors are visible and always reconcile the workspace process gate',async()=>{
  let active=null;
  const h=harness(async(path,options)=>{
    if(path==='/api/v1/workspace')return {project_name:'빈 프로젝트',sessions:[],runs:active?[active]:[],active_run:active,exports:[],context_files:[],workspace_revision:active?2:1};
    if(path==='/api/v1/settings'&&options.method==='GET')return {settings,revision:0,diagnostics:[]};
    if(path==='/api/v1/settings/diagnostics'){
      active={id:'diagnostic-unknown',session_id:'',kind:'diagnostic',status:'interrupted',process_termination:'unknown'};
      return {diagnostics:[],error:{code:'process_termination_unknown',message:'이전 프로세스를 정리한 뒤 확인해 주세요.'}};
    }
    assert.fail(`unexpected request ${path}`);
  });
  try{
    await eventually(()=>h.app.state.scene==='welcome');h.click('settings');
    await eventually(()=>h.app.state.modal==='settings'&&!h.app.state.busy);
    const before=h.requests.filter(call=>call.path==='/api/v1/workspace').length;
    h.click('diagnose');await eventually(()=>!h.app.state.busy&&h.app.state.modalError);
    assert.match(h.app.state.modalError,/이전 프로세스/);
    assert.ok(h.requests.filter(call=>call.path==='/api/v1/workspace').length>before);
    assert.equal(h.app.state.scene,'diagnostic');h.click('close-modal');
    assert.match(h.doc.getElementById('workspace-content').innerHTML,/수동 종료 확인/);
  }finally{h.dispose();}
});

test('an in-flight pre-command snapshot cannot replace an accepted newer generation',async()=>{
  let session=sessionFixture();
  const previous={id:'conversation-old',session_id:session.id,kind:'conversation',status:'completed',process_termination:'confirmed'};
  let run=previous,defer=false,release;
  const h=harness(async(path,options)=>{
    if(path==='/api/v1/workspace')return structuredClone({project_name:'프로젝트',sessions:[session],runs:run===previous?[previous]:[previous,run],exports:[],context_files:[],active_run:run.status==='running'?run:null});
    if(path.endsWith('/draft')&&options.method==='POST'){
      session={...session,revision:5,active_run_id:'design-new'};
      run={id:'design-new',session_id:session.id,kind:'design',status:'running',process_termination:'pending'};
      return structuredClone({session,run});
    }
    if(path.endsWith('/runs/conversation-old'))return previous;
    if(path.endsWith('/runs/design-new'))return run;
    if(defer){defer=false;const captured=structuredClone(session);return new Promise(resolve=>{release=()=>resolve(captured);});}
    return structuredClone(session);
  });
  try{
    await eventually(()=>h.app.state.scene==='recovery');h.click('interview');
    defer=true;const oldRefresh=h.app.refresh({keep:true});await eventually(()=>release);
    h.click('generate');await eventually(()=>h.app.state.run?.id==='design-new'&&!h.app.state.busy);
    release();await oldRefresh;
    assert.equal(h.app.state.scene,'generating');assert.equal(h.app.state.session.revision,5);
  }finally{h.dispose();}
});

test('completion between session and run GETs cannot leave a finished draft in the interview scene',async()=>{
  let session=sessionFixture();
  const previous={id:'conversation-old',session_id:session.id,kind:'conversation',status:'completed',process_termination:'confirmed'};
  let run=previous,finishOnRead=false;
  const h=harness(async(path,options)=>{
    if(path==='/api/v1/workspace')return structuredClone({project_name:'프로젝트',sessions:[session],runs:run===previous?[previous]:[previous,run],exports:[],context_files:[],active_run:run.status==='running'?run:null});
    if(path.endsWith('/draft')&&options.method==='POST'){
      session={...session,revision:5,active_run_id:'design-new'};
      run={id:'design-new',session_id:session.id,kind:'design',status:'running',process_termination:'pending'};
      return structuredClone({session,run});
    }
    if(path.endsWith('/runs/conversation-old'))return previous;
    if(path.endsWith('/runs/design-new')){
      if(finishOnRead){finishOnRead=false;run={...run,status:'completed',process_termination:'confirmed'};session={...session,revision:7,active_run_id:null,drafts:[{id:'draft-new',input_revision:2,markdown:'# 완성된 설계',reviewed:true,content_status:'draft'}]};}
      return structuredClone(run);
    }
    return structuredClone(session);
  });
  try{
    await eventually(()=>h.app.state.scene==='recovery');h.click('interview');h.click('generate');
    await eventually(()=>h.app.state.scene==='generating'&&!h.app.state.busy);
    finishOnRead=true;await h.app.refresh({keep:true});await h.app.refresh({keep:true});
    assert.equal(h.app.state.scene,'review');assert.match(h.doc.getElementById('workspace-content').innerHTML,/draft-picker/);
  }finally{h.dispose();}
});


test('authoritative empty workspace drops the previous project view and stream',async()=>{
  const session=sessionFixture();
  let workspace={project_name:'before',sessions:[session],runs:[],exports:[],context_files:[],active_run:null};
  const h=harness(async path=>path==='/api/v1/workspace'?workspace:session);
  try{
    await eventually(()=>h.app.state.scene==='recovery');h.click('interview');
    Object.assign(h.app.state,{settings:{ai:{model:'shared preference'}},selectedDraft:'old-draft',selectedDocument:'decisions',saved:{old:true},lastExport:{old:true},plan:{old:true},live:'old delta',answer:'old answer'});
    workspace={...workspace,project_name:'fresh',sessions:[],runs:[],exports:[]};
    await h.app.refresh({keep:true});
    assert.equal(h.app.state.session,null);assert.equal(h.app.state.run,null);assert.equal(h.app.state.feed,null);
    for(const field of ['selectedDraft','saved','lastExport','plan'])assert.equal(h.app.state[field],null,field);
    for(const field of ['live','answer','cursor'])assert.equal(h.app.state[field],'',field);
    assert.equal(h.app.state.settings.ai.model,'shared preference');
    assert.equal(h.app.state.selectedDocument,'architecture');assert.equal(h.app.state.scene,'welcome');
    assert.doesNotMatch(h.doc.getElementById('workspace-content').innerHTML,/실제 CLI에서 확정된 질문/);
    assert.equal(h.win.sessionStorage.getItem('architalk.session'),null);
    assert.equal(h.requests.filter(call=>call.options.method!=='GET').length,0);
  }finally{h.dispose();}
});

test('terminal reset SSE clears project data and ignores late commands and deltas',async()=>{
  const session=sessionFixture();let release;
  const h=harness(async(path,options)=>{
    if(path==='/api/v1/workspace')return {project_name:'before',sessions:[session],runs:[],exports:[],context_files:[],active_run:null};
    if(path.endsWith('/draft')&&options.method==='POST')return new Promise(resolve=>{release=()=>resolve({session:{...session,revision:5,drafts:[{id:'late',markdown:'# late draft',input_revision:2}]}});});
    return session;
  });
  try{
    await eventually(()=>h.app.state.scene==='recovery'&&h.streams.length);h.click('interview');h.click('generate');await eventually(()=>release);
    h.streams[0].enqueue(encode.encode('event: workspace_reset_required\ndata: {"type":"workspace_reset_required","error":{"code":"workspace_reset_required","message":"합성 초기화"}}\n\nevent: delta\ndata: {"type":"delta","payload":{"text":"late old delta"}}\n\n'));
    await eventually(()=>h.app.state.connectionBlocked===true);
    release();await eventually(()=>!h.app.state.busy);
    assert.equal(h.app.state.workspace,null);assert.equal(h.app.state.session,null);assert.equal(h.app.state.run,null);assert.equal(h.app.state.feed,null);
    assert.equal(h.app.state.live,'');assert.equal(h.app.state.cursor,'');
    assert.equal(h.app.state.connection,'disconnected');assert.equal(h.doc.getElementById('connection').lastElementChild.textContent,'새 접속 필요');
    assert.equal(h.app.state.refreshTimer,null);assert.equal(h.app.state.diagnosticTimer,null);
    const count=h.requests.length;h.click('generate');h.click('new-session');await h.app.refresh();
    assert.equal(h.requests.length,count);assert.equal(h.app.state.session,null);
    const html=h.doc.getElementById('workspace-content').innerHTML;
    assert.match(html,/서버|새 접속/);assert.doesNotMatch(html,/실제 CLI에서 확정된 질문|late draft|late old delta/);
    assert.equal(h.win.sessionStorage.getItem('architalk.authorization'),null);
  }finally{release?.();h.dispose();}
});

test('last-view selection stays in memory instead of reusing an old browser session ID',async()=>{
  const first=sessionFixture(),last={...sessionFixture(),id:'session-2',title:'last server session'};
  const h=harness(async path=>path==='/api/v1/workspace'?{project_name:'project',sessions:[first,last],runs:[],exports:[],context_files:[],active_run:null}:path.endsWith('/session-1')?first:last,{storageEntries:[['architalk.session','session-1']]});
  try{
    await eventually(()=>h.app.state.scene==='recovery');assert.equal(h.app.state.session.id,'session-2');
    assert.equal(h.win.sessionStorage.getItem('architalk.session'),null);
    h.click('',{session:'session-1'});await eventually(()=>h.app.state.session.id==='session-1'&&!h.app.state.busy);
    assert.equal(h.win.sessionStorage.getItem('architalk.session'),null);
    assert.equal(h.win.sessionStorage.getItem('architalk.authorization'),'private-test-capability');
  }finally{h.dispose();}
});

test('revoked connection clears old content and stops further refresh requests',async()=>{
  const session=sessionFixture();let revoked=false;
  const h=harness(async path=>{
    if(path==='/api/v1/workspace')return revoked?new Response(JSON.stringify({error:{code:'unauthorized',message:'old capability expired'}}),{status:401}):{project_name:'before',sessions:[session],runs:[],exports:[],context_files:[],active_run:null};
    return session;
  });
  try{
    await eventually(()=>h.app.state.scene==='recovery');revoked=true;
    await assert.rejects(h.app.refresh(),error=>error.status===401);
    assert.equal(h.app.state.connectionBlocked,true);assert.equal(h.app.state.session,null);assert.equal(h.app.state.feed,null);
    assert.equal(h.win.sessionStorage.getItem('architalk.authorization'),null);
    const count=h.requests.length;await h.app.refresh();assert.equal(h.requests.length,count);
    assert.doesNotMatch(h.doc.getElementById('workspace-content').innerHTML,/실제 CLI에서 확정된 질문/);
  }finally{h.dispose();}
});


test('a new capability fragment reboots the same-origin connection but ordinary anchors do not',async()=>{
  const session=sessionFixture();
  const h=harness(async path=>path==='/api/v1/workspace'?{project_name:'before',sessions:[session],runs:[],exports:[],context_files:[],active_run:null}:session);
  try{
    await eventually(()=>h.app.state.scene==='recovery');
    h.win.location.hash='#ordinary-document-anchor';h.emitWindow('hashchange');assert.equal(h.reloads.length,0);assert.equal(h.app.state.session.id,session.id);
    h.win.location.hash='#token=new-synthetic-capability';h.emitWindow('hashchange');
    assert.equal(h.reloads.length,1);assert.equal(h.app.state.session,null);assert.equal(h.app.state.feed,null);assert.equal(h.app.state.connectionBlocked,true);
    assert.equal(h.win.sessionStorage.getItem('architalk.authorization'),null);
    assert.doesNotMatch(h.doc.getElementById('workspace-content').innerHTML,/실제 CLI에서 확정된 질문/);
  }finally{h.dispose();}
});

test('failed conversation offers explicit retry in recovery and error without duplicating user input',async()=>{
  let session=sessionFixture(),run={id:'failed-conversation',session_id:session.id,kind:'conversation',status:'failed',process_termination:'confirmed',error:{code:'invalid_model_output',message:'questions[0].options: observed 5 items; limit 4 items'}};
  const original=structuredClone(session.messages);let release;
  const workspace=()=>({project_name:'retry project',sessions:[session],runs:[run],active_run:run.status==='running'?run:null,exports:[],context_files:[]});
  const h=harness(async(path,options,body)=>{
    if(path==='/api/v1/workspace')return workspace();
    if(path.endsWith('/conversation/retry')){
      assert.equal(options.method,'POST');assert.equal(body.expected_revision,4);assert.deepEqual(body.payload,{});assert.match(body.command_id,/^[\da-f-]{36}$/);
      return new Promise(resolve=>{release=()=>{run={...run,id:'retry-run',status:'running',process_termination:'pending',error:null};session={...session,revision:5,active_run_id:run.id};resolve({session,run});};});
    }
    return path.includes('/runs/')?run:session;
  });
  try{
    await eventually(()=>h.app.state.scene==='recovery');
    assert.match(h.doc.getElementById('workspace-content').innerHTML,/class="primary" data-action="retry-conversation"[^>]*>답변 다시 받기/);
    await h.app.refresh({keep:false});assert.equal(h.app.state.scene,'error');
    assert.match(h.doc.getElementById('workspace-content').innerHTML,/class="primary" data-action="retry-conversation"[^>]*>답변 다시 받기/);
    assert.match(h.doc.getElementById('workspace-content').innerHTML,/대화로 돌아가기/);
    assert.match(h.doc.getElementById('workspace-content').innerHTML,/현재 답변으로 다시 정리/);
    h.app.state.answer='아직 보내지 않은 입력';h.click('retry-conversation');h.click('retry-conversation');
    await eventually(()=>release);assert.equal(h.requests.filter(r=>r.path.endsWith('/conversation/retry')).length,1);
    release();await eventually(()=>h.app.state.scene==='generating'&&!h.app.state.busy);
    assert.deepEqual(h.app.state.session.messages,original);assert.equal(h.app.state.answer,'아직 보내지 않은 입력');
    assert.match(h.doc.getElementById('workspace-content').innerHTML,/pending-turn/);
    assert.match(h.doc.getElementById('workspace-content').innerHTML,/실제 CLI에서 확정된 질문/);
    assert.equal(h.requests.filter(r=>r.path.endsWith('/messages')||r.path.endsWith('/draft')).length,0);
  }finally{h.dispose();}
});

test('conversation retry respects latest kind, status, and confirmed termination gate',async()=>{
  for(const [kind,status,termination,eligible] of [
    ['conversation','failed','confirmed',true],['conversation','cancelled','confirmed',true],['conversation','interrupted','manually_confirmed',true],
    ['conversation','completed','confirmed',false],['design','failed','confirmed',false],['critic','failed','confirmed',false],
    ['conversation','interrupted','unknown',false],['conversation','failed','pending',false]
  ]){
    const session=sessionFixture(),run={id:'latest-run',session_id:session.id,kind,status,process_termination:termination};
    const h=harness(async path=>path==='/api/v1/workspace'?{project_name:'gate',sessions:[session],runs:[run],active_run:termination==='unknown'?run:null,exports:[],context_files:[]}:path.includes('/runs/')?run:session);
    try{
      await eventually(()=>h.app.state.scene==='recovery');
      const html=h.doc.getElementById('workspace-content').innerHTML;
      assert.equal(html.includes('data-action="retry-conversation"'),eligible,`${kind}/${status}/${termination}`);
      if(termination==='unknown')assert.match(html,/수동 종료 확인/);
      if(!eligible){h.click('retry-conversation');assert.equal(h.requests.filter(r=>r.options.method==='POST').length,0);}
    }finally{h.dispose();}
  }
});

test('timeout recovery shows the failed run limit and focuses settings without changing project content',async()=>{
  let session=sessionFixture();session.drafts=[{id:'d1',input_revision:2,markdown:'# Existing draft',documents:[{kind:'state',markdown:'# Existing state'}],reviewed:true,content_status:'draft'}];
  let run={id:'timed-out-design',session_id:session.id,kind:'design',status:'failed',process_termination:'confirmed',effective_settings:{timeout_seconds:300},error:{code:'timeout',message:'AI CLI reached its wall-time limit.'}};
  const current=structuredClone(settings);current.ai.timeout_seconds=1800;
  const workspace=()=>({project_name:'timeout',sessions:[session],runs:[run],active_run:run.status==='running'?run:null,exports:[],context_files:[],settings:current,document_types:[{kind:'architecture',filename:'아키텍처-설계.md',title:'설계'},{kind:'state',filename:'상태도.md',title:'상태도'}]});
  const original=structuredClone(session);
  const h=harness(async(path,options,body)=>{
    if(path==='/api/v1/workspace')return workspace();
    if(path==='/api/v1/settings')return {settings:current,revision:7,diagnostics:[],choices:{}};
    if(path.endsWith('/draft')&&options.method==='POST'){
      assert.deepEqual(body.payload,{});assert.equal(body.expected_revision,4);
      run={...run,id:'new-design',status:'running',process_termination:'pending',effective_settings:{timeout_seconds:1800},error:null};session={...session,revision:5,active_run_id:run.id};return {session,run};
    }
    return path.includes('/runs/')?run:session;
  });
  try{
    await eventually(()=>h.app.state.scene==='recovery');
    let html=h.doc.getElementById('workspace-content').innerHTML;assert.match(html,/300초/);assert.match(html,/시간 제한 조정/);assert.match(html,/초안 다시 생성/);assert.doesNotMatch(html,/1800초/);
    await h.app.refresh({keep:false});assert.equal(h.app.state.scene,'error');
    html=h.doc.getElementById('workspace-content').innerHTML;assert.match(html,/작업 시간이 초과됐어요/);assert.match(html,/300초/);assert.match(html,/<details class="error-detail">/);
    h.app.state.answer='보내지 않은 수정 의견';h.app.state.selectedDraft='d1';h.app.state.selectedDocument='state';
    h.click('timeout-settings');await eventually(()=>h.app.state.modal==='settings'&&!h.app.state.busy);
    assert.equal(h.doc.activeElement.id,'timeout-seconds');assert.match(h.doc.getElementById('scene-dialog').innerHTML,/max="3600"/);assert.match(h.doc.getElementById('scene-dialog').innerHTML,/value="1800"/);
    h.click('close-modal');assert.equal(h.app.state.scene,'error');assert.equal(h.app.state.selectedDocument,'state');assert.equal(h.app.state.selectedDraft,'d1');assert.equal(h.app.state.answer,'보내지 않은 수정 의견');assert.deepEqual(h.app.state.session,original);
    assert.equal(h.requests.filter(request=>request.options.method==='POST').length,0,'opening or cancelling settings must not retry');
    h.click('generate');h.click('generate');await eventually(()=>h.app.state.scene==='generating'&&!h.app.state.busy);
    assert.equal(h.app.state.runScene,'review');assert.equal(h.app.state.selectedDocument,'state');assert.equal(h.app.state.answer,'보내지 않은 수정 의견');assert.deepEqual(h.app.state.session.messages,original.messages);assert.deepEqual(h.app.state.session.drafts,original.drafts);
    html=h.doc.getElementById('workspace-content').innerHTML;assert.match(html,/pending-turn/);assert.match(html,/요청당 시간 제한: 1800초/);assert.match(html,/다음 작업/);
    assert.equal(h.requests.filter(request=>request.path.endsWith('/draft')&&request.options.method==='POST').length,1);assert.equal(h.requests.filter(request=>request.path.endsWith('/messages')||request.path.endsWith('/conversation/retry')).length,0);
  }finally{h.dispose();}
});

test('timeout recovery keeps review and conversation actions distinct and honors unconfirmed termination',async()=>{
  for(const [kind,termination,limit] of [['critic','confirmed',300],['conversation','confirmed',120],['design','unknown',10],['design','confirmed',null]]){
    const session=sessionFixture();session.drafts=[{id:'d1',input_revision:2,markdown:'# Preserved draft',reviewed:true,content_status:'draft'}];
    const run={id:'timeout-run',session_id:session.id,kind,status:'failed',process_termination:termination,effective_settings:limit?{timeout_seconds:limit}:{},error:{code:'timeout',message:'AI CLI reached its wall-time limit.'}};
    const h=harness(async path=>path==='/api/v1/workspace'?{project_name:'timeout kinds',sessions:[session],runs:[run],active_run:termination==='unknown'?run:null,exports:[],context_files:[]}:path.includes('/runs/')?run:session);
    try{
      await eventually(()=>h.app.state.scene==='recovery');
      if(termination==='unknown'){
        const html=h.doc.getElementById('workspace-content').innerHTML;assert.match(html,/수동 종료 확인/);assert.doesNotMatch(html,/>초안 다시 생성</);h.click('generate');assert.equal(h.requests.filter(request=>request.options.method==='POST').length,0);
      }else{
        await h.app.refresh({keep:false});const html=h.doc.getElementById('workspace-content').innerHTML;
        if(kind==='critic'){assert.match(html,/class="primary" data-action="review"[^>]*>완성된 초안 보기/);assert.doesNotMatch(html,/>초안 다시 생성</);}
        if(kind==='conversation'){assert.match(html,/data-action="retry-conversation"/);assert.doesNotMatch(html,/>초안 다시 생성</);}
        if(limit===null)assert.doesNotMatch(html,/300초|1800초/,'a missing run limit must not be inferred from defaults');
      }
    }finally{h.dispose();}
  }
});

test('timeout settings use the new default only when the server value is absent',async()=>{
  const current=structuredClone(settings);delete current.ai.timeout_seconds;
  const h=harness(async path=>path==='/api/v1/workspace'?{project_name:'missing timeout',sessions:[],runs:[],exports:[],context_files:[]}:path==='/api/v1/settings'?{settings:current,revision:0,diagnostics:[]}:null);
  try{
    await eventually(()=>h.app.state.scene==='welcome');h.click('settings');await eventually(()=>h.app.state.modal==='settings'&&!h.app.state.busy);
    assert.match(h.doc.getElementById('scene-dialog').innerHTML,/id="timeout-seconds"[^>]+value="1800"/);
    assert.match(h.doc.getElementById('scene-dialog').innerHTML,/요청마다 제한을 따로/);
    assert.equal(h.requests.filter(request=>request.options.method==='POST'||request.options.method==='PATCH').length,0);
  }finally{h.dispose();}
});
