import {test,expect} from '@playwright/test';
import {createServer} from 'node:http';
import {readFile,writeFile} from 'node:fs/promises';
import {resolve,extname} from 'node:path';

// A test-owned static loopback page imports the real renderer. No user server,
// project database, installed CLI, model, browser profile or capability is used.
const webRoot=resolve(new URL('../../web/',import.meta.url).pathname);
let server,origin;
const stateSource=['<s> --> (LongState_00_SelectedDocument)',...Array.from({length:11},(_,index)=>`(LongState_${String(index).padStart(2,'0')}_SelectedDocument) --> (LongState_${String(index+1).padStart(2,'0')}_SelectedDocument) : 완료 결과 확인`) ].join('\n');
const markdown=`# Diagram viewport\n\n\`\`\`state\n${stateSource}\n\`\`\`\n`;
const csp="default-src 'none'; script-src 'self'; style-src 'self' 'unsafe-inline'; connect-src 'self'; img-src 'self' data:; base-uri 'none'; frame-ancestors 'none'; form-action 'none'";

test.beforeAll(async()=>{
  server=createServer(async(request,response)=>{
    response.setHeader('Content-Security-Policy',csp);
    response.setHeader('X-Content-Type-Options','nosniff');
    const pathname=new URL(request.url,'http://127.0.0.1').pathname;
    if(pathname==='/'){
      response.setHeader('Content-Type','text/html; charset=utf-8');
      response.end('<!doctype html><html lang="ko"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><link rel="stylesheet" href="/document.css"><style>body{margin:16px;background:#f7f8f5}main{max-width:860px;min-width:0;margin:auto}#document{min-width:0}button{font:inherit}</style><main><div id="document"></div></main></html>');return;
    }
    let path;
    try{path=resolve(webRoot,`.${decodeURIComponent(pathname)}`);}catch{response.writeHead(400).end();return;}
    if(!path.startsWith(`${webRoot}/`)){response.writeHead(404).end();return;}
    try{
      const source=await readFile(path);
      response.setHeader('Content-Type',extname(path)==='.css'?'text/css; charset=utf-8':'text/javascript; charset=utf-8');
      response.end(source);
    }catch{response.writeHead(404).end();}
  });
  await new Promise(resolveListening=>server.listen(0,'127.0.0.1',resolveListening));
  origin=`http://127.0.0.1:${server.address().port}`;
});
test.afterAll(async()=>{server?.closeAllConnections();await new Promise(resolveClosed=>server?.close(resolveClosed));});

async function render(page,source=markdown){
  const errors=[],requests=[];
  page.on('pageerror',error=>errors.push(error.message));
  await page.route('**/*',route=>{
    if(new URL(route.request().url()).origin===origin)return route.continue();
    requests.push('external request');return route.abort('blockedbyclient');
  });
  await page.goto(origin,{waitUntil:'domcontentloaded'});
  await page.evaluate(async text=>{
    const {renderDocument}=await import('/document-renderer.js');
    await renderDocument(document.querySelector('#document'),text);
  },source);
  await expect(page.locator('#document')).toHaveAttribute('data-render-state','ready');
  return {errors,requests};
}

async function geometry(figure){
  return figure.evaluate(element=>{
    const viewport=element.querySelector('.document-diagram-viewport');
    const content=element.querySelector('.document-diagram-content');
    const visual=content?.getBoundingClientRect();
    const box=viewport?.getBoundingClientRect();
    return {figureWidth:element.clientWidth,figureScroll:element.scrollWidth,pageWidth:document.documentElement.scrollWidth,windowWidth:innerWidth,
      viewportWidth:viewport?.clientWidth,viewportScroll:viewport?.scrollWidth,viewportHeight:viewport?.clientHeight,
      content:visual?{left:visual.left,right:visual.right,top:visual.top,bottom:visual.bottom,width:visual.width,height:visual.height}:null,
      box:box?{left:box.left,right:box.right,top:box.top,bottom:box.bottom}:null,
      zoom:viewport?.dataset.zoom,mode:viewport?.dataset.mode};
  });
}

