import {test,expect} from './fixtures.mjs';
import {createHash} from 'node:crypto';
import {basename} from 'node:path';

async function noOverflow(page) {
  expect(await page.evaluate(()=>document.documentElement.scrollWidth<=window.innerWidth)).toBe(true);
}
async function screenshot(page,testInfo,name) {
  await page.screenshot({path:testInfo.outputPath(`${name}.png`),fullPage:true,animations:'disabled'});
}
async function modalFocusStaysInside(page) {
  const focus=()=>page.evaluate(()=>({inside:Boolean(document.activeElement?.closest('dialog[open]')),browserChrome:!document.hasFocus()&&document.activeElement===document.body}));
  expect((await focus()).inside).toBe(true);
  // Native modal dialogs keep background document controls inert; Chrome may
  // still move keyboard focus into browser chrome at the traversal boundary.
  // That is allowed only while the document itself has no focus, and the next
  // traversal must return to the open dialog, never a background control.
  for(const key of [...Array(30).fill('Tab'),...Array(3).fill('Shift+Tab')]){
    await page.keyboard.press(key);
    const state=await focus();expect(state.inside||state.browserChrome).toBe(true);
    if(state.browserChrome){await page.keyboard.press(key);expect((await focus()).inside).toBe(true);}
  }
  await page.locator('.app-header [data-action="settings"]').evaluate(element=>element.focus());
  expect((await focus()).inside).toBe(true);
}

