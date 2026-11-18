import test from 'node:test';
import assert from 'node:assert/strict';
import {createFileTabs,matchCatalogPath} from '../web/file-viewer.js';
import * as recording from '../web/document-recorder.js';
import {pathCopyValues} from '../web/viewer-menu.js';
import {resolveFileTarget} from '../web/document-renderer.js';
import {createApi} from '../web/app.js';
const {createDocumentRecorder,buildRecordingInstruction,buildRecordingTimeline,lineRangeFromOffsets}=recording;

const tick=()=>new Promise(resolve=>setImmediate(resolve));
async function eventually(predicate){for(let n=0;n<100;n++){if(predicate())return;await tick();}assert.fail('state transition did not settle');}
function media({autoStart=true,autoStop=true}={}){
  const instances=[],tracks={stops:0,stop(){this.stops++;}},stream={getTracks:()=>[tracks]};
  class Recorder extends EventTarget {
    static isTypeSupported(type){return type.startsWith('audio/webm');}
    constructor(input,options){super();assert.equal(input,stream);this.state='inactive';this.mimeType=options?.mimeType||'audio/webm';instances.push(this);}
    start(){this.state='recording';if(autoStart)this.started();}
    started(){this.dispatchEvent(new Event('start'));}
    data(value='synthetic audio'){const event=new Event('dataavailable');event.data=new Blob([value],{type:this.mimeType});this.dispatchEvent(event);}
    stop(){if(this.state==='inactive')return;this.data();this.state='inactive';if(autoStop)queueMicrotask(()=>this.stopped());}
    stopped(){this.dispatchEvent(new Event('stop'));}
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

test('timed speech and document actions merge stably across ties, overlaps and file tabs',()=>{
  const response={text:'늦은 발화 겹치는 발화 같은 시작 발화',timing:'segment',segments:[{start:2,end:3.5,text:'늦은 발화'},{start:0.5,end:2.5,text:'겹치는 발화'},{start:0.5,end:1,text:'같은 시작 발화'},{start:0,end:0,text:' \n '}]};
  const events=[{t:500,type:'selection',file:'docs/b.md',source:'draft',heading:'두 번째',anchor:'second-2',line:3,lineEnd:4,startOffset:12,endOffset:34,text:'선택\n문장'},{t:100,type:'open',file:'docs/a.md',source:'file',heading:'첫 번째',anchor:'first',text:''},{t:500,type:'focus',file:'docs/a.md',source:'file',heading:'복귀',anchor:'return',text:'클릭 문장'},{t:700,type:'word',file:'words.txt',text:'단어'},{t:800,type:'copy',file:'copy.txt',text:'복사'},{t:0,type:'unknown',file:'ignored.txt'}];
  const before=structuredClone({response,events});const result=buildRecordingTimeline(response,events);
  assert.equal(result.untimedSpeech,response.text);assert.match(buildRecordingInstruction(response,events),/시간 정보 없는 전사문[\s\S]*늦은 발화 겹치는 발화 같은 시작 발화/);assert.deepEqual(result.timeline.map(item=>item.kind==='speech'?[item.kind,item.startMs,item.endMs,item.text]:[item.kind,item.atMs,item.action,item.file]),[['action',100,'open','docs/a.md'],['speech',500,2500,'겹치는 발화'],['speech',500,1000,'같은 시작 발화'],['action',500,'select','docs/b.md'],['action',500,'click','docs/a.md'],['action',700,'select','words.txt'],['action',800,'copy','copy.txt'],['speech',2000,3500,'늦은 발화']]);
  assert.deepEqual(result.timeline[3],{kind:'action',atMs:500,action:'select',file:'docs/b.md',source:'draft',heading:'두 번째',anchor:'second-2',line:3,lineEnd:4,startOffset:12,endOffset:34,text:'선택\n문장'});assert.deepEqual({response,events},before);
});

test('timing is never invented for explicit untimed or legacy transcription',()=>{
  const events=[{t:1200,type:'focus',file:'README.md',heading:'개요',anchor:'개요-1',text:'한 줄\n다음 줄'}];
  for(const response of ['합성 음성',{text:'합성 음성',model:'legacy'},{text:'합성 음성',timing:'none',segments:[{start:0,end:4,text:'이 시간은 사용하지 않음'}]}]){const merged=buildRecordingTimeline(response,events);assert.equal(merged.untimedSpeech,'합성 음성');assert.equal(merged.timeline.length,1);assert.equal(merged.timeline[0].kind,'action');const instruction=buildRecordingInstruction(response,events);assert.equal((instruction.match(/## 통합 타임라인/g)||[]).length,1);assert.match(instruction,/시간 정보 없는 전사문/);assert.match(instruction,/발화와 문서 조작의 시점을 맞추지 않았/);assert.match(instruction,/합성 음성/);assert.match(instruction,/\[00:01\.200\] click · README.md#개요-1 · 제목: 개요/);assert.match(instruction,/한 줄\n  다음 줄/);}
  assert.equal(buildRecordingTimeline({text:'빈 세그먼트의 원문',timing:'segment',segments:[{text:'  ',start:0,end:1}]}).untimedSpeech,'빈 세그먼트의 원문');
  for(const segments of [undefined,null,[]])assert.equal(buildRecordingTimeline({text:'시간 없는 원문',timing:'segment',segments}).untimedSpeech,'시간 없는 원문');
});

test('partial timed segments keep the complete source transcript available to the clipboard instruction',()=>{
  const response={text:'  첫 발화\n뒤에 빠진 발화. ',timing:'segment',segments:[{start:1,end:2,text:'첫 발화'}]};const result=buildRecordingTimeline(response);assert.equal(result.timeline.length,1);assert.equal(result.untimedSpeech,response.text);const instruction=buildRecordingInstruction(response);assert.match(instruction,/뒤에 빠진 발화\./);assert.match(instruction,/시간 정보 없는 전사문/);assert.match(instruction,/발화와 문서 조작의 시점을 맞추지 않았/);
  const normalized=buildRecordingTimeline({text:'가 나\n다',timing:'segment',segments:[{start:0,end:1,text:'가'},{start:1,end:2,text:'나 다'}]});assert.equal(normalized.untimedSpeech,'');
});

test('speech comparison preserves meaningful word boundaries while normalizing spacing',()=>{
  const response={text:'a part',timing:'segment',segments:[{start:0,end:1,text:'apart'}]};assert.equal(buildRecordingTimeline(response).untimedSpeech,'a part');assert.match(buildRecordingInstruction(response),/시간 정보 없는 전사문[\s\S]*a part/);
  assert.equal(buildRecordingTimeline({text:'a  \n part',timing:'segment',segments:[{start:0,end:1,text:' a '},{start:1,end:2,text:' part '}]}).untimedSpeech,'');
});

test('timed instruction uses one timeline with real speech ranges and action labels',()=>{
  const result=buildRecordingInstruction({text:'이 문장을 바꿔 주세요',timing:'segment',segments:[{start:1.25,end:2.5,text:'이 문장을 바꿔 주세요'}]},[{t:1250,type:'selection',file:'docs/a.md',text:'이 문장'}]);assert.equal((result.match(/## 통합 타임라인/g)||[]).length,1);assert.match(result,/\[00:01\.250–00:02\.500\] 발화 · 이 문장을 바꿔 주세요/);assert.match(result,/\[00:01\.250\] select · docs\/a\.md\n  이 문장/);assert.ok(result.indexOf('발화 ·')<result.indexOf('select ·'));assert.doesNotMatch(result,/## 음성 지시|## 문서 참조 기록|시간 정보 없는 전사문/);
});

test('malformed real segment and action times are rejected without clamping or coercion',()=>{
  const invalidSegments=[{start:-0.001,end:1,text:'음성'},{start:NaN,end:1,text:'음성'},{start:0,end:Infinity,text:'음성'},{start:'0',end:1,text:'음성'},{start:2,end:1,text:'음성'},{start:0,end:Number.MAX_SAFE_INTEGER,text:'음성'},{start:0,end:1,text:42},{start:-1,end:NaN,text:'  '},null];
  for(const segment of invalidSegments)assert.throws(()=>buildRecordingTimeline({text:'원문',timing:'segment',segments:[segment]}),TypeError);
  for(const segments of [{},'bad'])assert.throws(()=>buildRecordingTimeline({text:'원문',timing:'segment',segments}),TypeError);
  for(const t of [-1,NaN,Infinity,'1',1.5,Number.MAX_SAFE_INTEGER+1])assert.throws(()=>buildRecordingTimeline('음성',[{t,type:'open',file:'a.md'}]),TypeError);
  assert.deepEqual(buildRecordingTimeline({text:'',timing:'segment',segments:[{start:0.0005,end:0.0015,text:'경계'}]}).timeline,[{kind:'speech',startMs:1,endMs:2,text:'경계'}]);
});

test('recorder starts its shared clock at the actual start event and freezes stop immediately',async()=>{
  const fake=media({autoStart:false,autoStop:false});let clock=100,timerTick,finish;const outputs=[];let context={file:'pending.md'};
  const r=createDocumentRecorder({api:async path=>path.endsWith('/settings')?{configured:true,model:'fixture-model'}:new Promise(resolve=>{finish=()=>resolve({text:'  원문 전체\n',model:'fixture-model',timing:'segment',segments:[{start:0.5,end:1.25,text:'원문 전체'}]});}),getContext:()=>context,mediaDevices:{getUserMedia:async()=>fake.stream},MediaRecorder:fake.Recorder,now:()=>clock,setTimer:callback=>{timerTick=callback;return 1;},clearTimer:()=>{},onResult:result=>outputs.push(result)});
  const pending=r.start();await eventually(()=>fake.instances.length===1);assert.equal(r.state.phase,'starting');r.note('selection',context,'이른 선택');assert.deepEqual(r.state.events,[]);clock=900;context={file:'actual.md',heading:'실제 시작'};fake.instances[0].started();await pending;assert.equal(r.state.phase,'recording');assert.deepEqual(r.state.events.map(event=>[event.t,event.file]),[[0,'actual.md']]);clock=1400;r.note('focus',context,'후속 클릭');timerTick();assert.equal(r.state.elapsed,500);clock=2150;r.stop();assert.equal(r.state.phase,'stopping');assert.equal(r.state.elapsed,1250);const frozen=structuredClone(r.state.events);clock=10000;timerTick();r.note('copy',context,'늦은 조작');assert.equal(r.state.elapsed,1250);assert.deepEqual(r.state.events,frozen);fake.instances[0].stopped();await eventually(()=>finish);assert.equal(r.state.phase,'uploading');clock=20000;timerTick();assert.equal(r.state.elapsed,1250);finish();await eventually(()=>r.state.phase==='complete');assert.equal(outputs.length,1);assert.equal(outputs[0].text,'  원문 전체\n');assert.equal(outputs[0].timing,'segment');assert.deepEqual(outputs[0].segments,[{start:0.5,end:1.25,text:'원문 전체'}]);assert.deepEqual(outputs[0].events,frozen);assert.deepEqual(outputs[0].timeline.map(item=>item.kind),['action','speech','action']);assert.equal(outputs[0].untimedSpeech,'');assert.equal(fake.tracks.stops,1);r.clear();
});

test('clear invalidates late recorder start and queued stop before any upload',async()=>{
  for(const stage of ['start','stop']){const fake=media({autoStart:stage!=='start',autoStop:false});let posts=0,results=0;const r=createDocumentRecorder({api:async path=>{if(path.endsWith('/settings'))return {configured:true};posts++;return {text:'늦은 전사'};},getContext:()=>({file:'a.md'}),mediaDevices:{getUserMedia:async()=>fake.stream},MediaRecorder:fake.Recorder,onResult:()=>results++,...timers});const pending=r.start();await eventually(()=>fake.instances.length===1);if(stage==='stop'){await pending;r.stop();}r.clear();fake.instances[0].started();fake.instances[0].stopped();await pending;await tick();assert.equal(r.state.phase,'idle');assert.equal(r.state.elapsed,0);assert.deepEqual(r.state.events,[]);assert.equal(r.state.result,null);assert.equal(posts,0);assert.equal(results,0);assert.equal(fake.tracks.stops,1);}
});

test('recording keeps bounded in-memory action metadata and old timer callbacks cannot alter a new epoch',async()=>{
  const fake=media();let clock=0;const ticks=[];const context={file:'bounded.md',heading:'h'.repeat(300),anchor:'a'.repeat(300),line:1,lineEnd:2,startOffset:0,endOffset:20};
  const r=createDocumentRecorder({api:async()=>({configured:true}),getContext:()=>context,mediaDevices:{getUserMedia:async()=>fake.stream},MediaRecorder:fake.Recorder,now:()=>clock,setTimer:callback=>{ticks.push(callback);return ticks.length;},clearTimer:()=>{}});
  await r.start();for(let n=0;n<600;n++)r.note('selection',context,'x'.repeat(2100));assert.equal(r.state.events.length,500);assert.equal(r.state.events[1].text.length,2000);assert.equal(r.state.events[1].heading.length,240);assert.equal(r.state.events[1].anchor.length,240);assert.equal(r.state.events[1].startOffset,0);r.clear();clock=1000;await r.start();clock=1400;ticks[0]();assert.equal(r.state.elapsed,0);ticks[1]();assert.equal(r.state.elapsed,400);r.clear();clock=99999;ticks[1]();assert.equal(r.state.elapsed,0);assert.equal(fake.tracks.stops,2);
});

test('plain text references retain exact repeated-line selection offsets',()=>{
  const text='same\r\nsame\r\nlast';const location=lineRangeFromOffsets(text,6,12);assert.deepEqual(location,{line:2,lineEnd:2,startOffset:6,endOffset:12});
  const result=buildRecordingInstruction('둘째 줄 수정',[{t:1,type:'selection',file:'repeat.rs',text:'same',...location}]);assert.match(result,/select · repeat\.rs:2/);
});
