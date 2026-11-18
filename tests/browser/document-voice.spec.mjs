import {test,expect} from './fixtures.mjs';
import {writeFile} from 'node:fs/promises';
import {join} from 'node:path';

async function fakeMedia(page,{denyClipboard=false,manualClock=false,manualStart=false}={}){
  // Every native clipboard attempt is intercepted, including unexpected ones.
  // Successful native copies below replace this route with a test-only sink.
  await page.route('**/api/v1/clipboard**',route=>route.fulfill({status:503,json:{error:{code:'clipboard_unavailable',message:'SYNTHETIC_NATIVE_RAW_ERROR_NOT_FOR_UI'}}}));
  await page.addInitScript(({denyClipboard,manualClock,manualStart})=>{
    window.voiceProbe={microphones:0,stops:0,copies:[],attemptedTexts:[],activations:[],attempts:0,now:1000,denyClipboard,holdClipboard:false};
    if(manualClock)Object.defineProperty(performance,'now',{configurable:true,value:()=>window.voiceProbe.now});
    Object.defineProperty(navigator,'mediaDevices',{configurable:true,value:{getUserMedia:async()=>{
      window.voiceProbe.microphones++;return {getTracks:()=>[{stop(){window.voiceProbe.stops++;}}]};
    }}});
    Object.defineProperty(navigator,'clipboard',{configurable:true,value:{writeText:async text=>{
      window.voiceProbe.attempts++;window.voiceProbe.attemptedTexts.push(text);window.voiceProbe.activations.push(navigator.userActivation?.isActive===true);
      if(window.voiceProbe.holdClipboard)return new Promise((resolve,reject)=>{window.pendingVoiceCopy={resolve,reject,text};});
      if(window.voiceProbe.denyClipboard)throw new DOMException('SYNTHETIC_BROWSER_RAW_ERROR_NOT_FOR_UI','NotAllowedError');window.voiceProbe.copies.push(text);
    }}});
    window.MediaRecorder=class extends EventTarget{
      static isTypeSupported(type){return type.startsWith('audio/webm');}
      constructor(_stream,options){super();this.state='inactive';this.mimeType=options?.mimeType||'audio/webm';window.voiceRecorder=this;}
      start(){this.state='recording';if(!manualStart)queueMicrotask(()=>this.dispatchEvent(new Event('start')));}
      stop(){if(this.state==='inactive')return;this.state='inactive';const event=new Event('dataavailable');
        event.data=new Blob([new Uint8Array([0x1a,0x45,0xdf,0xa3]),'SYNTHETIC AUDIO ONLY'],{type:this.mimeType});this.dispatchEvent(event);queueMicrotask(()=>this.dispatchEvent(new Event('stop')));
      }
    };
  },{denyClipboard,manualClock,manualStart});
}

async function openFile(page,server){
  await page.goto(server.origin,{waitUntil:'domcontentloaded'});
  const file=page.locator('[data-file-path="README.md"]');
  await expect(file).toHaveCount(1);
  if(!await file.isVisible())await page.locator('#sidebar-toggle').click();
  await file.click();
  await expect(page.locator('#file-content [data-file-preview]')).toHaveAttribute('data-render-state','ready');
  if(await page.locator('#sidebar-toggle').getAttribute('aria-expanded')==='true')await page.locator('#sidebar-toggle').click();
  await expect(page.locator('#record-toggle')).toBeVisible();
}

async function transcription(page,server,configured){
  const posts=[];
  await page.route(`${server.origin}/api/v1/transcription/settings`,route=>route.fulfill({json:{configured,model:'gpt-transcribe',timing_supported:false,max_audio_bytes:20971520,timeout_seconds:120,key_source:configured?'file':'none'}}));
  await page.route(`${server.origin}/api/v1/transcription`,async route=>{
    const request=route.request();posts.push({type:request.headers()['content-type'],body:request.postDataBuffer()?.toString('utf8')||''});
    await route.fulfill({json:{text:'선택한 부분을 간단히 고쳐 주세요.',model:'gpt-transcribe',segments:[],timing:'none'}});
  });
  return posts;
}