test('real local UI persists a draft, next-run settings and recovery without model calls',async({server,productPage},testInfo)=>{
  const {page,violations,errors}=productPage;
  const mobile=testInfo.project.name==='mobile';
  await page.setViewportSize(mobile?{width:390,height:844}:{width:1365,height:900});
  await page.goto(server.origin,{waitUntil:'domcontentloaded'});
  await expect(page.getByRole('heading',{name:'어떤 서비스를 만들고 싶으세요?'})).toBeVisible();
  expect(await page.evaluate(()=>location.hash===''&&Boolean(sessionStorage.getItem('architalk.authorization')))).toBe(true);
  await noOverflow(page);await screenshot(page,testInfo,'01-welcome');
  await expect(page.locator('#aux-content')).not.toContainText('아키텍처 설계');
  const intro='가입 안내 알림을 보내고 싶어요. 규모는 아직 모르겠어요. <img src=x onerror="window.architalkProbe=true">';
  await page.getByLabel('프로젝트를 한두 문장으로 소개해 주세요').fill(intro);
  await page.getByRole('button',{name:'대화 시작',exact:true}).click();
  await expect.poll(async()=>{
    const snapshot=await server.workspace();
    return snapshot.runs.some(run=>run.kind==='conversation'&&run.status==='completed');
  }).toBe(true);
  await expect(page.locator('.chat-thread')).toContainText('현재 말씀하신 내용은 제안과 미정으로 나누어 기록했습니다.');
  await expect(page.locator('.chat-message img')).toHaveCount(0);
  expect(await page.evaluate(()=>Boolean(window.architalkProbe))).toBe(false);
  const before=await server.workspace();
  expect(before.sessions).toHaveLength(1);
  expect(before.sessions[0].drafts).toHaveLength(0);
  await page.getByRole('button',{name:'다음 단계로 · 초안 만들기',exact:true}).click();
  await expect.poll(async()=>{
    const snapshot=await server.workspace();
    return snapshot.runs.some(run=>run.kind==='critic'&&run.status==='completed')&&snapshot.sessions[0].drafts[0]?.reviewed;
  }).toBe(true);
  await expect(page.locator('.markdown').getByRole('heading',{name:'알림 서비스 아키텍처',exact:true})).toBeVisible();
  await expect(page.locator('[data-action="approve"]')).toBeDisabled();
  await noOverflow(page);await screenshot(page,testInfo,'02-review');
  await page.locator('[data-action="save-preview"]').click();
  await expect(page.getByRole('dialog')).toBeVisible();
  await expect(page.locator('[data-action="export-apply"]')).toBeEnabled();
  await expect(page.locator('[data-action="approve-export"]')).toHaveCount(0);
  await expect(page.locator('.plan-file')).toHaveCount(10);
  await expect(page.locator('.plan-file').first()).toContainText('아키텍처-설계.md');
  await noOverflow(page);
  await page.getByRole('button',{name:'전체 10개를 초안으로 저장',exact:true}).click();
  await expect(page.getByRole('heading',{name:'설계 문서를 저장했어요'})).toBeVisible();
  const saved=await server.workspace();
  const exported=saved.exports.at(-1);
  expect(exported.status).toBe('completed');expect(exported.result.status).toBe('completed');
  expect(exported.result.paths).toHaveLength(10);
  expect(exported.plan.files).toHaveLength(10);
  expect(exported.result.paths.map(path=>basename(path))).toEqual(saved.document_types.map(type=>type.filename));
  const stored=new Map();
  for(const path of exported.result.paths){
    const name=basename(path),bytes=await server.savedFile(path);stored.set(name,bytes);
    expect(bytes.startsWith('---\n')).toBe(true);expect(bytes).toMatch(/^style:\s*smartmd$/m);
    const planned=exported.plan.files.find(file=>file.relative_path===name);
    expect(bytes).toBe(planned.content);
    expect(createHash('sha256').update(bytes).digest('hex')).toBe(planned.hash);
  }
  const bytes=stored.get('아키텍처-설계.md');
  for(const section of ['Method-R macro design','Method-R system design','Method-R module design','Method-R detailed design'])expect(bytes).toContain(`## ${section}`);
  for(const type of saved.document_types.slice(1))expect(bytes).toContain(`](${type.filename})`);
  for(const [name,language] of [['네비게이션.md','navigation'],['잡플로우.md','jobflow'],['상태도.md','state'],['화면설계.md','layout']])expect(stored.get(name)).toContain('```'+language+'\n');
  expect(stored.get('네비게이션.md')).toContain('화면설계.md#resource-list');
  expect(bytes).toContain('# 알림 서비스 아키텍처');expect(bytes).toContain('내용 상태: draft');
  expect(saved.sessions[0].content_status).not.toBe('approved');
  await screenshot(page,testInfo,'03-saved');
  const originalRuns=structuredClone(saved.runs);
  await page.getByRole('button',{name:'도구 설정',exact:true}).click();
  await expect(page.getByRole('dialog')).toBeVisible();
  await expect(page.locator('#claude-executable')).toHaveAttribute('readonly','');
  await expect(page.locator('#codex-profile')).toHaveAttribute('readonly','');
  await modalFocusStaysInside(page);await noOverflow(page);
  await page.locator('#claude-model').fill('fixture-browser-model');
  await page.locator('#claude-effort').fill('medium');
  await page.locator('#design-backend').selectOption('claude');
  await screenshot(page,testInfo,'04-settings');
  await page.getByRole('button',{name:'다음 작업부터 적용',exact:true}).click();
  await expect(page.getByRole('dialog')).not.toBeVisible();
  await expect(page.getByRole('button',{name:'도구 설정',exact:true})).toBeFocused();
  const configured=await server.workspace();
  expect(configured.settings_revision).toBeGreaterThan(saved.settings_revision);
  expect(configured.settings.ai.backends.claude.model).toBe('fixture-browser-model');
  expect(configured.settings.ai.backends.claude.effort).toBe('medium');
  expect(configured.runs).toEqual(originalRuns);
  await page.getByRole('button',{name:'도구 설정',exact:true}).click();
  await expect(page.getByRole('dialog')).toBeVisible();await page.keyboard.press('Escape');
  await expect(page.getByRole('dialog')).not.toBeVisible();
  await expect(page.getByRole('button',{name:'도구 설정',exact:true})).toBeFocused();
  await page.getByRole('button',{name:'문서 살펴보기',exact:true}).click();
  await page.getByRole('button',{name:'현재 답변으로 다시 정리',exact:true}).click();
  await expect.poll(async()=>{
    const snapshot=await server.workspace();
    return snapshot.sessions[0].drafts.length===2&&snapshot.sessions[0].drafts[1].reviewed&&snapshot.active_run===null;
  }).toBe(true);
  const next=await server.workspace();
  const nextDesign=next.runs.filter(run=>run.kind==='design').at(-1);
  expect(nextDesign.effective_settings.model).toBe('fixture-browser-model');
  expect(nextDesign.effective_settings.effort).toBe('medium');
  expect(next.runs.slice(0,originalRuns.length)).toEqual(originalRuns);
  const count=next.runs.length;
  await page.reload({waitUntil:'domcontentloaded'});
  await expect(page.getByRole('heading',{name:'남겨 둔 설계에서 이어가세요'})).toBeVisible();
  expect(await page.evaluate(()=>location.hash==='')).toBe(true);
  expect((await server.workspace()).runs).toHaveLength(count);
  await noOverflow(page);await screenshot(page,testInfo,'05-recovery');
  await page.getByRole('button',{name:'초안 먼저 살펴보기',exact:true}).click();
  await expect(page.locator('.markdown').getByRole('heading',{name:'알림 서비스 아키텍처',exact:true})).toBeVisible();
  if(mobile){
    await page.getByRole('button',{name:'설계 메모',exact:true}).click();
    await expect(page.locator('#aux-panel')).toBeVisible();await noOverflow(page);
    await page.setViewportSize({width:768,height:1024});
    await expect(page.locator('#session-switcher')).toBeVisible();
    await expect(page.locator('.sidebar')).not.toBeVisible();await noOverflow(page);
    await screenshot(page,testInfo,'06-tablet');
  }
  expect(violations).toEqual([]);expect(errors).toEqual([]);
});
