import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {join,resolve} from 'node:path';
import {pathToFileURL} from 'node:url';

// Optional integration with an independently installed renderer source tree.
// No production code, fixture or default test run depends on that local tree.
const rendererRoot=process.env.ARCHITALK_MDVIEW_ROOT;

function connectedMermaid(code,prefix,expectedNodes,expectedEdges) {
  assert.match(code,/^graph LR\n/);
  assert.doesNotMatch(code,/undefined|NaN|\[object Object\]/);
  const definitions=new Set([...code.matchAll(new RegExp(`^    (${prefix})(?=[\\[(\\{])`,'gm'))].map(match=>match[1]));
  const edges=[...code.matchAll(new RegExp(`^    (${prefix}) -->(?:\\|"[^\\n]*"\\|)? (${prefix})$`,'gm'))];
  assert.equal(definitions.size,expectedNodes);
  assert.equal(edges.length,expectedEdges);
  for(const [,from,to] of edges){assert.ok(definitions.has(from));assert.ok(definitions.has(to));}
}

test('representative SmartMD fixture is compatible with real mdview APIs',{
  skip:rendererRoot?false:'Set ARCHITALK_MDVIEW_ROOT to a mdview source tree to run external renderer compatibility.'
},async t=>{
  const staticRoot=join(resolve(rendererRoot),'internal/mdview/static');
  const module=relative=>import(pathToFileURL(join(staticRoot,relative)).href);
  const [navigation,state,{default:jobflow},{default:layout},smartmd,{Marked}]=await Promise.all([
    module('js/navigation.js'),module('js/state.js'),module('js/jobflow.js'),module('js/layout.js'),module('js/smartmd.js'),module('vendor/marked.js')
  ]);
  const marked=new Marked();
  const source=await readFile(new URL('./fixtures/architecture.smart.md',import.meta.url),'utf8');
  const code=marked.lexer(source).filter(token=>token.type==='code');
  const blocks=language=>code.filter(token=>token.lang===language).map(token=>token.text);
  assert.deepEqual(Object.fromEntries(['navigation','jobflow','state','layout'].map(language=>[language,blocks(language).length])),{navigation:1,jobflow:1,state:1,layout:2});

  await t.test('navigation retains success, failure, retry and return transitions',()=>{
    const result=navigation.navigationToMermaid(blocks('navigation')[0]);
    connectedMermaid(result,'N\\d+',4,5);
    for(const label of ['ResourceList','open_resource','ResourceDetail','ResourceList#40;LoadError#41;','자료 선택','조회 성공','조회 실패','다시 선택','목록으로 복귀'])assert.ok(result.includes(label),label);
  });

  await t.test('state retains one owner lifecycle and success/failure recovery',()=>{
    const result=state.stateToMermaid(blocks('state')[0]);
    connectedMermaid(result,'(?:[A-Z]|S\\d+)',5,6);
    for(const label of ['Ready','Running','Showing','Failed','자료 선택','조회 성공','조회 실패','다시 선택','목록 복귀'])assert.ok(result.includes(label),label);
    assert.match(result,/style \S+ fill:#fff,stroke:#000/);
    assert.doesNotMatch(result,/style \S+ fill:#000,stroke:#000/);
  });

  await t.test('jobflow parses declared component cooperation and generates SVG',()=>{
    const script=blocks('jobflow')[0],parsed=jobflow.parseJobFlow(script);
    assert.equal(parsed.orchestrator,'Reader');
    assert.deepEqual(parsed.objects,['Reader','Repository']);
    assert.deepEqual(parsed.relationships.map(({from,fromAction,to,toAction})=>({from,fromAction:jobflow.normalizeAction(fromAction),to,toAction:jobflow.normalizeAction(toAction)})),[
      {from:'Reader',fromAction:'Open',to:'Repository',toAction:'Load'},
      {from:'Repository',fromAction:'Load.found',to:'Reader',toAction:'ShowDetail'},
      {from:'Repository',fromAction:'Load.failed',to:'Reader',toAction:'ShowError'}
    ]);
    const svg=jobflow.generateSVG(script);
    assert.match(svg,/^<svg xmlns="http:\/\/www\.w3\.org\/2000\/svg"/);
    assert.match(svg,/viewBox="0 0 480 \d+"/);
    assert.match(svg,/<\/svg>$/);
    assert.doesNotMatch(svg,/NaN|Infinity|undefined/);
    for(const label of ['Reader','Repository','Open','Load','ShowDetail','ShowError'])assert.ok(svg.includes(`>${label}</text>`),label);
  });

  await t.test('layout parses both page trees and generates the intended components',()=>{
    const expected=[
      {root:'ListScreen',containers:['ListScreen','ListMain'],leaves:['ListHeader','ResourceRows','RetryAction','ListFeedback']},
      {root:'DetailScreen',containers:['DetailScreen'],leaves:['DetailHeader','DetailBody','ReturnAction']}
    ];
    blocks('layout').forEach((script,index)=>{
      const parsed=layout.parseLayoutDSL(script),page=expected[index];
      assert.equal(parsed.rootName,page.root);
      assert.deepEqual([...parsed.containers.keys()],page.containers);
      const html=layout.generateHTML(script);
      assert.match(html,/class="layout-preview"/);
      assert.doesNotMatch(html,/layout-error|NaN|Infinity|undefined/);
      for(const name of page.leaves)assert.ok(html.includes(`>${name}</div>`),name);
    });
  });

  await t.test('SmartMD preprocess preserves metadata, callout and two-column table specification',()=>{
    const previous=globalThis.window;
    globalThis.window={marked};
    let preprocessed;
    try{preprocessed=smartmd.preprocessSmartMd(source);}
    finally{if(previous===undefined)delete globalThis.window;else globalThis.window=previous;}
    assert.match(preprocessed,/^<div class="smart-frontmatter">/);
    const title=source.match(/^title:\s*(.+)$/m)?.[1];
    assert.ok(title,'fixture metadata has a title');
    for(const text of [title,'<strong>locale</strong>: ko','<strong>style</strong>: smartmd'])assert.ok(preprocessed.includes(text),text);
    assert.match(preprocessed,/<div class="smart-callout info"><div class="smart-callout-title">설계 범위와 근거<\/div><p>/);
    assert.doesNotMatch(preprocessed,/^:::/m);
    assert.match(preprocessed,/<div class="smart-caption">요구사항과 확인 상태<\/div>/);
    const specifications=[...preprocessed.matchAll(/<!--SMART_TABLE_SPEC:([^>]+)-->/g)];
    assert.equal(specifications.length,1);
    assert.deepEqual(JSON.parse(decodeURIComponent(specifications[0][1])),[
      {align:'left',width:'60%',title:'요구사항'},
      {align:'left',width:'40%',title:'근거와 검증'}
    ]);
    const tables=marked.lexer(preprocessed).filter(token=>token.type==='table');
    assert.equal(tables.length,2);
    assert.deepEqual(tables[0].header.map(cell=>cell.text),['요구사항','근거와 검증']);
    assert.equal(tables[0].rows.length,2);
    const html=marked.parse(preprocessed);
    assert.match(html,/<table>/);assert.match(html,/<th>요구사항<\/th>/);
    // The renderer must still receive every diagram after preprocessing.
    assert.deepEqual(marked.lexer(preprocessed).filter(token=>token.type==='code').map(token=>token.lang),['navigation','jobflow','state','layout','layout']);
  });
});