function expectNoScroll(observed){
  expect(observed.figureScroll).toBeLessThanOrEqual(observed.figureWidth+1);
  expect(observed.pageWidth).toBeLessThanOrEqual(observed.windowWidth);
  expect(observed.viewportScroll).toBeLessThanOrEqual(observed.viewportWidth+1);
}
function expectFits(observed){
  expectNoScroll(observed);
  expect(observed.content.left).toBeGreaterThanOrEqual(observed.box.left-1);
  expect(observed.content.right).toBeLessThanOrEqual(observed.box.right+1);
  expect(observed.content.top).toBeGreaterThanOrEqual(observed.box.top-1);
  expect(observed.content.bottom).toBeLessThanOrEqual(observed.box.bottom+1);
}

test('wide state diagram starts fully fitted without a horizontal scrollbar',async({page},testInfo)=>{
  const safety=await render(page);
  const figure=page.locator('figure.document-diagram');
  await expect(figure).toHaveAttribute('data-diagram-status','ready');
  const observed=await geometry(figure);
  await writeFile(testInfo.outputPath('wide-state.json'),JSON.stringify(observed,null,2));
  await figure.screenshot({path:testInfo.outputPath('wide-state.png'),animations:'disabled'});
  expectFits(observed);
  await expect(figure.getByRole('button',{name:'도표 확대',exact:true})).toBeVisible();
  await expect(figure.getByRole('button',{name:'전체 보기',exact:true})).toBeVisible();
  expect(safety.errors).toEqual([]);expect(safety.requests).toEqual([]);
});

test('zoom drag keyboard fit and resized container keep the diagram inside its box',async({page},testInfo)=>{
  const safety=await render(page);
  const figure=page.locator('figure.document-diagram');
  const viewport=figure.locator('.document-diagram-viewport');
  const initial=await geometry(figure);
  await figure.getByRole('button',{name:'도표 확대',exact:true}).click();
  await figure.getByRole('button',{name:'도표 확대',exact:true}).click();
  const zoomed=await geometry(figure);
  expect(Number(zoomed.zoom)).toBeGreaterThan(1);expectNoScroll(zoomed);
  expect(zoomed.content.width).toBeGreaterThan(initial.content.width);
  const box=await viewport.boundingBox();
  await page.mouse.move(box.x+box.width/2,box.y+box.height/2);
  await page.mouse.down();await page.mouse.move(box.x+box.width/2+60,box.y+box.height/2,{steps:8});await page.mouse.up();
  const dragged=await geometry(figure);
  expect(dragged.content.left).not.toBe(zoomed.content.left);expectNoScroll(dragged);
  await viewport.focus();await page.keyboard.press('ArrowLeft');
  const keyed=await geometry(figure);
  expect(keyed.content.left).toBeLessThan(dragged.content.left);
  await page.evaluate(()=>document.querySelector('main').style.maxWidth='310px');
  await expect.poll(async()=>Number((await geometry(figure)).viewportWidth)).toBeLessThanOrEqual(310);
  expectNoScroll(await geometry(figure));
  await figure.getByRole('button',{name:'전체 보기',exact:true}).click();
  expectFits(await geometry(figure));
  await page.evaluate(()=>document.querySelector('main').style.maxWidth='860px');
  await expect.poll(async()=> (await geometry(figure)).content.right-(await geometry(figure)).box.right).toBeLessThanOrEqual(1);
  expectFits(await geometry(figure));
  await figure.screenshot({path:testInfo.outputPath('zoom-reset.png'),animations:'disabled'});
  expect(safety.errors).toEqual([]);expect(safety.requests).toEqual([]);
});