test('a missing key is visible in the document tab before any microphone permission',async({server,productPage})=>{
  const {page,errors,violations}=productPage;await fakeMedia(page);const posts=await transcription(page,server,false);
  await openFile(page,server);await page.locator('#record-toggle').click();
  await expect(page.locator('#record-error')).toBeVisible();await expect(page.locator('#record-error')).toContainText('OpenAI API 키');
  expect(await page.evaluate(()=>voiceProbe.microphones)).toBe(0);expect(posts).toHaveLength(0);expect(errors).toEqual([]);expect(violations).toEqual([]);
});

test('transcription settings keep keys write-only and distinguish keep from delete',async({server,productPage})=>{
  const {page,errors,violations}=productPage;await fakeMedia(page);let configured=false;const patches=[];
  await page.route(`${server.origin}/api/v1/transcription/settings`,route=>route.fulfill({json:{configured,model:'gpt-transcribe',max_audio_bytes:20971520,timeout_seconds:120,key_source:configured?'file':'none'}}));
  await page.route(`${server.origin}/api/v1/transcription/key`,route=>{
    const value=route.request().postDataJSON();patches.push(value);if('api_key' in value)configured=Boolean(value.api_key);
    return route.fulfill({json:{configured,model:value.model,max_audio_bytes:20971520,timeout_seconds:120,key_source:configured?'file':'none'}});
  });
  await openFile(page,server);const dialog=page.locator('#transcription-dialog');
  await page.locator('#recording-settings').click();await expect(page.locator('#transcription-model')).toHaveValue('gpt-transcribe');
  await page.locator('#transcription-key').fill('sk-synthetic-ui-fixture');await dialog.getByRole('button',{name:'전사 설정 저장'}).click();await expect(dialog).not.toBeVisible();
  expect(patches[0]).toEqual({model:'gpt-transcribe',api_key:'sk-synthetic-ui-fixture'});await expect(page.locator('#transcription-key')).toHaveValue('');
  await page.locator('#recording-settings').click();await expect(page.locator('#transcription-model')).toBeEnabled();await expect(page.locator('#transcription-key')).toHaveValue('');
  await dialog.getByRole('button',{name:'전사 설정 저장'}).click();await expect(dialog).not.toBeVisible();expect(patches[1]).toEqual({model:'gpt-transcribe'});
  await page.locator('#recording-settings').click();await expect(page.locator('#transcription-model')).toBeEnabled();await page.locator('#transcription-clear-key').check();
  await dialog.getByRole('button',{name:'전사 설정 저장'}).click();await expect(dialog).not.toBeVisible();expect(patches[2]).toEqual({model:'gpt-transcribe',api_key:null});
  expect(await page.evaluate(()=>voiceProbe.microphones)).toBe(0);expect(errors).toEqual([]);expect(violations).toEqual([]);
});

