import {test,expect} from './fixtures.mjs';
import {writeFile} from 'node:fs/promises';
import {join} from 'node:path';

const preview=page=>page.locator('#file-content > [data-file-preview]:not([hidden])');
const tab=(page,path)=>page.locator(`#workspace-tabs [role="tab"][data-tab-path="${path}"]`);
const latch=()=>{let release;const promise=new Promise(resolve=>{release=resolve;});return {promise,release};};
const markdown=title=>`# ${title}\n\n검증용 임시 파일입니다. 실제 프로젝트 자료나 모델 응답이 아닙니다.\n\n\`\`\`state\n<s> --> (Ready)\n(Ready) --> (Running) : 실행\n(Running) --> (Ready) : 완료\n\`\`\`\n`;

async function openTreeFile(page,path){
  const button=page.locator(`[data-file-path="${path}"]`);
  await expect(button).toHaveCount(1);
  if(!await button.isVisible())await page.locator('#sidebar-toggle').click();
  await button.click();
}

function onlyReadRequests(page){
  const writes=[];
  page.on('request',request=>{
    const path=new URL(request.url()).pathname;
    if(path.startsWith('/api/v1/')&&request.method()!=='GET')writes.push(`${request.method()} ${path}`);
  });
  return writes;
}

async function importSettled(page,path){
  await page.evaluate(async modulePath=>{
    await import(modulePath);
    await new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve)));
  },path);
}

test('same-file refresh ignores an old delayed renderer import and shows the new file',async({server,productPage},testInfo)=>{
  const {page,errors,violations}=productPage;
  const writes=onlyReadRequests(page);
  const path='viewer-race-refresh.md';
  await writeFile(join(server.project,path),markdown('Old file revision'));
  const rendererSeen=latch(),rendererRelease=latch(),refreshSeen=latch(),refreshRelease=latch();
  let reads=0;
  await page.route(`${server.origin}/document-renderer.js`,async route=>{
    rendererSeen.release();await rendererRelease.promise;
    if(!page.isClosed())await route.continue();
  });
  await page.route(`${server.origin}/api/v1/files/read?**`,async route=>{
    if(new URL(route.request().url()).searchParams.get('path')===path&&++reads===2){refreshSeen.release();await refreshRelease.promise;}
    if(!page.isClosed())await route.continue();
  });
  try{
    await page.goto(server.origin,{waitUntil:'domcontentloaded'});
    await openTreeFile(page,path);
    await rendererSeen.promise;
    await expect(preview(page)).toHaveCount(1);
    const oldNode=await preview(page).elementHandle();
    await page.locator('#file-reload').click();
    await refreshSeen.promise;
    await writeFile(join(server.project,path),markdown('Newest file revision'));
    rendererRelease.release();
    await importSettled(page,'/document-renderer.js');
    // The stale import must not dereference the refresh's absent/new tab.node.
    expect(errors).toEqual([]);
    refreshRelease.release();
    await expect(preview(page)).toHaveAttribute('data-render-state','ready');
    expect(await page.locator('#main').evaluate(node=>node.getBoundingClientRect().width)).toBeGreaterThan(200);
    await expect(preview(page)).toBeVisible();
    await expect(preview(page).locator('h1')).toHaveText('Newest file revision');
    await expect(preview(page)).not.toContainText('Old file revision');
    await expect(preview(page).locator('figure[data-diagram="state"]')).toHaveAttribute('data-diagram-status','ready');
    await expect(preview(page).locator('.document-diagram-controls')).toHaveCount(1);
    expect(await oldNode.evaluate(node=>node.isConnected)).toBe(false);
    await page.screenshot({path:testInfo.outputPath('refresh-after-delayed-import.png'),fullPage:true,animations:'disabled'});
    expect(writes).toEqual([]);expect(errors).toEqual([]);expect(violations).toEqual([]);
  }finally{rendererRelease.release();refreshRelease.release();}
});

test('a diagram loading behind another file and chat settles when its cached tab returns',async({server,productPage},testInfo)=>{
  const {page,errors,violations}=productPage;
  const writes=onlyReadRequests(page);
  const diagramPath='viewer-race-diagram.md',otherPath='viewer-race-other.txt';
  await writeFile(join(server.project,diagramPath),markdown('Diagram file'));
  await writeFile(join(server.project,otherPath),'Other file content. No model or transcription is used.\n');
  const mermaidSeen=latch(),mermaidRelease=latch();
  await page.route(`${server.origin}/vendor/mermaid.js`,async route=>{
    mermaidSeen.release();await mermaidRelease.promise;
    if(!page.isClosed())await route.continue();
  });
  try{
    await page.goto(server.origin,{waitUntil:'domcontentloaded'});
    await openTreeFile(page,diagramPath);
    await mermaidSeen.promise;
    await expect(preview(page)).toHaveAttribute('data-render-state','loading');
    const cachedNode=await preview(page).elementHandle();
    await openTreeFile(page,otherPath);
    await expect(preview(page)).toContainText('Other file content.');
    await tab(page,otherPath).click();
    await tab(page,'chat').click();
    await expect(page.locator('#chat-workspace')).toBeVisible();
    await expect(page.locator('#file-workspace')).toBeHidden();
    // Resolve while the source tab is inactive. Returning before this point
    // would hide the old disconnected-container/current() regression.
    mermaidRelease.release();
    await importSettled(page,'/vendor/mermaid.js');
    await tab(page,diagramPath).click();
    await expect(preview(page)).toHaveAttribute('data-render-state','ready');
    await expect(preview(page).locator('h1')).toHaveText('Diagram file');
    await expect(preview(page).locator('figure[data-diagram="state"]')).toHaveAttribute('data-diagram-status','ready');
    await expect(preview(page).locator('.document-diagram-controls')).toHaveCount(1);
    expect(await preview(page).evaluate((node,cached)=>node===cached,cachedNode)).toBe(true);
    expect(await page.locator('#main').evaluate(node=>node.getBoundingClientRect().width)).toBeGreaterThan(200);
    expect(await preview(page).locator('.document-diagram-viewport').evaluate(node=>node.clientWidth)).toBeGreaterThan(0);
    // A hidden connected camera measures on its first visible ResizeObserver
    // delivery; semantic render-ready can precede that paint by one frame.
    await expect.poll(()=>preview(page).locator('.document-diagram-viewport').evaluate(node=>node.clientHeight)).toBeGreaterThan(0);
    const fit=await preview(page).locator('.document-diagram-viewport').evaluate(node=>({width:node.clientWidth,scroll:node.scrollWidth,height:node.clientHeight,mode:node.dataset.mode}));
    expect(fit.width).toBeGreaterThan(0);expect(fit.height).toBeGreaterThan(0);expect(fit.scroll).toBeLessThanOrEqual(fit.width+1);expect(fit.mode).toBe('fit');
    await page.screenshot({path:testInfo.outputPath('cached-diagram-after-tab-return.png'),fullPage:true,animations:'disabled'});
    expect(writes).toEqual([]);expect(errors).toEqual([]);expect(violations).toEqual([]);
  }finally{mermaidRelease.release();}
});
