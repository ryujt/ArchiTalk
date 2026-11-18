export function lineRangeFromOffsets(text,start,end=start) {
  const source=String(text);start=Math.max(0,Math.min(source.length,Number.isSafeInteger(start)?start:0));end=Math.max(start,Math.min(source.length,Number.isSafeInteger(end)?end:start));
  return {line:source.slice(0,start).split(/\r?\n/).length,lineEnd:source.slice(0,end>start?end-1:end).split(/\r?\n/).length,startOffset:start,endOffset:end};
}

const ACTIONS=Object.freeze({open:'open',focus:'click',selection:'select',word:'select',copy:'copy'});
const LOCATION_FIELDS=['line','lineEnd','startOffset','endOffset'];

function secondsToMilliseconds(value) {
  if(typeof value!=='number'||!Number.isFinite(value)||value<0||value>Number.MAX_SAFE_INTEGER/1000)throw new TypeError('전사 구간의 시간 정보가 올바르지 않아요.');
  const milliseconds=Math.round(value*1000);
  if(!Number.isSafeInteger(milliseconds))throw new TypeError('전사 구간의 시간 정보가 올바르지 않아요.');
  return milliseconds;
}

function normalizeTranscript(transcript) {
  const text=typeof transcript==='string'?transcript:typeof transcript?.text==='string'?transcript.text:'';
  const timing=transcript?.timing==='segment'?'segment':'none';
  const segments=[];
  if(timing==='segment'&&transcript.segments!=null){
    if(!Array.isArray(transcript.segments))throw new TypeError('전사 구간 목록이 올바르지 않아요.');
    for(const segment of transcript.segments){
      if(!segment||typeof segment.text!=='string')throw new TypeError('전사 구간의 내용이 올바르지 않아요.');
      secondsToMilliseconds(segment.start);secondsToMilliseconds(segment.end);
      if(segment.end<segment.start)throw new TypeError('전사 구간의 끝 시간이 시작 시간보다 빨라요.');
      if(!segment.text.trim())continue;
      segments.push({start:segment.start,end:segment.end,text:segment.text});
    }
  }
  return {text,segments,timing};
}

export function buildRecordingTimeline(transcript,events=[]) {
  const {text,segments,timing}=normalizeTranscript(transcript);
  if(!Array.isArray(events))throw new TypeError('문서 조작 기록 목록이 올바르지 않아요.');
  const timeline=segments.map(segment=>({kind:'speech',startMs:secondsToMilliseconds(segment.start),endMs:secondsToMilliseconds(segment.end),text:segment.text.trim()}));
  for(const event of events){
    const action=Object.hasOwn(ACTIONS,event?.type)?ACTIONS[event.type]:null;
    if(!action)continue;
    if(!Number.isSafeInteger(event.t)||event.t<0)throw new TypeError('문서 조작 기록의 시간 정보가 올바르지 않아요.');
    const item={kind:'action',atMs:event.t,action,file:event.file,source:event.source||'file',heading:event.heading||'',anchor:event.anchor||'',text:event.text||''};
    for(const field of LOCATION_FIELDS)if(event[field]!==undefined)item[field]=event[field];
    timeline.push(item);
  }
  timeline.sort((a,b)=>(a.kind==='speech'?a.startMs:a.atMs)-(b.kind==='speech'?b.startMs:b.atMs)||(a.kind===b.kind?0:a.kind==='speech'?-1:1));
  const comparable=value=>value.normalize('NFC').replace(/\s+/gu,' ').trim();
  const untimedSpeech=timing!=='segment'||!segments.length||comparable(text)!==comparable(timeline.filter(item=>item.kind==='speech').map(item=>item.text).join(' '))?text:'';
  return {timeline,untimedSpeech};
}

function formatTime(milliseconds) {
  const seconds=Math.floor(milliseconds/1000);
  return `${String(Math.floor(seconds/60)).padStart(2,'0')}:${String(seconds%60).padStart(2,'0')}.${String(milliseconds%1000).padStart(3,'0')}`;
}

export function buildRecordingInstruction(transcript,events=[]) {
  const {timeline,untimedSpeech}=buildRecordingTimeline(transcript,events);
  const lines=timeline.map(item=>{
    if(item.kind==='speech')return `- [${formatTime(item.startMs)}–${formatTime(item.endMs)}] 발화 · ${item.text.replace(/\n/g,'\n  ')}`;
    return `- [${formatTime(item.atMs)}] ${item.action} · ${item.source==='draft'?'[생성 초안] ':''}${item.file}${item.line?`:${item.line}${item.lineEnd>item.line?`-${item.lineEnd}`:''}`:''}${item.anchor?`#${item.anchor}`:''}${item.heading?` · 제목: ${item.heading}`:''}${item.text?`\n  ${item.text.replace(/\n/g,'\n  ')}`:''}`;
  });
  return `다음 음성 지시와 문서 참조 기록을 함께 확인해 요청한 변경을 정리해 주세요.\n\n## 통합 타임라인\n\n${lines.join('\n')||'시간 정보가 있는 기록 없음'}\n${untimedSpeech.trim()?`\n### 시간 정보 없는 전사문\n\n아래 전사문은 발화와 문서 조작의 시점을 맞추지 않았습니다.\n\n${untimedSpeech}\n`:''}`;
}

