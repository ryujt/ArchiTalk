import {test,expect} from './fixtures.mjs';
import {writeFile} from 'node:fs/promises';
import {join} from 'node:path';

async function fakeMedia(page,{denyClipboard=false}={}){
  await page.addInitScript(({denyClipboard})=>{
    window.voiceProbe={microphones:0,stops:0,copies:[],attempts:0};
    Object.defineProperty(navigator,'mediaDevices',{configurable:true,value:{getUserMedia:async()=>{
      window.voiceProbe.microphones++;return {getTracks:()=>[{stop(){window.voiceProbe.stops++;}}]};
    }}});
    Object.defineProperty(navigator,'clipboard',{configurable:true,value:{writeText:async text=>{
      window.voiceProbe.attempts++;if(denyClipboard)throw new Error('Synthetic clipboard denial');window.voiceProbe.copies.push(text);
    }}});
    window.MediaRecorder=class extends EventTarget{
      static isTypeSupported(type){return type.startsWith('audio/webm');}
      constructor(_stream,options){super();this.state='inactive';this.mimeType=options?.mimeType||'audio/webm';}
      start(){this.state='recording';}
      stop(){if(this.state==='inactive')return;this.state='inactive';const event=new Event('dataavailable');
        event.data=new Blob([new Uint8Array([0x1a,0x45,0xdf,0xa3]),'SYNTHETIC AUDIO ONLY'],{type:this.mimeType});this.dispatchEvent(event);queueMicrotask(()=>this.dispatchEvent(new Event('stop')));
      }
    };
  },{denyClipboard});
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
  await page.route(`${server.origin}/api/v1/transcription/settings`,route=>route.fulfill({json:{configured,model:'gpt-transcribe',max_audio_bytes:20971520,timeout_seconds:120,key_source:configured?'file':'none'}}));
  await page.route(`${server.origin}/api/v1/transcription`,async route=>{
    const request=route.request();posts.push({type:request.headers()['content-type'],body:request.postDataBuffer()?.toString('utf8')||''});
    await route.fulfill({json:{text:'선택한 부분을 간단히 고쳐 주세요.',model:'gpt-transcribe'}});
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
  expect(posts).toHaveLength(1);expect(posts[0].type).toMatch(/^multipart\/form-data; boundary=/);
  expect(posts[0].body.match(/name="audio"/g)).toHaveLength(1);expect(posts[0].body).not.toContain(selected);expect(posts[0].body).not.toContain('같은 제목');
  expect(posts[0].body).not.toMatch(/name="(?:events|transcript|context|api_key)"/);
  await page.screenshot({path:testInfo.outputPath('voice-instruction-copied.png'),fullPage:true});
  expect(errors).toEqual([]);expect(violations).toEqual([]);
});

test('clipboard denial offers manual copy and pagehide stops a later live microphone',async({server,productPage},testInfo)=>{
  const {page,errors,violations}=productPage;await fakeMedia(page,{denyClipboard:true});const posts=await transcription(page,server,true);await openFile(page,server);
  await page.locator('#record-toggle').click();await expect(page.locator('#record-status')).toHaveText('녹음 중');await page.locator('#record-toggle').click();
  const fallback=page.getByRole('dialog',{name:'텍스트 직접 복사'});await expect(fallback).toBeVisible();await expect(page.locator('#manual-copy-text')).toHaveValue(/선택한 부분을 간단히 고쳐 주세요/);
  expect(await page.evaluate(()=>voiceProbe.copies.length)).toBe(0);expect(posts).toHaveLength(1);
  await page.screenshot({path:testInfo.outputPath('manual-voice-copy.png'),fullPage:true});await fallback.getByRole('button',{name:'닫기',exact:true}).click();
  await page.locator('#record-toggle').click();await expect(page.locator('#record-status')).toHaveText('녹음 중');
  await page.evaluate(()=>window.dispatchEvent(new Event('pagehide')));
  await expect.poll(()=>page.evaluate(()=>voiceProbe.stops)).toBe(2);expect(posts).toHaveLength(1);expect(errors).toEqual([]);expect(violations).toEqual([]);
});