test('every SVG language and layout uses the same safe fitted viewport',async({page},testInfo)=>{
  const source=['# All diagrams',
    '```mermaid\nflowchart LR\nA[Beginning] --> B[Complete]\n```',
    '```navigation\nResourceList --> ResourceDetail : 열기\nResourceDetail --> ResourceList : 복귀\n```',
    '```jobflow\norchestrator: Reader\nObject: Reader, Repository\nReader.Open --> Repository.Load\nRepository.Load.found --> Reader.Show\n```',
    '```state\n<s> --> (Ready)\n(Ready) --> (Running) : 실행\n(Running) --> (Ready) : 완료\n```',
    '```layout\nScreen > Navigation : 25, Content : 75\nContent V Input, Result, Feedback\n```',
    '```mermaid\nthis is invalid\n```',
    '<button data-action="approve" onclick="window.diagramProbe=true">untrusted</button>',
  ].join('\n\n');
  const safety=await render(page,source);
  const figures=page.locator('figure.document-diagram');
  await expect(figures).toHaveCount(6);
  for(const language of ['mermaid','navigation','jobflow','state','layout']){
    const figure=page.locator(`figure.document-diagram[data-diagram="${language}"]`).first();
    await expect(figure).toHaveAttribute('data-diagram-status','ready');
    expectFits(await geometry(figure));
    await figure.getByRole('button',{name:'도표 확대',exact:true}).click();
    expectNoScroll(await geometry(figure));
    await figure.getByRole('button',{name:'전체 보기',exact:true}).click();
    expectFits(await geometry(figure));
  }
  await expect(figures.last()).toHaveAttribute('data-diagram-status','error');
  await expect(figures.last().locator('.document-diagram-error')).toBeVisible();
  const unsafe=await page.locator('#document').evaluate(element=>({
    handlers:[...element.querySelectorAll('*')].some(node=>[...node.attributes].some(attribute=>attribute.name.startsWith('on'))),
    actions:element.querySelectorAll('[data-action]').length,probe:Boolean(window.diagramProbe),
  }));
  expect(unsafe).toEqual({handlers:false,actions:0,probe:false});
  await page.screenshot({path:testInfo.outputPath('all-diagrams.png'),fullPage:true,animations:'disabled'});
  expect(safety.errors).toEqual([]);expect(safety.requests).toEqual([]);
});

test('replacement stale work and a hidden layout cannot leave broken viewport state',async({page})=>{
  const safety=await render(page);
  await page.locator('figure.document-diagram').getByRole('button',{name:'도표 확대',exact:true}).click();
  const layout='## Current layout\n\n```layout\nScreen V Header, Main, Footer\nMain V Input, Result, Feedback\n```';
  await page.evaluate(async source=>{
    const {renderDocument}=await import('/document-renderer.js');
    const container=document.querySelector('#document');
    const oldViewport=container.querySelector('.document-diagram-viewport');
    const oldButton=container.querySelector('[aria-label="도표 확대"]');
    const oldZoom=oldViewport.dataset.zoom;
    document.querySelector('main').style.display='none';
    const stale=renderDocument(container,'## Stale diagram\n\n```mermaid\nflowchart LR\nA --> B\n```');
    const latest=renderDocument(container,source);
    await Promise.all([stale,latest]);
    oldButton.click();
    window.__diagramReplacement={oldDisconnected:!oldViewport.isConnected,oldZoomPreserved:oldViewport.dataset.zoom===oldZoom};
    document.querySelector('main').style.display='block';
  },layout);
  const figure=page.locator('figure.document-diagram');
  await expect(figure).toHaveCount(1);
  await expect(figure).toHaveAttribute('data-diagram','layout');
  await expect(figure).toHaveAttribute('data-diagram-status','ready');
  await expect.poll(()=>figure.locator('.document-diagram-content').evaluate(element=>Number.parseFloat(element.style.height))).toBeGreaterThan(180);
  expectFits(await geometry(figure));
  await expect(figure.locator('button')).toHaveCount(3);
  await expect(page.locator('#document')).not.toContainText('Stale diagram');
  expect(await page.evaluate(()=>window.__diagramReplacement)).toEqual({oldDisconnected:true,oldZoomPreserved:true});
  expect(safety.errors).toEqual([]);expect(safety.requests).toEqual([]);
});
