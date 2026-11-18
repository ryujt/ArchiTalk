import test from 'node:test';
import assert from 'node:assert/strict';
import {createFileTabs,matchCatalogPath} from '../web/file-viewer.js';
import {createDocumentRecorder,buildRecordingInstruction,lineRangeFromOffsets} from '../web/document-recorder.js';
import {pathCopyValues} from '../web/viewer-menu.js';
import {resolveFileTarget} from '../web/document-renderer.js';
import {createApi} from '../web/app.js';

const tick=()=>new Promise(resolve=>setImmediate(resolve));
async function eventually(predicate){for(let n=0;n<100;n++){if(predicate())return;await tick();}assert.fail('state transition did not settle');}
function media(){
  const instances=[],tracks={stops:0,stop(){this.stops++;}},stream={getTracks:()=>[tracks]};
  class Recorder extends EventTarget {
    static isTypeSupported(type){return type.startsWith('audio/webm');}
    constructor(input,options){super();assert.equal(input,stream);this.state='inactive';this.mimeType=options?.mimeType||'audio/webm';instances.push(this);}
    start(){this.state='recording';}
    data(value='synthetic audio'){const event=new Event('dataavailable');event.data=new Blob([value],{type:this.mimeType});this.dispatchEvent(event);}
    stop(){if(this.state==='inactive')return;this.data();this.state='inactive';queueMicrotask(()=>this.dispatchEvent(new Event('stop')));}
  }
  return {Recorder,instances,tracks,stream};
}
const timers={setTimer:()=>1,clearTimer:()=>{}};

test('file tabs reuse open files and ignore closed or cleared late responses',async()=>{
  let resolve,calls=0,signal;const tabs=createFileTabs({readFile:(path,abort)=>{calls++;signal=abort;return new Promise(done=>{resolve=()=>done({path,content:'private fixture',kind:'text'});});}});
  const pending=tabs.open({path:'docs/a.md',name:'a.md'});await tabs.open({path:'docs/a.md',name:'a.md'});assert.equal(calls,1);tabs.close('docs/a.md');assert.equal(signal.aborted,true);resolve();assert.equal(await pending,null);assert.equal(tabs.tabs.size,0);assert.equal(tabs.active,'chat');
  const later=tabs.open({path:'docs/a.md',name:'a.md'});tabs.clear();resolve();await later;assert.equal(tabs.tabs.size,0);assert.equal(tabs.active,'chat');
});

test('out-of-order file reads keep the chosen tab and cache only their own path',async()=>{
  const pending=new Map();let calls=0;const tabs=createFileTabs({readFile:path=>{calls++;return new Promise(resolve=>pending.set(path,()=>resolve({path,kind:'text',content:path})));}});
  const a=tabs.open({path:'a.txt',name:'a.txt'}),b=tabs.open({path:'b.txt',name:'b.txt'});pending.get('b.txt')();await b;pending.get('a.txt')();await a;assert.equal(tabs.active,'b.txt');assert.equal(tabs.tabs.get('a.txt').file.content,'a.txt');await tabs.open({path:'a.txt',name:'a.txt'});assert.equal(calls,2);tabs.activate('chat');assert.equal(tabs.tabs.size,2);tabs.clear();
});

test('catalog and Markdown file links preserve server Unicode and reject unsafe paths',()=>{
  const nfd='문서/요구.md',files=[{path:nfd,name:'요구.md'}];assert.equal(matchCatalogPath('문서/요구.md',files).path,nfd);
  assert.deepEqual(resolveFileTarget('../문서/요구.md#근거','docs/start.md',[nfd]),{path:nfd,anchor:'근거'});
  for(const value of ['../../outside.md','/etc/passwd','file:///etc/passwd','javascript:alert(1)','https://example.invalid/a.md','a.md?secret=1','..\\a.md','%2fetc/passwd'])assert.equal(resolveFileTarget(value,'docs/start.md',[nfd]),null,value);
  assert.equal(resolveFileTarget('../unknown.md','docs/start.md',[nfd]),null);
  assert.deepEqual(pathCopyValues({path:nfd,name:'요구.md'},'/synthetic/project'),{relative:nfd,absolute:`/synthetic/project/${nfd}`,name:'요구.md'});
});

test('audio uploads keep the browser FormData boundary and authorization header',async()=>{
  const body=new FormData();body.append('audio',new Blob(['fake'],{type:'audio/webm'}),'recording');let received;
  const api=createApi('synthetic-token',async(path,options)=>{received={path,options};return new Response(JSON.stringify({text:'fixture'}),{headers:{'content-type':'application/json'}});});
  await api('/api/v1/transcription',{method:'POST',body});assert.equal(received.options.body,body);assert.equal(received.options.headers.get('content-type'),null);assert.equal(received.options.headers.get('authorization'),'Bearer synthetic-token');assert.equal(received.options.redirect,'error');
});