test('stopping a recording copies the transcript and exact selected document location',async({server,productPage},testInfo)=>{
  const {page,errors,violations}=productPage;
  const selected='선택 문장 UNIQUE_SELECTION_NOT_UPLOADED';
  await writeFile(join(server.project,'README.md'),`# 음성 검증\n\n## 같은 제목\n\n첫 번째 절입니다.\n\n## 같은 제목\n\n${selected}\n`);
  await fakeMedia(page);const posts=await transcription(page,server,true);await openFile(page,server);
  await page.locator('#record-toggle').click();await expect(page.locator('#record-status')).toHaveText('녹음 중');
  const paragraph=page.locator('#file-content p').filter({hasText:selected});
  await paragraph.evaluate(node=>{const range=document.createRange();range.selectNodeContents(node);const selection=getSelection();selection.removeAllRanges();selection.addRange(range);});
  await paragraph.dispatchEvent('mouseup');await page.locator('#record-toggle').click();
  await expect.poll(()=>page.evaluate(()=>voiceProbe.copies.length)).toBe(1);
  const probe=await page.evaluate(()=>voiceProbe);
  expect(probe.microphones).toBe(1);expect(probe.stops).toBe(1);expect(probe.copies[0]).toContain('선택한 부분을 간단히 고쳐 주세요.');
  expect(probe.copies[0]).toContain(selected);expect(probe.copies[0]).toContain('README.md');expect(probe.copies[0]).toContain('같은-제목-1');
  expect(probe.copies[0]).toContain('시간 정보 없는 전사문');await expect(page.locator('#record-status')).toContainText('시간 정보 없이');
  expect(posts).toHaveLength(1);expect(posts[0].type).toMatch(/^multipart\/form-data; boundary=/);
  expect(posts[0].body.match(/name="audio"/g)).toHaveLength(1);expect(posts[0].body).not.toContain(selected);expect(posts[0].body).not.toContain('같은 제목');
  expect(posts[0].body).not.toMatch(/name="(?:events|transcript|context|api_key)"/);
  await page.screenshot({path:testInfo.outputPath('voice-instruction-copied.png'),fullPage:true});
  expect(errors).toEqual([]);expect(violations).toEqual([]);
});

test('speech and selection share the actual start clock and keep a frozen stop time during upload',async({server,productPage},testInfo)=>{
  const {page,errors,violations}=productPage,selected='CLOCK_SELECTION_ONLY';
  await writeFile(join(server.project,'README.md'),`# 시각 검증\n\n## 참조 위치\n\n${selected}\n`);
  await fakeMedia(page,{manualClock:true,manualStart:true});
  await page.route(`${server.origin}/api/v1/transcription/settings`,route=>route.fulfill({json:{configured:true,model:'whisper-1',timing_supported:true,max_audio_bytes:20971520,timeout_seconds:120,key_source:'file'}}));
  let finishUpload;const held=new Promise(resolve=>{finishUpload=resolve;}),posts=[];
  await page.route(`${server.origin}/api/v1/transcription`,async route=>{
    posts.push(route.request().postDataBuffer()?.toString('utf8')||'');await held;
    await route.fulfill({json:{text:'선택한 부분을 고쳐 주세요.',model:'whisper-1',timing:'segment',segments:[{start:1.5,end:1.9,text:'고쳐 주세요.'},{start:.2,end:.8,text:'선택한 부분을'}]}});
  });
  try{
    await openFile(page,server);await page.locator('#record-toggle').click();
    await expect.poll(()=>page.evaluate(()=>Boolean(window.voiceRecorder))).toBe(true);
    await expect(page.locator('#record-status')).toContainText('확인하고 있어요');
    await page.evaluate(()=>{voiceProbe.now=5000;voiceRecorder.dispatchEvent(new Event('start'));});
    await expect(page.locator('#record-status')).toHaveText('녹음 중');
    await page.evaluate(()=>{voiceProbe.now=6500;});const paragraph=page.locator('#file-content p').filter({hasText:selected});
    await paragraph.evaluate(node=>{const range=document.createRange();range.selectNodeContents(node);const selection=getSelection();selection.removeAllRanges();selection.addRange(range);});await paragraph.dispatchEvent('mouseup');
    await page.evaluate(()=>{voiceProbe.now=7000;});await page.locator('#record-toggle').click();
    await expect.poll(()=>posts.length).toBe(1);await expect(page.locator('#record-time')).toHaveText('00:02');
    await page.evaluate(()=>{voiceProbe.now=100000;});finishUpload();
    await expect.poll(()=>page.evaluate(()=>voiceProbe.copies.length)).toBe(1);
    const probe=await page.evaluate(()=>voiceProbe),instruction=probe.copies[0];
    expect(instruction).toContain('00:00.200');expect(instruction).toContain('00:01.500');expect(instruction).toContain('00:01.900');
    expect(instruction.indexOf('선택한 부분을')).toBeLessThan(instruction.indexOf('고쳐 주세요.'));
    expect(instruction.indexOf('고쳐 주세요.')).toBeLessThan(instruction.indexOf(selected));
    expect(instruction.split(selected).length-1).toBe(1);
    expect(instruction).toContain('README.md#참조-위치');expect(instruction).not.toContain('시간 정보 없는 전사문');
    await expect(page.locator('#record-status')).toContainText('시간순으로');await expect(page.locator('#record-events')).toContainText('발화 2개');await expect(page.locator('#record-time')).toHaveText('00:02');
    expect(posts[0]).not.toContain(selected);expect(posts[0].match(/name="audio"/g)).toHaveLength(1);expect(probe.microphones).toBe(1);expect(probe.stops).toBe(1);
    await page.screenshot({path:testInfo.outputPath('synchronized-speech-selection.png'),fullPage:true});expect(errors).toEqual([]);expect(violations).toEqual([]);
  }finally{finishUpload();}
});

