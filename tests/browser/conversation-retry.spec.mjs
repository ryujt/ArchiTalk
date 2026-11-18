import {test,expect} from './fixtures.mjs';
import {writeFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';

test('a failed answer can be explicitly retried from recovery without duplicating its user message',async({server,productPage},testInfo)=>{
  const {page,errors,violations}=productPage;
  const mutations=[],networkFailures=[],faviconRequests=[],assetResponses=[],consoleErrors=[];
  page.on('console',message=>{if(message.type()==='error')consoleErrors.push(message.text());});
  page.on('response',response=>{const path=new URL(response.url()).pathname;if(!path.startsWith('/api/'))assetResponses.push({path,status:response.status()});});
  page.on('requestfailed',request=>networkFailures.push({path:new URL(request.url()).pathname,error:request.failure()?.errorText}));
  page.on('request',request=>{
    const path=new URL(request.url()).pathname;
    if(path==='/favicon.ico')faviconRequests.push({authorization:request.headers().authorization||null});
    if(path.startsWith('/api/v1/')&&request.method()==='POST')mutations.push({path,body:request.postDataJSON()});
  });
  // Observe only verified initialization progress from the app's own SSE
  // response. This reader opens no second connection and stores no model text.
  await page.addInitScript(()=>{
    window.__retryProbe={connected:false,attempts:0,csp:[]};
    document.addEventListener('securitypolicyviolation',event=>window.__retryProbe.csp.push({directive:event.effectiveDirective,blocked:event.blockedURI}));
    const nativeFetch=window.fetch;
    window.fetch=async function(...args){
      const response=await nativeFetch.apply(this,args);
      const url=new URL(typeof args[0]==='string'?args[0]:args[0].url,location.href);
      if(url.pathname.endsWith('/events')&&response.ok){
        window.__retryProbe.connected=true;
        void (async()=>{
          const reader=response.clone().body.getReader(),decoder=new TextDecoder();let buffer='';
          try{
            while(true){
              const {done,value}=await reader.read();if(done)break;
              buffer+=decoder.decode(value,{stream:true});let end;
              while((end=buffer.indexOf('\n\n'))>=0){
                const frame=buffer.slice(0,end);buffer=buffer.slice(end+2);
                const text=frame.split('\n').filter(line=>line.startsWith('data:')).map(line=>line.slice(5).trimStart()).join('\n');
                if(!text)continue;
                const event=JSON.parse(text);
                if(event.type==='progress'&&event.payload?.message==='Claude is generating a response.')window.__retryProbe.attempts++;
              }
            }
          }catch{/* Page reload/abort closes this observation with the app feed. */}
        })();
      }
      return response;
    };
  });
  await server.configureConversationOverflow(true,{hold:true});
  await page.goto(server.origin,{waitUntil:'domcontentloaded'});
  await expect(page.locator('link[rel=icon]')).toHaveAttribute('href','/favicon.ico');
  const nodeIcon=await fetch(`${server.origin}/favicon.ico`,{redirect:'error'});
  const exactIconBytes=Buffer.from(await nodeIcon.arrayBuffer());
  const nodeIconResult={status:nodeIcon.status,type:nodeIcon.headers.get('content-type'),bytes:exactIconBytes.length,hash:createHash('sha256').update(exactIconBytes).digest('hex')};
  expect(nodeIconResult.status).toBe(200);expect(nodeIconResult.type).toBe('image/x-icon');
  // Playwright 1.65.1 silently aborts URLs ending in /favicon.ico when routing
  // is enabled (coreBundle requestStarted). A query keeps the same HTTP handler
  // while allowing real browser fetch/decode and retaining our external guard.
  let icon;
  try{
    icon=await page.evaluate(async()=>{
      const url=new URL(document.querySelector('link[rel=icon]').href);url.searchParams.set('browser-probe','1');
      const response=await fetch(url.href,{cache:'reload',redirect:'error'}),bytes=await response.arrayBuffer();
      const digest=await crypto.subtle.digest('SHA-256',bytes);
      const image=new Image();image.src=url.href;await image.decode();
      return {status:response.status,type:response.headers.get('content-type'),signature:[...new Uint8Array(bytes).subarray(0,4)],hash:[...new Uint8Array(digest)].map(byte=>byte.toString(16).padStart(2,'0')).join(''),width:image.naturalWidth,height:image.naturalHeight};
    });
  }catch(error){
    const diagnostic={nodeIconResult,networkFailures,assetResponses,consoleErrors,browser:await page.evaluate(()=>({csp:window.__retryProbe.csp,asset:document.querySelector('link[rel=icon]').href,origin:location.origin,base:document.baseURI}))};
    await writeFile(testInfo.outputPath('favicon-diagnostic.json'),JSON.stringify(diagnostic,null,2));
    throw new Error(`Declared favicon failed in the browser: ${JSON.stringify(diagnostic)}`,{cause:error});
  }
  expect(icon.status).toBe(200);expect(icon.type).toBe('image/x-icon');
  expect(faviconRequests.length).toBeGreaterThan(0);expect(faviconRequests.every(request=>request.authorization===null)).toBe(true);
  expect(icon.signature).toEqual([0,0,1,0]);
  expect(icon.hash).toBe(nodeIconResult.hash);expect(icon.width).toBeGreaterThan(0);expect(icon.height).toBeGreaterThan(0);
  const intro='합성 알림 요구를 보존한 채 답변만 다시 요청합니다.';
  await page.locator('#project-intro').fill(intro);await page.locator('#start-form button[type=submit]').click();
  await expect(page.locator('.pending-turn')).toBeVisible();
  await expect.poll(()=>page.evaluate(()=>window.__retryProbe.connected)).toBe(true);
  await server.releaseConversationFixture();
  await expect(page.getByRole('heading',{name:'작업을 완료하지 못했어요',exact:true})).toBeVisible();
  await expect(page.locator('.error-detail')).toHaveText('questions[0].options: observed 5 items; limit 4 items');
  await expect(page.getByRole('button',{name:'답변 다시 받기',exact:true})).toHaveClass('primary');
  await expect(page.getByRole('button',{name:'대화로 돌아가기',exact:true})).toBeVisible();
  await expect(page.getByRole('button',{name:'현재 답변으로 다시 정리',exact:true})).toBeVisible();
  await expect.poll(()=>page.evaluate(()=>window.__retryProbe.attempts)).toBe(2);
  const failed=await server.workspace(),failedRun=failed.runs[0];
  expect(failed.runs).toHaveLength(1);expect(failedRun.status).toBe('failed');expect(failedRun.error.code).toBe('invalid_model_output');
  expect(failed.sessions[0].messages).toHaveLength(1);expect(failed.sessions[0].messages[0].content).toBe(intro);
  await page.screenshot({path:testInfo.outputPath('01-precise-question-error.png'),fullPage:true,animations:'disabled'});

  await page.reload({waitUntil:'domcontentloaded'});
  await expect(page.getByRole('heading',{name:'남겨 둔 설계에서 이어가세요',exact:true})).toBeVisible();
  await expect(page.getByRole('button',{name:'답변 다시 받기',exact:true})).toHaveClass('primary');
  expect((await server.workspace()).runs).toEqual(failed.runs);
  await page.screenshot({path:testInfo.outputPath('02-recovery-retry.png'),fullPage:true,animations:'disabled'});
  await server.configureConversationOverflow(false,{hold:true});
  await page.getByRole('button',{name:'답변 다시 받기',exact:true}).click();
  await expect(page.locator('.pending-turn')).toBeVisible();
  await expect(page.locator('.chat-turn.user')).toHaveCount(1);
  await expect(page.locator('.chat-turn.user')).toContainText(intro);
  const pending=await server.workspace();expect(pending.runs).toHaveLength(2);expect(pending.active_run.kind).toBe('conversation');
  expect(pending.sessions[0].messages).toEqual(failed.sessions[0].messages);
  await expect.poll(()=>page.evaluate(()=>window.__retryProbe.connected)).toBe(true);
  await page.screenshot({path:testInfo.outputPath('03-retry-inline-pending.png'),fullPage:true,animations:'disabled'});
  await server.releaseConversationFixture();
  await expect(page.locator('.chat-turn')).toHaveCount(2);await expect(page.locator('.pending-turn')).toHaveCount(0);
  await expect(page.locator('.chat-thread')).toContainText('현재 말씀하신 내용은 제안과 미정으로 나누어 기록했습니다.');
  await expect.poll(()=>page.evaluate(()=>window.__retryProbe.attempts)).toBe(1);
  const complete=await server.workspace();
  expect(complete.runs).toHaveLength(2);expect(complete.runs[0]).toEqual(failedRun);expect(complete.runs[1].status).toBe('completed');
  expect(complete.active_run).toBeNull();
  expect(complete.sessions[0].messages.filter(message=>message.role==='user').map(message=>message.content)).toEqual([intro]);
  expect(complete.sessions[0].messages.filter(message=>message.role==='assistant')).toHaveLength(1);
  expect(mutations.map(request=>request.path)).toEqual(['/api/v1/sessions',`/api/v1/sessions/${complete.sessions[0].id}/conversation/retry`]);
  expect(mutations[1].body.payload).toEqual({});expect(mutations[1].body.expected_revision).toBe(failed.sessions[0].revision);
  expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true);
  await page.screenshot({path:testInfo.outputPath('04-retried-answer.png'),fullPage:true,animations:'disabled'});
  await writeFile(testInfo.outputPath('retry-boundary.json'),JSON.stringify({faviconStatus:icon.status,faviconContentType:icon.type,exactIconHashMatches:true,decodedIcon:{width:icon.width,height:icon.height},initialAttempts:2,retryAttempts:1,failedRuns:1,completedRuns:1,userMessages:1,assistantMessages:1,postCommands:mutations.length},null,2));
  expect(errors).toEqual([]);expect(violations).toEqual([]);
});