test('a missing transcription key fails before any microphone access',async()=>{
  let microphone=0;const calls=[];const r=createDocumentRecorder({api:async path=>{calls.push(path);return {configured:false};},getContext:()=>({file:'README.md'}),mediaDevices:{getUserMedia:async()=>{microphone++;throw Error('must not run');}},MediaRecorder:class {},...timers});
  await r.start();assert.equal(microphone,0);assert.deepEqual(calls,['/api/v1/transcription/settings']);assert.equal(r.state.phase,'error');assert.match(r.state.error,/OpenAI API 키/);r.clear();
});

test('a microphone granted after reset is stopped without creating a recorder or upload',async()=>{
  const fake=media();let release,calls=0;const r=createDocumentRecorder({api:async()=>{calls++;return {configured:true};},getContext:()=>({file:'README.md'}),mediaDevices:{getUserMedia:()=>new Promise(resolve=>{release=()=>resolve(fake.stream);})},MediaRecorder:fake.Recorder,...timers});
  const pending=r.start();await eventually(()=>release);r.clear();release();await pending;assert.equal(fake.tracks.stops,1);assert.equal(fake.instances.length,0);assert.equal(calls,1);assert.equal(r.state.phase,'idle');
});

test('recording preserves file selection timeline and submits audio only once',async()=>{
  const fake=media();let clock=100,context={file:'docs/a.md',heading:'시작',anchor:'start',source:'file'};const calls=[],results=[];
  const r=createDocumentRecorder({api:async(path,options)=>{calls.push({path,options});return path.endsWith('/settings')?{configured:true,max_audio_bytes:1000,model:'fake-model'}:{text:'선택한 문단을 고쳐 주세요.',model:'fake-model'};},getContext:()=>context,mediaDevices:{getUserMedia:async()=>fake.stream},MediaRecorder:fake.Recorder,now:()=>clock,onResult:result=>results.push(result),...timers});
  await r.start();clock=350;context={file:'docs/b.md',heading:'근거',anchor:'evidence',source:'file'};r.note('open');clock=1450;r.note('selection',context,'선택한 합성 문장');r.stop();r.stop();await eventually(()=>r.state.phase==='complete');
  assert.equal(fake.tracks.stops,1);assert.equal(calls.length,2);assert.equal(calls[1].path,'/api/v1/transcription');assert.deepEqual([...calls[1].options.body.keys()],['audio']);assert.equal(results.length,1);assert.deepEqual(results[0].events.map(event=>[event.t,event.type,event.file]),[[0,'open','docs/a.md'],[250,'open','docs/b.md'],[1350,'selection','docs/b.md']]);assert.match(results[0].instruction,/00:01\.350/);assert.match(results[0].instruction,/근거/);assert.match(results[0].instruction,/선택한 문단을 고쳐 주세요/);r.clear();assert.equal(r.state.result,null);
});

test('oversized recording and late transcription completion cannot leak a result after cleanup',async()=>{
  const fake=media();let posts=0;const r=createDocumentRecorder({api:async path=>{if(!path.endsWith('/settings'))posts++;return {configured:true,max_audio_bytes:5};},getContext:()=>({file:'a.md'}),mediaDevices:{getUserMedia:async()=>fake.stream},MediaRecorder:fake.Recorder,...timers});await r.start();fake.instances[0].data('too-large');await tick();assert.equal(r.state.phase,'error');assert.equal(posts,0);assert.equal(fake.tracks.stops,1);r.clear();
  const next=media();let finish,results=0;const s=createDocumentRecorder({api:async path=>path.endsWith('/settings')?{configured:true}:new Promise(resolve=>{finish=()=>resolve({text:'late private response'});}),getContext:()=>({file:'a.md'}),mediaDevices:{getUserMedia:async()=>next.stream},MediaRecorder:next.Recorder,onResult:()=>results++,...timers});await s.start();s.stop();await eventually(()=>finish);s.clear();finish();await tick();assert.equal(results,0);assert.equal(s.state.result,null);assert.equal(s.state.phase,'idle');
});

test('instruction formatting keeps transcript and document timeline distinct',()=>{
  const result=buildRecordingInstruction('합성 음성',[{t:1200,type:'focus',file:'README.md',heading:'개요',anchor:'개요-1',text:'한 줄\n다음 줄'}]);assert.match(result,/## 음성 지시\n\n합성 음성/);assert.match(result,/\[00:01\.200\] README.md#개요-1 · 제목: 개요/);assert.match(result,/한 줄\n  다음 줄/);
});

test('plain text references retain exact repeated-line selection offsets',()=>{
  const text='same\r\nsame\r\nlast';const location=lineRangeFromOffsets(text,6,12);assert.deepEqual(location,{line:2,lineEnd:2,startOffset:6,endOffset:12});
  const result=buildRecordingInstruction('둘째 줄 수정',[{t:1,type:'selection',file:'repeat.rs',text:'same',...location}]);assert.match(result,/repeat\.rs:2 · selection/);
});
