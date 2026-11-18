import {test,expect} from './fixtures.mjs';
import {readFile,writeFile} from 'node:fs/promises';

const input=process.env.ARCHITALK_RENDER_PACKAGE_JSON;
const files={architecture:'아키텍처-설계.md',drivers:'요구사항과-아키텍처-드라이버.md',quality_scenarios:'품질속성과-검증시나리오.md',alternatives:'설계대안과-트레이드오프.md',decisions:'아키텍처-결정기록.md',delivery:'구현과-운영검증.md',navigation:'네비게이션.md',jobflow:'잡플로우.md',state:'상태도.md',screen_layout:'화면설계.md'};
const languages=new Set(['navigation','jobflow','state','layout','mermaid']);

// Count actual diagram fences while skipping ordinary code and nested examples
// in mdview's text containers. This does not call the renderer's slot logic.
function diagramFences(markdown){
  const lines=markdown.replace(/\r\n?/g,'\n').split('\n'),result=[];
  for(let i=0;i<lines.length;i++){
    const opener=lines[i].match(/^ {0,3}(`{3,}|~{3,})([^\n]*)$/);
    if(!opener||!opener[2].trim())continue;
    const language=opener[2].trim().split(/\s/)[0].toLowerCase();
    if(languages.has(language))result.push(language);
    const text=language==='text'&&!lines[i].startsWith(' ');
    const close=new RegExp(`^ {0,3}${opener[1][0]}{${opener[1].length},}\\s*$`);
    let depth=0,end=lines.length;
    for(let j=i+1;j<lines.length;j++){
      if(text){
        const nested=lines[j].match(/^(`{3,}|~{3,})(.*)$/);
        if(!nested)continue;
        if(nested[2].trim())depth++;
        else if(depth)depth--;
        else{end=j;break;}
      }else if(close.test(lines[j])){end=j;break;}
    }
    i=end;
  }
  return result;
}

test.describe('opt-in actual generated package renderer',()=>{
  test.skip(!input,'Set ARCHITALK_RENDER_PACKAGE_JSON to one existing package JSON; this test never invokes a model.');
  test('all ten generated documents render through the public browser API',async({server,productPage},testInfo)=>{
    const {page,violations,errors}=productPage;
    const candidate=JSON.parse(await readFile(input,'utf8'));
    expect(typeof candidate.markdown).toBe('string');expect(candidate.documents).toHaveLength(9);
    expect(candidate.documents.map(document=>document.kind).sort()).toEqual(Object.keys(files).filter(kind=>kind!=='architecture').sort());
    const documents=[{kind:'architecture',markdown:candidate.markdown},...candidate.documents];
    const links=Object.entries(files).map(([kind,filename])=>({kind,filename}));
    const apiRequests=[];page.on('request',request=>{if(new URL(request.url()).pathname.startsWith('/api/v1/'))apiRequests.push('unexpected app API request');});
    const harness=`${server.origin}/__generated_renderer_probe__`;
    await page.route(harness,route=>route.fulfill({status:200,contentType:'text/html',headers:{'Content-Security-Policy':"default-src 'none'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self'; font-src 'self'; connect-src 'self'; object-src 'none'; base-uri 'none'"},body:'<!doctype html><html lang="ko"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><link rel="stylesheet" href="/style.css"><link rel="stylesheet" href="/document.css"><title>생성 패키지 렌더 검증</title></head><body><main style="width:100%;max-width:1200px;margin:0 auto;padding:20px;min-width:0"><p id="probe-kind"></p><article class="document-paper" id="generated-preview"></article></main></body></html>'}));
    await page.goto(harness,{waitUntil:'load'});
    await page.evaluate(()=>history.replaceState(null,'',location.pathname));
    const results=[];
    for(const document of documents){
      expect(typeof document.markdown).toBe('string');expect(Buffer.byteLength(document.markdown)).toBeGreaterThan(0);
      const expected=diagramFences(document.markdown);
      const observed=await page.evaluate(async({document,links})=>{
        const {renderDocument}=await import('/document-renderer.js');
        const root=window.document.getElementById('generated-preview');
        window.document.getElementById('probe-kind').textContent=`생성 응답 · ${document.kind}`;
        await renderDocument(root,document.markdown,{documentLinks:links,currentDocument:document.kind,onDocumentLink:()=>{}});
        return {kind:document.kind,state:root.dataset.renderState,headings:root.querySelectorAll('h1,h2,h3').length,errors:root.querySelectorAll('.document-diagram-error').length,
          diagrams:[...root.querySelectorAll('figure.document-diagram')].map(figure=>({kind:figure.dataset.diagram,status:figure.dataset.diagramStatus,svg:figure.querySelectorAll('svg').length,layout:figure.querySelectorAll('.layout-preview').length,shapes:figure.querySelectorAll('svg path,svg rect,svg circle,svg polygon,.layout-component').length,width:figure.getBoundingClientRect().width,height:figure.getBoundingClientRect().height})),
          viewport:innerWidth,documentWidth:window.document.documentElement.scrollWidth};
      },{document,links});
      results.push({...observed,sourceBytes:Buffer.byteLength(document.markdown),expected});
      await page.screenshot({path:testInfo.outputPath(`${document.kind}.png`),fullPage:true,animations:'disabled'});
      expect.soft(observed.headings,document.kind).toBeGreaterThan(0);expect.soft(observed.state,document.kind).toBe('ready');expect.soft(observed.errors,document.kind).toBe(0);
      expect.soft(observed.diagrams.map(diagram=>diagram.kind).sort(),document.kind).toEqual(expected.sort());
      expect.soft(observed.diagrams.every(diagram=>diagram.status==='ready'&&diagram.shapes>0&&diagram.width>0&&diagram.height>0&&(diagram.kind==='layout'?diagram.layout===1:diagram.svg===1)),document.kind).toBe(true);
      expect.soft(observed.documentWidth,document.kind).toBeLessThanOrEqual(observed.viewport);
    }
    await writeFile(testInfo.outputPath('generated-render-summary.json'),JSON.stringify(results,null,2));
    expect(apiRequests).toEqual([]);expect(violations).toEqual([]);expect(errors).toEqual([]);
  });
});