export function createDocumentRecorder({api,getContext,onChange=()=>{},onResult=()=>{},mediaDevices=globalThis.navigator?.mediaDevices,MediaRecorder=globalThis.MediaRecorder,now=()=>performance.now(),setTimer=setInterval,clearTimer=clearInterval}) {
  const state={phase:'idle',error:'',events:[],elapsed:0,result:null};
  let epoch=0,stream=null,recorder=null,chunks=[],bytes=0,started=0,timer=null,request=null,pendingStart=null,frozenEvents=null;
  const notify=()=>onChange(state);
  const release=()=>{if(timer!==null){clearTimer(timer);timer=null;}stream?.getTracks().forEach(track=>track.stop());stream=null;};
  const settleStart=()=>{pendingStart?.();pendingStart=null;};
  const stopDevice=()=>{const active=recorder;recorder=null;if(active&&active.state!=='inactive')active.stop();};
  const elapsed=()=>Math.max(0,Math.floor(now()-started));
  function freezeRecording(){if(frozenEvents!==null)return;state.elapsed=elapsed();frozenEvents=Object.freeze(state.events.map(event=>Object.freeze({...event})));state.events=frozenEvents;}
  function fail(message){epoch++;request?.abort();release();settleStart();stopDevice();chunks=[];state.phase='error';state.error=message;notify();}
  function note(type,context=getContext(),text=''){
    if(state.phase!=='recording'||!context?.file||state.events.length>=500)return;
    const event={t:elapsed(),type,file:context.file,source:context.source||'file',heading:String(context.heading||'').slice(0,240),anchor:String(context.anchor||'').slice(0,240),text:String(text).slice(0,2000)};
    for(const field of LOCATION_FIELDS)if(context[field]!==undefined)event[field]=context[field];
    state.events.push(event);notify();
  }
  async function start(){
    if(['starting','recording','uploading','stopping'].includes(state.phase))return;
    const current=++epoch;state.phase='starting';state.error='';state.result=null;state.events=[];state.elapsed=0;frozenEvents=null;notify();request=new AbortController();
    try{
      const settings=await api('/api/v1/transcription/settings',{signal:request.signal});
      if(current!==epoch)return;
      if(!settings.configured)throw new Error('OpenAI API 키를 먼저 설정해 주세요. 마이크는 아직 사용하지 않았어요.');
      if(!mediaDevices?.getUserMedia||!MediaRecorder)throw new Error('이 브라우저에서 마이크 녹음을 사용할 수 없어요.');
      const acquired=await mediaDevices.getUserMedia({audio:true});
      if(current!==epoch){acquired.getTracks().forEach(track=>track.stop());return;}
      stream=acquired;
      const mime=['audio/webm;codecs=opus','audio/webm','audio/mp4','audio/ogg'].find(type=>MediaRecorder.isTypeSupported?.(type));
      const active=new MediaRecorder(stream,mime?{mimeType:mime}:undefined);recorder=active;chunks=[];bytes=0;
      const began=new Promise(resolve=>{pendingStart=resolve;});
      active.addEventListener('start',()=>{
        if(current!==epoch||state.phase!=='starting')return;
        started=now();state.elapsed=0;state.phase='recording';
        timer=setTimer(()=>{if(current!==epoch||state.phase!=='recording')return;state.elapsed=elapsed();notify();},250);
        note('open');notify();settleStart();
      });
      active.addEventListener('dataavailable',event=>{
        if(current!==epoch||!['recording','stopping'].includes(state.phase)||!event.data?.size)return;
        bytes+=event.data.size;
        if(bytes>(settings.max_audio_bytes||20971520)){fail('녹음 용량이 전사 가능한 크기를 넘었어요. 짧게 나누어 녹음해 주세요.');return;}
        chunks.push(event.data);
      });
      active.addEventListener('stop',()=>{
        if(current!==epoch||!['recording','stopping'].includes(state.phase))return;
        freezeRecording();void finish(current,settings,active.mimeType);
      });
      active.addEventListener('error',()=>{if(current===epoch)fail('녹음 장치에서 오류가 발생했어요.');});
      active.start(1000);await began;
    }catch(error){if(current===epoch)fail(error.message||'녹음을 시작하지 못했어요.');}
  }
  async function finish(current,settings,mimeType){
    release();state.phase='uploading';notify();
    const audio=new Blob(chunks,{type:mimeType||'audio/webm'});chunks=[];recorder=null;
    if(!audio.size){state.phase='error';state.error='녹음된 오디오가 없어요.';notify();return;}
    const events=frozenEvents;const form=new FormData();form.append('audio',audio,'recording');request=new AbortController();
    try{
      const response=await api('/api/v1/transcription',{method:'POST',body:form,signal:request.signal});
      if(current!==epoch)return;
      const transcript=normalizeTranscript(response);
      if(!transcript.text.trim())throw new Error('전사된 내용이 없어요.');
      const merged=buildRecordingTimeline(transcript,events);
      state.result={...transcript,model:response.model||settings.model,events,...merged,instruction:buildRecordingInstruction(transcript,events)};
      state.phase='complete';notify();onResult(state.result);
    }catch(error){if(current!==epoch)return;state.phase='error';state.error=error.message||'음성 전사에 실패했어요. 자동으로 다시 요청하지 않았어요.';notify();}
  }
  return {state,start,note,stop(){if(state.phase!=='recording'||!recorder)return;freezeRecording();state.phase='stopping';notify();recorder.stop();release();},clear(){epoch++;request?.abort();release();settleStart();stopDevice();chunks=[];bytes=0;frozenEvents=null;state.phase='idle';state.error='';state.events=[];state.elapsed=0;state.result=null;notify();},dispose(){this.clear();}};
}