test('clipboard denial uses one authenticated local copy for the complete timed instruction',async({server,productPage},testInfo)=>{
  const {page,errors,violations}=productPage,selected='LOCAL_COPY_TIMED_SELECTION';
  await writeFile(join(server.project,'README.md'),`# 복사 검증\n\n## 원문 위치\n\n${selected}\n`);
  await fakeMedia(page,{denyClipboard:true,manualClock:true});const native=[];
  await page.route(`${server.origin}/api/v1/clipboard`,async route=>{
    const request=route.request();native.push({method:request.method(),url:request.url(),headers:request.headers(),body:request.postDataJSON()});
    await route.fulfill({json:{copied:true}});
  });
  await page.route(`${server.origin}/api/v1/transcription/settings`,route=>route.fulfill({json:{configured:true,model:'whisper-1',timing_supported:true,max_audio_bytes:20971520,timeout_seconds:120,key_source:'file'}}));
  const posts=[];
  await page.route(`${server.origin}/api/v1/transcription`,async route=>{
    posts.push(route.request().postDataBuffer()?.toString('utf8')||'');
    await route.fulfill({json:{text:'선택한 부분을 고쳐 주세요.',model:'whisper-1',timing:'segment',segments:[{start:.2,end:.8,text:'선택한 부분을'},{start:1.5,end:1.9,text:'고쳐 주세요.'}]}});
  });
  await openFile(page,server);await page.locator('#record-toggle').click();await expect(page.locator('#record-status')).toHaveText('녹음 중');
  await page.evaluate(()=>{voiceProbe.now=2500;});const paragraph=page.locator('#file-content p').filter({hasText:selected});
  await paragraph.evaluate(node=>{const range=document.createRange();range.selectNodeContents(node);const selection=getSelection();selection.removeAllRanges();selection.addRange(range);});await paragraph.dispatchEvent('mouseup');
  await page.evaluate(()=>{voiceProbe.now=3000;});await page.locator('#record-toggle').click();
  await expect(page.locator('#announcer')).toContainText('복사했어요');await expect.poll(()=>native.length).toBe(1);
  const probe=await page.evaluate(()=>voiceProbe),request=native[0],instruction=request.body.text;
  expect(request.method).toBe('POST');expect(request.url).toBe(`${server.origin}/api/v1/clipboard`);
  expect(request.headers.authorization).toBe(`Bearer ${server.token}`);expect(request.headers.origin).toBe(server.origin);expect(request.headers['content-type']).toContain('application/json');
  expect(Object.keys(request.body)).toEqual(['text']);expect(instruction).toBe(probe.attemptedTexts[0]);expect(probe.attempts).toBe(1);expect(probe.copies).toEqual([]);
  expect(instruction).toContain('## 통합 타임라인');expect(instruction).toContain('[00:00.200–00:00.800] 발화');expect(instruction).toContain('[00:01.500–00:01.900] 발화');
  expect(instruction).toContain('README.md#원문-위치');expect(instruction.split(selected).length-1).toBe(1);expect(instruction.indexOf('고쳐 주세요.')).toBeLessThan(instruction.indexOf(selected));
  expect(posts).toHaveLength(1);expect(posts[0]).not.toContain(selected);expect(posts[0].match(/name="audio"/g)).toHaveLength(1);
  await expect(page.locator('.copy-dialog')).not.toBeVisible();await expect(page.locator('#record-time')).toHaveText('00:02');
  await page.screenshot({path:testInfo.outputPath('native-voice-copy.png'),fullPage:true});expect(errors).toEqual([]);expect(violations).toEqual([]);
});

