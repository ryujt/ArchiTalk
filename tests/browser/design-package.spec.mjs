import {test,expect} from './fixtures.mjs';
import {readFile,writeFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {basename,dirname,join} from 'node:path';

const fixture=JSON.parse(await readFile(new URL('../fixtures/design-package.json',import.meta.url),'utf8'));
const catalog=[
  ['architecture','아키텍처-설계.md'],['drivers','요구사항과-아키텍처-드라이버.md'],
  ['quality_scenarios','품질속성과-검증시나리오.md'],['alternatives','설계대안과-트레이드오프.md'],
  ['decisions','아키텍처-결정기록.md'],['delivery','구현과-운영검증.md'],
  ['navigation','네비게이션.md'],['jobflow','잡플로우.md'],['state','상태도.md'],['screen_layout','화면설계.md']
];
const intro='패키지 QA 합성 근거: 조회 결과가 선택한 자료와 일치해야 합니다.';
const digest=bytes=>createHash('sha256').update(bytes).digest('hex');

async function ready(page,kind){
  await expect(page.locator('#document-kind-picker')).toHaveValue(kind);
  await expect(page.locator('.markdown')).toHaveAttribute('data-document-kind',kind);
  await expect(page.locator('.markdown')).toHaveAttribute('data-render-state','ready');
  expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true);
}
async function choose(page,kind){await page.locator('#document-kind-picker').selectOption(kind);await ready(page,kind);}
async function focusedHeading(page,name){
  const heading=page.locator('.markdown').getByRole('heading',{name,exact:true});
  await expect(heading).toBeFocused();
  const box=await heading.boundingBox();
  expect(box.y).toBeGreaterThanOrEqual(0);expect(box.y).toBeLessThan(await page.evaluate(()=>innerHeight));
}
async function picture(page,testInfo,name){await page.screenshot({path:testInfo.outputPath(`${name}.png`),fullPage:true,animations:'disabled'});}

