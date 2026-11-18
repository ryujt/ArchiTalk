export function lineRangeFromOffsets(text,start,end=start) {
  const source=String(text);start=Math.max(0,Math.min(source.length,Number.isSafeInteger(start)?start:0));end=Math.max(start,Math.min(source.length,Number.isSafeInteger(end)?end:start));
  return {line:source.slice(0,start).split(/\r?\n/).length,lineEnd:source.slice(0,end>start?end-1:end).split(/\r?\n/).length,startOffset:start,endOffset:end};
}

export function buildRecordingInstruction(transcript,events=[]) {
  const time=milliseconds=>{const seconds=Math.floor(Math.max(0,milliseconds)/1000);return `${String(Math.floor(seconds/60)).padStart(2,'0')}:${String(seconds%60).padStart(2,'0')}.${String(Math.floor(milliseconds%1000)).padStart(3,'0')}`;};
  const timeline=events.map(event=>`- [${time(event.t)}] ${event.source==='draft'?'[생성 초안] ':''}${event.file}${event.line?`:${event.line}${event.lineEnd>event.line?`-${event.lineEnd}`:''}`:''}${event.anchor?`#${event.anchor}`:''}${event.heading?` · 제목: ${event.heading}`:''} · ${event.type}${event.text?`\n  ${event.text.replace(/\n/g,'\n  ')}`:''}`).join('\n');
  return `다음 음성 지시와 문서 참조 기록을 함께 확인해 요청한 변경을 정리해 주세요.\n\n## 음성 지시\n\n${transcript.trim()}\n\n## 문서 참조 기록\n\n${timeline||'문서 조작 기록 없음'}\n`;
}

export function createDocumentRecorder({api,getContext,onChange=()=>{},onResult=()=>{},mediaDevices=globalThis.navigator?.mediaDevices,MediaRecorder=globalThis.MediaRecorder,now=()=>performance.now(),setTimer=setInterval,clearTimer=clearInterval}) {
  const state={phase:'idle',error:'',events:[],elapsed:0,result:null};let epoch=0,stream=null,recorder=null,chunks=[],bytes=0,started=0,timer=null,request=null;
  const notify=()=>onChange(state);
  const release=()=>{if(timer!==null){clearTimer(timer);timer=null;}stream?.getTracks().forEach(track=>track.stop());stream=null;};
  function note(type,context=getContext(),text=''){if(state.phase!=='recording'||!context?.file||state.events.length>=500)return;state.events.push({t:Math.max(0,Math.floor(now()-started)),type,file:context.file,source:context.source||'file',heading:String(context.heading||'').slice(0,240),anchor:String(context.anchor||'').slice(0,240),...(Number.isSafeInteger(context.line)&&context.line>0?{line:context.line,lineEnd:context.lineEnd,startOffset:context.startOffset,endOffset:context.endOffset}:{}),text:String(text).slice(0,2000)});notify();}
  async function start(){if(['starting','recording','uploading','stopping'].includes(state.phase))return;const current=++epoch;state.phase='starting';state.error='';state.result=null;state.events=[];notify();request=new AbortController();
    try{const settings=await api('/api/v1/transcription/settings',{signal:request.signal});if(current!==epoch)return;if(!settings.configured)throw new Error('OpenAI API 키를 먼저 설정해 주세요. 마이크는 아직 사용하지 않았어요.');if(!mediaDevices?.getUserMedia||!MediaRecorder)throw new Error('이 브라우저에서 마이크 녹음을 사용할 수 없어요.');
      const acquired=await mediaDevices.getUserMedia({audio:true});if(current!==epoch){acquired.getTracks().forEach(track=>track.stop());return;}stream=acquired;const mime=['audio/webm;codecs=opus','audio/webm','audio/mp4','audio/ogg'].find(type=>MediaRecorder.isTypeSupported?.(type));recorder=new MediaRecorder(stream,mime?{mimeType:mime}:undefined);chunks=[];bytes=0;started=now();state.elapsed=0;state.phase='recording';
      recorder.addEventListener('dataavailable',event=>{if(current!==epoch||!event.data?.size)return;bytes+=event.data.size;if(bytes>(settings.max_audio_bytes||20971520)){epoch++;release();if(recorder.state!=='inactive')recorder.stop();chunks=[];state.phase='error';state.error='녹음 용량이 전사 가능한 크기를 넘었어요. 짧게 나누어 녹음해 주세요.';notify();return;}chunks.push(event.data);});
      recorder.addEventListener('stop',()=>{if(current===epoch)void finish(current,settings);});recorder.addEventListener('error',()=>{if(current===epoch){epoch++;release();chunks=[];state.phase='error';state.error='녹음 장치에서 오류가 발생했어요.';notify();}});recorder.start(1000);timer=setTimer(()=>{state.elapsed=Math.max(0,Math.floor(now()-started));notify();},250);note('open');notify();
    }catch(error){if(current!==epoch)return;epoch++;release();if(recorder?.state==='recording')recorder.stop();recorder=null;chunks=[];state.phase='error';state.error=error.message||'녹음을 시작하지 못했어요.';notify();}
  }
  async function finish(current,settings){state.elapsed=Math.max(0,Math.floor(now()-started));release();state.phase='uploading';notify();const audio=new Blob(chunks,{type:recorder?.mimeType||'audio/webm'});chunks=[];recorder=null;if(!audio.size){state.phase='error';state.error='녹음된 오디오가 없어요.';notify();return;}
    const form=new FormData();form.append('audio',audio,'recording');request=new AbortController();
    try{const result=await api('/api/v1/transcription',{method:'POST',body:form,signal:request.signal});if(current!==epoch)return;const text=String(result.text||'').trim();if(!text)throw new Error('전사된 내용이 없어요.');state.result={text,model:result.model||settings.model,events:state.events.map(event=>({...event})),instruction:buildRecordingInstruction(text,state.events)};state.phase='complete';notify();onResult(state.result);}
    catch(error){if(current!==epoch)return;state.phase='error';state.error=error.message||'음성 전사에 실패했어요. 자동으로 다시 요청하지 않았어요.';notify();}
  }
  return {state,start,note,stop(){if(state.phase!=='recording'||!recorder)return;state.phase='stopping';notify();recorder.stop();release();},clear(){epoch++;request?.abort();release();if(recorder?.state!=='inactive')recorder?.stop();recorder=null;chunks=[];bytes=0;state.phase='idle';state.error='';state.events=[];state.elapsed=0;state.result=null;notify();},dispose(){this.clear();}};
}
