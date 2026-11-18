import {test,expect} from './fixtures.mjs';
import {access,rm,writeFile} from 'node:fs/promises';
import {join} from 'node:path';

test('deleting this test project state clears an open view and stops old-connection work',async({server,productPage},testInfo)=>{
  const {page,violations,errors}=productPage;
  const requests=[];
  page.on('request',request=>{const path=new URL(request.url()).pathname;if(path.startsWith('/api/v1/'))requests.push({path,method:request.method()});});
  await server.configureSlowFixture(5);
  await page.goto(server.origin,{waitUntil:'domcontentloaded'});
  const oldMarker='초기화 이전의 합성 대화 기록';
  await page.locator('#project-intro').fill(oldMarker);await page.locator('#start-form button[type=submit]').click();
  await expect(page.locator('.chat-turn')).toHaveCount(2);
  await page.locator('[data-action=generate]').click();
  await expect.poll(async()=>{const w=await server.workspace();return w.sessions[0]?.drafts[0]?.reviewed&&w.active_run===null;}).toBe(true);
  await expect(page.locator('.markdown')).toHaveAttribute('data-render-state','ready');
  // An explicit capability fragment on this same origin must reboot the page,
  // even when the browser would otherwise treat it as only an anchor change.
  const rebooted=page.waitForEvent('domcontentloaded');
  const newDocument=page.waitForResponse(response=>new URL(response.url()).pathname==='/'&&response.request().isNavigationRequest());
  await page.evaluate(capability=>{window.__previousConnection=true;location.hash='token='+capability;},server.token);
  await newDocument;await rebooted;
  expect(await page.evaluate(()=>window.__previousConnection===undefined)).toBe(true);
  await page.getByRole('button',{name:'초안 먼저 살펴보기',exact:true}).click();
  await expect(page.locator('.markdown')).toHaveAttribute('data-render-state','ready');
  expect(await page.evaluate(()=>location.hash===''&&sessionStorage.getItem('architalk.session')===null)).toBe(true);
  await page.locator('[data-action=interview]').click();
  await page.locator('#answer-input').fill('SLOW_REQUEST: 초기화 중에 완료될 수 있는 합성 요청');
  await page.locator('#answer-form button[type=submit]').click();
  await expect(page.locator('.pending-turn')).toBeVisible();
  await page.locator('[data-action=review]').click();
  await expect(page.locator('.markdown')).toHaveAttribute('data-render-state','ready');
  await page.getByRole('button',{name:'도구 설정',exact:true}).click();
  await expect(page.getByRole('dialog')).toContainText('모든 프로젝트에 공통 적용');
  await expect(page.getByRole('dialog')).toContainText('문서 저장 폴더는 현재 프로젝트');
  await page.screenshot({path:testInfo.outputPath('01-before-own-state-removal.png'),fullPage:true,animations:'disabled'});

  // This fixture owns its temporary project. Never remove a user's project,
  // browser storage, AI home, or shared configuration directory.
  const stateDirectory=join(server.project,'.architalk');
  expect((await server.workspace()).active_run).not.toBeNull();
  await rm(stateDirectory,{recursive:true});
  await expect(page.getByRole('heading',{name:'프로젝트 기록 연결이 초기화됐어요',exact:true})).toBeVisible();
  await expect(page.locator('.markdown,.chat-thread,.session-item,.plan-file,#answer-form,#start-form')).toHaveCount(0);
  await expect(page.locator('#scene-dialog')).not.toBeVisible();
  await expect(page.locator('#scene-dialog')).toBeEmpty();
  await expect(page.locator('.app-header [data-action=settings]')).toBeDisabled();
  await expect(page.locator('#connection')).toContainText('새 접속 필요');
  expect(await page.evaluate(()=>({authorization:sessionStorage.getItem('architalk.authorization'),selection:sessionStorage.getItem('architalk.session')}))).toEqual({authorization:null,selection:null});
  expect(await page.locator('#workspace-content').textContent()).not.toContain(oldMarker);
  const atReset=requests.length;
  await page.evaluate(()=>{window.dispatchEvent(new Event('online'));return new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve)));});
  expect(requests).toHaveLength(atReset);
  const response=await fetch(`${server.origin}/api/v1/workspace`,{headers:{Authorization:`Bearer ${server.token}`},redirect:'error'});
  expect(response.status).toBe(410);expect((await response.json()).error.code).toBe('workspace_reset_required');
  await expect.poll(async()=>{try{await access(stateDirectory);return false;}catch(error){return error.code==='ENOENT';}}).toBe(true);
  expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true);
  await page.screenshot({path:testInfo.outputPath('02-terminal-reset.png'),fullPage:true,animations:'disabled'});
  await writeFile(testInfo.outputPath('reset-boundary.json'),JSON.stringify({terminalStatus:410,sameOriginCapabilityReboot:true,activeRunAtRemoval:true,oldContentAbsent:true,browserAuthorizationAbsent:true,browserSelectionAbsent:true,requestsAtReset:atReset,requestsAfterOnline:requests.length,projectStateDirectoryAbsent:true},null,2));
  expect(violations).toEqual([]);expect(errors).toEqual([]);
});