test('a connected Method-R package switches, follows anchors, survives snapshots and saves all ten files',async({server,productPage},testInfo)=>{
  const {page,violations,errors}=productPage;
  const candidate=structuredClone(fixture);
  const nfdFile=encodeURIComponent('화면설계.md'.normalize('NFD'));
  const nfdAnchor=encodeURIComponent('한국어-앵커'.normalize('NFD'));
  candidate.markdown+='\n## 링크 검증\n\n[같은 문서 이동](#링크-검증)\n\n[중복 제목 이동](#중복-제목-1)\n\n'+
    `검증 입력: <a href="${nfdFile}#${nfdAnchor}">분해된 한국어 파일과 앵커 이동</a>\n\n`+
    '검증 입력: <a href="../outside.md">허용하지 않은 파일 이동</a>\n\n### 중복 제목\n\n첫 번째 대상입니다.\n\n### 중복 제목\n\n두 번째 대상입니다.\n';
  candidate.documents.find(document=>document.kind==='screen_layout').markdown+='\n## 한국어 앵커\n\n한글 정규화가 같은 문서와 앵커를 선택해야 하며 파일 GET 요청을 만들지 않는다.\n';
  await server.configurePackageFixture(candidate);
  const markdownRequests=[];
  page.on('request',request=>{if(/\.md$/i.test(new URL(request.url()).pathname))markdownRequests.push('Markdown file GET attempted');});
  await page.goto(server.origin,{waitUntil:'domcontentloaded'});
  await page.locator('#project-intro').fill(intro);await page.locator('#start-form button[type=submit]').click();
  await expect(page.locator('.chat-turn')).toHaveCount(2);
  await page.locator('[data-action=generate]').click();
  await expect.poll(async()=>{const w=await server.workspace();return w.sessions[0]?.drafts[0]?.reviewed&&w.active_run===null;}).toBe(true);
  await ready(page,'architecture');
  const before=await server.workspace(),draft=before.sessions[0].drafts[0];
  expect(before.document_types.map(({kind,filename})=>[kind,filename])).toEqual(catalog);
  expect(draft.documents).toHaveLength(9);
  expect(await page.locator('#document-kind-picker option').evaluateAll(options=>options.map(option=>option.value))).toEqual(catalog.map(([kind])=>kind));
  await expect(page.locator('.markdown')).toContainText(intro);
  for(const name of ['Method-R macro design','Method-R system design','Method-R module design','Method-R detailed design'])await expect(page.locator('.markdown').getByRole('heading',{name,exact:true})).toBeVisible();
  await expect(page.locator('[data-action=approve]')).toHaveText('전체 10개 문서를 검토본으로 확정');
  await expect(page.locator('[data-action=approve]')).toBeDisabled();
  await picture(page,testInfo,'01-principal');

  for(const [kind] of catalog.slice(1)){
    await choose(page,'architecture');
    const filename=catalog.find(([value])=>value===kind)[1];
    await page.locator('.markdown table a').filter({hasText:({drivers:'드라이버',quality_scenarios:'품질',alternatives:'대안',decisions:'결정',delivery:'구현·검증',navigation:'Navigation',jobflow:'Jobflow',state:'State',screen_layout:'Screen Layout'})[kind]}).click();
    await ready(page,kind);
    const source=candidate.documents.find(document=>document.kind===kind).markdown;
    await expect(page.locator('.markdown h1')).toHaveText(source.match(/^# (.+)$/m)[1]);
    await expect(page.locator('.doc-label')).toContainText(filename);
    if(['drivers','quality_scenarios','alternatives','decisions','delivery'].includes(kind)){
      const own={drivers:'DRV',quality_scenarios:'QA',alternatives:'ALT',decisions:'ADR',delivery:'VAL'}[kind];
      await expect(page.locator('.markdown h3')).toContainText(`${own}-001`);
      for(const ref of ({drivers:['QA-001','ADR-001'],quality_scenarios:['DRV-001'],alternatives:['DRV-001','QA-001'],decisions:['DRV-001','QA-001','ALT-001'],delivery:['QA-001','ADR-001']})[kind])await expect(page.locator('.markdown')).toContainText(ref);
    }
  }
  await choose(page,'quality_scenarios');
  await page.locator('.markdown').evaluate(node=>{window.__packagePreviewNode=node;});
  const response=page.waitForResponse(response=>response.url()===`${server.origin}/api/v1/workspace`);
  await page.locator('#workspace-content').evaluate(content=>new Promise(resolve=>{
    const observer=new MutationObserver(()=>{observer.disconnect();requestAnimationFrame(resolve);});observer.observe(content,{childList:true});window.dispatchEvent(new Event('online'));
  }));await response;await ready(page,'quality_scenarios');
  expect(await page.locator('.markdown').evaluate(node=>node===window.__packagePreviewNode)).toBe(true);

  await choose(page,'navigation');await page.locator('.markdown a').filter({hasText:'상세 배치'}).click();
  await ready(page,'screen_layout');await focusedHeading(page,'Resource Detail');
  // Earlier documents can link a later file, whose heading anchors do not exist
  // yet in the staged generation. Backward links still test exact anchor focus.
  expect(draft.documents.find(document=>document.kind==='screen_layout').markdown).toContain('](네비게이션.md)');
  await expect(page.locator('.markdown a').filter({hasText:'화면 흐름'})).toHaveAttribute('href','#');
  await page.locator('.markdown a').filter({hasText:'화면 흐름'}).click();
  await ready(page,'navigation');await expect(page.locator('.doc-label')).toContainText('네비게이션.md');
  await choose(page,'jobflow');
  expect(draft.documents.find(document=>document.kind==='jobflow').markdown).toContain('](아키텍처-설계.md)');
  await expect(page.locator('.markdown a').filter({hasText:'부모 계약'})).toHaveAttribute('href','#');
  await page.locator('.markdown a').filter({hasText:'부모 계약'}).click();
  await ready(page,'architecture');await expect(page.locator('.markdown').getByRole('heading',{name:'Method-R module design',exact:true})).toHaveCount(1);
  await page.locator('.markdown a').filter({hasText:'같은 문서 이동'}).click();await focusedHeading(page,'링크 검증');
  await page.locator('.markdown a').filter({hasText:'중복 제목 이동'}).click();
  await expect(page.locator('.markdown h3').filter({hasText:'중복 제목'}).nth(1)).toBeFocused();
  await expect(page.locator('.markdown a').filter({hasText:'허용하지 않은 파일 이동'})).not.toHaveAttribute('href');
  await page.locator('.markdown a').filter({hasText:'분해된 한국어 파일과 앵커 이동'}).click();
  await ready(page,'screen_layout');await focusedHeading(page,'한국어 앵커');
  expect(new URL(page.url()).pathname).toBe('/');expect(new URL(page.url()).hash).toBe('');
  await picture(page,testInfo,'02-linked-layout');

  // Selecting a companion affects only preview; save still publishes the package.
  await choose(page,'decisions');await page.locator('[data-action=save-preview]').click();
  await expect(page.locator('.plan-file')).toHaveCount(10);
  await expect(page.locator('[data-action=export-apply]')).toHaveText('전체 10개를 초안으로 저장');
  await page.locator('[data-action=export-apply]').click();
  await expect(page.getByRole('heading',{name:'설계 문서를 저장했어요',exact:true})).toBeVisible();
  const saved=await server.workspace(),entry=saved.exports.at(-1);
  expect(entry.status).toBe('completed');expect(entry.result.paths.map(path=>basename(path))).toEqual(catalog.map(([,name])=>name));
  expect(entry.source_document_hash).toBe(draft.hash);
  const hashes=[];
  for(const path of entry.result.paths){
    const name=basename(path),bytes=await server.savedFile(path),planned=entry.plan.files.find(file=>file.relative_path===name);
    expect(bytes).toBe(planned.content);expect(digest(bytes)).toBe(planned.hash);hashes.push({name,hash:planned.hash});
    expect(bytes).toMatch(/^---\ntitle:/);expect(bytes).toMatch(/^style: smartmd$/m);
  }
  const manifest=JSON.parse(await server.savedFile(join(dirname(entry.result.paths[0]),'.architalk-export.json')));
  expect(manifest.files.map(file=>({name:file.path,hash:file.hash}))).toEqual(hashes);
  await writeFile(testInfo.outputPath('published-package.json'),JSON.stringify({files:hashes,sourceHash:entry.source_document_hash},null,2));
  await picture(page,testInfo,'03-saved-package');
  const runCount=saved.runs.length;
  await page.reload({waitUntil:'domcontentloaded'});
  await page.getByRole('button',{name:'초안 먼저 살펴보기',exact:true}).click();await ready(page,'architecture');
  await choose(page,'decisions');await expect(page.locator('.markdown h3')).toContainText('ADR-001');
  expect((await server.workspace()).runs).toHaveLength(runCount);
  expect(markdownRequests).toEqual([]);expect(violations).toEqual([]);expect(errors).toEqual([]);
});