test('both copy failures preserve the instruction and direct-click retry succeeds without transcription',async({server,productPage},testInfo)=>{
  const {page,errors,violations}=productPage;await fakeMedia(page,{denyClipboard:true});const native=[];
  await page.route(`${server.origin}/api/v1/clipboard`,route=>{
    native.push(route.request().postDataJSON());return route.fulfill({status:503,json:{error:{code:'clipboard_unavailable',message:'SYNTHETIC_NATIVE_RAW_ERROR_NOT_FOR_UI'}}});
  });
  const posts=await transcription(page,server,true);await openFile(page,server);
  await page.locator('#record-toggle').click();await expect(page.locator('#record-status')).toHaveText('녹음 중');await page.locator('#record-toggle').click();
  const fallback=page.getByRole('dialog',{name:'텍스트 직접 복사'});await expect(fallback).toBeVisible();await expect(page.locator('#manual-copy-text')).toHaveValue(/선택한 부분을 간단히 고쳐 주세요/);
  const instruction=await page.locator('#manual-copy-text').inputValue();expect(native).toEqual([{text:instruction}]);
  expect(instruction).toBe(await page.evaluate(()=>voiceProbe.attemptedTexts[0]));
  await expect(fallback.locator('[data-copy-reason]')).toBeVisible();await expect(fallback.locator('[data-copy-reason]')).toContainText(/브라우저|클립보드|로컬/);
  await expect(fallback).not.toContainText('SYNTHETIC_NATIVE_RAW_ERROR_NOT_FOR_UI');await expect(fallback).not.toContainText('SYNTHETIC_BROWSER_RAW_ERROR_NOT_FOR_UI');
  expect(await page.evaluate(()=>voiceProbe.copies.length)).toBe(0);expect(posts).toHaveLength(1);
  for(const name of ['닫기','복사']){
    const button=fallback.getByRole('button',{name,exact:true});await expect(button).toBeVisible();const box=await button.boundingBox();
    expect(box.y).toBeGreaterThanOrEqual(0);expect(box.y+box.height).toBeLessThanOrEqual((await page.viewportSize()).height);expect(box.x+box.width).toBeLessThanOrEqual((await page.viewportSize()).width);
  }
  expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true);
  await page.screenshot({path:testInfo.outputPath('manual-voice-copy.png'),fullPage:true});
  await page.evaluate(()=>{voiceProbe.denyClipboard=false;});await fallback.getByRole('button',{name:'복사',exact:true}).click();
  await expect(fallback).not.toBeVisible();expect(await page.evaluate(()=>voiceProbe.copies)).toEqual([instruction]);
  expect(await page.evaluate(()=>voiceProbe.activations.at(-1))).toBe(true);expect(native).toHaveLength(1);expect(posts).toHaveLength(1);
  await page.locator('#record-toggle').click();await expect(page.locator('#record-status')).toHaveText('녹음 중');
  await page.evaluate(()=>window.dispatchEvent(new Event('pagehide')));
  await expect.poll(()=>page.evaluate(()=>voiceProbe.stops)).toBe(2);expect(posts).toHaveLength(1);expect(errors).toEqual([]);expect(violations).toEqual([]);
});

