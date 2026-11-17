import {test,expect} from './fixtures.mjs';
import {writeFile} from 'node:fs/promises';

function longMessage(turn) {
  return `${turn}번째 합성 요구사항입니다.\n`+Array.from({length:36},(_,line)=>`상세 ${line+1}: 가입 안내는 중복 발송을 피하고 전달 이력을 확인할 수 있어야 합니다. 실패한 요청의 처리 기준도 함께 정하고 싶어요.`).join('\n');
}

async function viewportPosition(page) {
  return page.evaluate(()=>{
    const latest=document.querySelector('.chat-turn:last-child').getBoundingClientRect();
    const composer=document.querySelector('#answer-form').getBoundingClientRect();
    const thread=document.querySelector('.chat-thread').getBoundingClientRect();
    const next=document.querySelector('[data-action="generate"]').getBoundingClientRect();
    return {height:innerHeight,scrollY,threadHeight:thread.height,latestTop:latest.top,latestBottom:latest.bottom,composerTop:composer.top,composerBottom:composer.bottom,
      latestVisible:latest.bottom>0&&latest.bottom<=innerHeight,
      composerVisible:composer.top<innerHeight&&composer.bottom>0,
      nextVisible:next.top<innerHeight&&next.bottom>0};
  });
}

test('completed long conversation follows the newest reply and snapshot refresh preserves older reading position',async({server,productPage},testInfo)=>{
  const {page,violations,errors}=productPage;
  const samples=[];
  if(testInfo.project.name==='desktop')await page.setViewportSize({width:1920,height:900});
  // Only this server's temporary copy changes. The shared fixture's slow marker
  // retains real subprocess/HTTP/SSE behavior with a short observation window.
  await server.configureSlowFixture(2);
  await page.goto(server.origin,{waitUntil:'domcontentloaded'});
  await page.locator('#project-intro').fill(longMessage(1));
  await page.locator('#start-form button[type="submit"]').click();
  for(let turn=1;turn<=3;turn++){
    if(turn>1){
      // Filling the composer may scroll it into view before sending. After the
      // click the test never scrolls until it has measured the completed turn.
      await page.locator('#answer-input').fill((turn===3?'SLOW_REQUEST\n':'')+longMessage(turn));
      await page.locator('#answer-form button[type="submit"]').click();
      if(turn===3){
        await expect(page.locator('.pending-turn[aria-busy="true"] [role="status"]')).toHaveText('답변 작성 중…');
        await expect(page.locator('[data-action="cancel"]')).toBeVisible();
        await expect(page.locator('.chat-turn').first()).toContainText('1번째 합성 요구사항입니다.');
        await expect(page.locator('.chat-turn').nth(1)).toContainText('현재 말씀하신 내용은 제안과 미정으로 나누어 기록했습니다.');
        await expect(page.locator('#answer-form')).toBeVisible();
        await page.screenshot({path:testInfo.outputPath('00-inline-pending.png'),fullPage:false,animations:'disabled'});
      }
    }
    await expect.poll(async()=>{
      const snapshot=await server.workspace();
      return snapshot.runs.filter(run=>run.kind==='conversation'&&run.status==='completed').length===turn&&snapshot.active_run===null;
    }).toBe(true);
    await expect(page.locator('.chat-turn')).toHaveCount(turn*2);
    await expect(page.locator('#answer-input')).toBeEnabled();
    await expect(page.locator('.pending-turn')).toHaveCount(0);
    await expect(page.locator('[data-action="cancel"]')).toHaveCount(0);
    await expect(page.locator('[data-action="generate"]')).toHaveClass(/primary/);
    await expect(page.locator('[data-action="generate"]')).toHaveText('다음 단계로 · 초안 만들기');
    await page.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
    const position=await viewportPosition(page);samples.push({turn,...position});
    await page.screenshot({path:testInfo.outputPath(`0${turn}-completed-turn.png`),fullPage:false,animations:'disabled'});
  }
  const geometryPath=testInfo.outputPath('completed-turn-viewports.json');
  await writeFile(geometryPath,JSON.stringify(samples,null,2));
  await testInfo.attach('completed-turn-viewports',{path:geometryPath,contentType:'application/json'});
  expect(samples.every(sample=>sample.threadHeight>sample.height)).toBe(true);
  const widths=await page.evaluate(()=>{
    const main=document.querySelector('#main'),style=getComputedStyle(main);
    return {viewport:innerWidth,document:document.documentElement.scrollWidth,
      available:main.getBoundingClientRect().width-parseFloat(style.paddingLeft)-parseFloat(style.paddingRight),
      content:document.querySelector('#workspace-content').getBoundingClientRect().width,
      composer:document.querySelector('#answer-form').getBoundingClientRect().width};
  });
  await writeFile(testInfo.outputPath('conversation-widths.json'),JSON.stringify(widths,null,2));
  expect(widths.document).toBeLessThanOrEqual(widths.viewport);
  expect(widths.content).toBeGreaterThanOrEqual(widths.available-2);
  expect(widths.composer).toBeGreaterThanOrEqual(widths.available-2);
  if(testInfo.project.name==='desktop'){
    expect(widths.content).toBeGreaterThan(1000);
    await page.screenshot({path:testInfo.outputPath('05-wide-conversation.png'),fullPage:false,animations:'disabled'});
  }

  // Read an earlier turn, then use a public browser connectivity event to ask
  // for a real HTTP snapshot. No product state or DOM is changed by a mock.
  const earlier=page.locator('.chat-turn').nth(2);
  await earlier.evaluate(turn=>turn.scrollIntoView({block:'start'}));
  const before=await earlier.boundingBox();
  expect((await viewportPosition(page)).latestVisible).toBe(false);
  const snapshotResponse=page.waitForResponse(response=>new URL(response.url()).pathname==='/api/v1/workspace');
  await page.locator('#workspace-content').evaluate(content=>new Promise(resolve=>{
    const observer=new MutationObserver(()=>{observer.disconnect();requestAnimationFrame(()=>resolve());});
    observer.observe(content,{childList:true});
    window.dispatchEvent(new Event('online'));
  }));
  expect((await snapshotResponse).ok()).toBe(true);
  const after=await earlier.boundingBox();
  await page.screenshot({path:testInfo.outputPath('04-reading-after-snapshot.png'),fullPage:false,animations:'disabled'});
  expect(Math.abs(after.y-before.y)).toBeLessThanOrEqual(2);
  expect((await viewportPosition(page)).latestVisible).toBe(false);
  expect(violations).toEqual([]);expect(errors).toEqual([]);
  // Assert all turns together so a broken baseline still collects evidence for
  // repeated turns instead of stopping after its first invisible reply.
  expect(samples.map(({turn,latestVisible,composerVisible,nextVisible})=>({turn,latestVisible,composerVisible,nextVisible}))).toEqual([
    {turn:1,latestVisible:true,composerVisible:true,nextVisible:true},
    {turn:2,latestVisible:true,composerVisible:true,nextVisible:true},
    {turn:3,latestVisible:true,composerVisible:true,nextVisible:true}
  ]);
});