test('workspace invalidation retires a pending browser copy before native fallback can run',async({server,productPage})=>{
  const {page,errors,violations}=productPage;await fakeMedia(page);const native=[];
  await page.route(`${server.origin}/api/v1/clipboard`,route=>{native.push(route.request().postDataJSON());return route.fulfill({json:{copied:true}});});
  const posts=await transcription(page,server,true);await openFile(page,server);
  await page.evaluate(()=>{voiceProbe.holdClipboard=true;});
  await page.locator('#record-toggle').click();await expect(page.locator('#record-status')).toHaveText('녹음 중');await page.locator('#record-toggle').click();
  await expect.poll(()=>page.evaluate(()=>Boolean(window.pendingVoiceCopy))).toBe(true);
  await page.route(`${server.origin}/api/v1/workspace`,route=>route.fulfill({status:410,json:{error:{code:'workspace_reset_required',message:'합성 테스트의 프로젝트 연결 무효화'}}}));
  await page.evaluate(()=>window.dispatchEvent(new Event('online')));await expect(page.locator('#connection')).toContainText('새 접속 필요');
  await page.evaluate(async()=>{pendingVoiceCopy.reject(new DOMException('late synthetic denial','NotAllowedError'));await new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve)));});
  expect(native).toEqual([]);await expect(page.locator('.copy-dialog')).not.toBeVisible();await expect(page.locator('#manual-copy-text')).toHaveValue('');
  await expect(page.locator('#record-copy')).not.toBeVisible();expect(posts).toHaveLength(1);expect(await page.evaluate(()=>voiceProbe.stops)).toBe(1);expect(errors).toEqual([]);expect(violations).toEqual([]);
});

test('workspace invalidation aborts or ignores a late native clipboard response',async({server,productPage})=>{
  const {page,errors,violations}=productPage;await fakeMedia(page,{denyClipboard:true});const native=[],failed=[];
  let finishNative;const held=new Promise(resolve=>{finishNative=resolve;});let nativeSettled=false,deliveryError=null;
  page.on('requestfailed',request=>{if(new URL(request.url()).pathname==='/api/v1/clipboard')failed.push(request.failure()?.errorText||'failed');});
  await page.route(`${server.origin}/api/v1/clipboard`,async route=>{
    native.push(route.request().postDataJSON());await held;
    try{await route.fulfill({json:{copied:true}});}catch(error){deliveryError=error;}finally{nativeSettled=true;}
  });
  const posts=await transcription(page,server,true);
  try{
    await openFile(page,server);await page.locator('#record-toggle').click();await expect(page.locator('#record-status')).toHaveText('녹음 중');await page.locator('#record-toggle').click();
    await expect.poll(()=>native.length).toBe(1);
    await page.route(`${server.origin}/api/v1/workspace`,route=>route.fulfill({status:410,json:{error:{code:'workspace_reset_required',message:'합성 테스트의 프로젝트 연결 무효화'}}}));
    await page.evaluate(()=>window.dispatchEvent(new Event('online')));await expect(page.locator('#connection')).toContainText('새 접속 필요');
    const notice=await page.locator('#announcer').textContent();expect(notice).toContain('이전 프로젝트 기록을 비웠어요');
    finishNative();await expect.poll(()=>nativeSettled).toBe(true);
    await page.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
    expect(deliveryError===null||failed.length>0).toBe(true);expect(native).toHaveLength(1);
    await expect(page.locator('#announcer')).toHaveText(notice);await expect(page.locator('.copy-dialog')).not.toBeVisible();await expect(page.locator('#manual-copy-text')).toHaveValue('');
    await expect(page.locator('#record-copy')).not.toBeVisible();expect(posts).toHaveLength(1);expect(await page.evaluate(()=>voiceProbe.stops)).toBe(1);expect(errors).toEqual([]);expect(violations).toEqual([]);
  }finally{finishNative();}
});