test('a conversation after an existing draft opens the new reply and its composer',async({server,productPage},testInfo)=>{
  const {page,violations,errors}=productPage;
  await page.goto(server.origin,{waitUntil:'domcontentloaded'});
  await page.locator('#project-intro').fill('가입 안내 알림을 보내는 작은 서비스를 만들고 싶어요.');
  await page.locator('#start-form button[type="submit"]').click();
  await expect(page.locator('.chat-turn')).toHaveCount(2);
  await page.locator('[data-action="generate"]').click();
  await expect.poll(async()=>{
    const snapshot=await server.workspace();
    return snapshot.sessions[0]?.drafts[0]?.reviewed&&snapshot.active_run===null;
  }).toBe(true);
  await expect(page.locator('.markdown')).toContainText('# 알림 서비스 아키텍처');
  await page.locator('[data-action="interview"]').click();
  await expect(page.locator('.chat-turn')).toHaveCount(2);
  await page.locator('#answer-input').fill(longMessage(2));
  await page.locator('#answer-form button[type="submit"]').click();
  await expect.poll(async()=>{
    const snapshot=await server.workspace();
    return snapshot.runs.filter(run=>run.kind==='conversation'&&run.status==='completed').length===2&&snapshot.active_run===null;
  }).toBe(true);
  await expect(page.locator('.chat-turn')).toHaveCount(4);
  await expect(page.locator('#answer-input')).toBeEnabled();
  await expect(page.locator('[data-action="generate"]')).toHaveClass(/primary/);
  await expect(page.locator('[data-action="generate"]')).toHaveText('다음 단계로 · 초안 다시 정리');
  await page.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
  const position=await viewportPosition(page);
  await writeFile(testInfo.outputPath('conversation-after-draft-viewport.json'),JSON.stringify(position,null,2));
  await page.screenshot({path:testInfo.outputPath('01-conversation-after-draft.png'),fullPage:false,animations:'disabled'});
  expect(position.threadHeight).toBeGreaterThan(position.height);
  expect(position.latestVisible).toBe(true);expect(position.composerVisible).toBe(true);
  expect(position.nextVisible).toBe(true);
  expect((await server.workspace()).sessions[0].drafts).toHaveLength(1);
  expect(violations).toEqual([]);expect(errors).toEqual([]);
});
