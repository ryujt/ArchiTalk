import {test,expect} from './fixtures.mjs';
import {mkdir,writeFile,realpath} from 'node:fs/promises';
import {join} from 'node:path';

const preview=page=>page.locator('#file-content > [data-file-preview]:not([hidden])');
async function left(page,open){
  const toggle=page.locator('#sidebar-toggle');
  if((await toggle.getAttribute('aria-expanded'))!==String(open))await toggle.click();
  await expect(toggle).toHaveAttribute('aria-expanded',String(open));
}
async function right(page,open){
  const toggle=page.locator('#info-toggle');
  if((await toggle.getAttribute('aria-expanded'))!==String(open))await toggle.click();
  await expect(toggle).toHaveAttribute('aria-expanded',String(open));
}
async function openFile(page,path){
  await left(page,true);
  await page.locator('#file-tree [data-file-path]').filter({hasText:path.split('/').at(-1)}).click();
  await expect(preview(page)).toHaveAttribute('data-render-state','ready');
  await expect(page.locator('#file-path')).toHaveText(path);
}
async function geometry(page){
  const dimensions=await page.locator('.workspace').evaluate(node=>({width:node.getBoundingClientRect().width,right:node.getBoundingClientRect().right}));
  expect(dimensions.width).toBeGreaterThan(200);
  expect(dimensions.right).toBeLessThanOrEqual((await page.viewportSize()).width+1);
  expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true);
  await expect(page.locator('#sidebar-toggle')).toBeVisible();
  await expect(page.locator('#info-toggle')).toBeVisible();
  return dimensions.width;
}

test('file tree, tabs, panels and path copying stay inside the workspace',async({server,productPage},testInfo)=>{
  const {page,errors,violations}=productPage,mobile=testInfo.project.name==='mobile';
  await mkdir(join(server.project,'notes'));
  await writeFile(join(server.project,'README.md'),'# 파일 작업공간\n\n[근거 문서](notes/guide.md#근거)\n\n읽기 전용 파일입니다.\n');
  await writeFile(join(server.project,'notes/guide.md'),'# 안내\n\n## 근거\n\n교차 파일 링크가 선택한 제목으로 이동합니다.\n');
  await writeFile(join(server.project,'code.txt'),'const marker = "<img src=x onerror=alert(1)>";\n반복 문장\n반복 문장\n');
  await page.addInitScript(()=>{
    window.__pathCopies=[];window.__denyCopy=false;
    Object.defineProperty(navigator,'clipboard',{configurable:true,value:{writeText:async value=>{if(window.__denyCopy)throw new Error('stub denied');window.__pathCopies.push(value);}}});
  });
  const nativeCopies=[];
  // Never dispatch a test copy to the host operating system's real clipboard.
  await page.route('**/api/v1/clipboard**',route=>{
    nativeCopies.push(route.request().postDataJSON());return route.fulfill({status:503,json:{error:{code:'clipboard_unavailable',message:'SYNTHETIC_RAW_PATH_COPY_ERROR'}}});
  });
  const reads=[],writes=[];
  page.on('request',request=>{
    const url=new URL(request.url());
    if(url.pathname==='/api/v1/files/read')reads.push(url.searchParams.get('path'));
    if(url.pathname.startsWith('/api/v1/')&&request.method()!=='GET')writes.push(url.pathname);
  });
  await page.goto(server.origin,{waitUntil:'domcontentloaded'});
  await expect(page.locator('#file-tree [data-file-path="README.md"]')).toBeAttached();
  await right(page,false);await left(page,false);const largest=await geometry(page);
  await left(page,true);const withLeft=await geometry(page);
  if(!mobile)expect(largest-withLeft).toBeGreaterThan(180);
  await left(page,false);await right(page,true);const withRight=await geometry(page);
  if(!mobile)expect(largest-withRight).toBeGreaterThan(250);
  await right(page,false);await left(page,true);await right(page,true);await geometry(page);
  await right(page,false);
  await openFile(page,'README.md');
  await expect(preview(page).getByRole('heading',{name:'파일 작업공간',exact:true})).toBeVisible();
  await openFile(page,'README.md');
  await expect(page.locator('#workspace-tabs [data-tab-path="README.md"]')).toHaveCount(1);
  expect(reads.filter(path=>path==='README.md')).toHaveLength(1);
  await left(page,false);
  await preview(page).getByRole('link',{name:'근거 문서',exact:true}).click();
  await expect(page.locator('#file-path')).toHaveText('notes/guide.md');
  await expect(preview(page)).toHaveAttribute('data-render-state','ready');
  await expect(preview(page).getByRole('heading',{name:'근거',exact:true})).toBeFocused();
  expect(reads).toEqual(['README.md','notes/guide.md']);
  expect(new URL(page.url()).pathname).toBe('/');
  await openFile(page,'code.txt');
  await expect(preview(page).locator('pre')).toContainText('<img src=x onerror=alert(1)>');
  await expect(preview(page).locator('img')).toHaveCount(0);
  await left(page,true);
  const code=page.locator('#file-tree [data-file-path="code.txt"]');
  for(const [label,value] of [['상대 경로 복사','code.txt'],['절대 경로 복사',await realpath(join(server.project,'code.txt'))],['이름 복사','code.txt']]){
    await code.click({button:'right'});
    await page.getByRole('menuitem',{name:label,exact:true}).click();
    await expect.poll(()=>page.evaluate(()=>window.__pathCopies.at(-1))).toBe(value);
  }
  expect(nativeCopies).toEqual([]);
  await page.evaluate(()=>{window.__denyCopy=true;});
  await code.click({button:'right'});await page.getByRole('menuitem',{name:'상대 경로 복사',exact:true}).click();
  await expect(page.locator('.copy-dialog')).toBeVisible();
  await expect(page.locator('#manual-copy-text')).toHaveValue('code.txt');
  expect(nativeCopies).toEqual([{text:'code.txt'}]);
  await expect(page.locator('.copy-dialog [data-copy-reason]')).toBeVisible();
  await expect(page.locator('.copy-dialog')).not.toContainText('SYNTHETIC_RAW_PATH_COPY_ERROR');
  await page.getByRole('button',{name:'복사 창 닫기',exact:true}).click();
  await expect(code).toBeFocused();await left(page,false);
  const active=page.locator('#workspace-tabs [aria-selected="true"]');
  await active.focus();await page.keyboard.press('Home');
  await expect(page.locator('#workspace-tabs [data-tab-path="chat"]')).toBeFocused();
  await expect(page.locator('#chat-workspace')).toBeVisible();
  await page.keyboard.press('End');await expect(page.locator('#workspace-tabs [data-tab-path="code.txt"]')).toBeFocused();
  await expect(page.locator('#notes-tab')).toHaveAttribute('aria-selected','true');
  await page.getByRole('button',{name:'code.txt 탭 닫기',exact:true}).click();
  await expect(page.locator('#workspace-tabs [data-tab-path="code.txt"]')).toHaveCount(0);
  await expect(page.locator('#file-path')).toHaveText('notes/guide.md');
  await left(page,true);await page.locator('.new-session').click();
  await expect(page.locator('#chat-workspace')).toBeVisible();
  await expect(page.locator('#workspace-tabs [data-tab-path="chat"]')).toHaveAttribute('aria-selected','true');
  if(mobile)await expect(page.locator('#sidebar-toggle')).toHaveAttribute('aria-expanded','false');
  await geometry(page);
  await page.screenshot({path:testInfo.outputPath('file-tabs-panels.png'),fullPage:true,animations:'disabled'});
  expect(writes).toEqual(['/api/v1/clipboard']);
  expect((await server.workspace()).sessions).toHaveLength(0);
  expect(violations).toEqual([]);expect(errors).toEqual([]);
});

test('ten-document save keeps its actions reachable on a short screen',async({server,productPage},testInfo)=>{
  const {page,errors,violations}=productPage,mobile=testInfo.project.name==='mobile';
  await page.route('**/api/v1/clipboard**',route=>route.fulfill({status:503,json:{error:{code:'clipboard_unavailable',message:'Unexpected test copy was blocked.'}}}));
  await page.goto(server.origin,{waitUntil:'domcontentloaded'});
  await page.getByLabel('프로젝트를 한두 문장으로 소개해 주세요').fill('합성 저장 modal 검증용 알림 서비스입니다.');
  await page.getByRole('button',{name:'대화 시작',exact:true}).click();
  await expect(page.getByRole('button',{name:'다음 단계로 · 초안 만들기',exact:true})).toBeEnabled();
  await page.getByRole('button',{name:'다음 단계로 · 초안 만들기',exact:true}).click();
  await expect(page.locator('[data-action="save-preview"]')).toBeEnabled({timeout:30_000});
  await page.locator('[data-action="save-preview"]').click();
  const dialog=page.locator('dialog.save-dialog');
  await expect(dialog.locator('.plan-file')).toHaveCount(10);
  await expect(dialog.locator('.plan-preview[open]')).toHaveCount(0);
  await expect(dialog.locator('.plan-preview')).toHaveCount(10);
  await expect(dialog.locator('.plan-preview pre').first()).not.toBeVisible();
  const apply=dialog.locator('[data-action="export-apply"]');
  const checkFooter=async()=>{
    await expect(apply).toBeVisible();await expect(apply).toBeEnabled();
    const box=await apply.boundingBox(),footer=await dialog.locator('.save-dialog-footer').boundingBox();
    expect(box.width).toBeGreaterThan(80);expect(box.y).toBeGreaterThanOrEqual(0);
    expect(box.y+box.height).toBeLessThanOrEqual((await page.viewportSize()).height);
    expect(footer.y+footer.height).toBeLessThanOrEqual((await page.viewportSize()).height);
    expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true);
  };
  await page.setViewportSize({width:mobile?390:1024,height:360});await checkFooter();
  await dialog.locator('.plan-preview summary').first().click();
  await expect(dialog.locator('.plan-preview[open]')).toHaveCount(1);
  await checkFooter();
  await dialog.locator('.save-dialog-body').evaluate(node=>{node.scrollTop=node.scrollHeight;});
  await checkFooter();
  await page.screenshot({path:testInfo.outputPath('ten-files-short-screen.png'),animations:'disabled'});
  await apply.click();
  await expect(page.getByRole('heading',{name:'설계 문서를 저장했어요',exact:true})).toBeVisible();
  const snapshot=await server.workspace(),exported=snapshot.exports.at(-1);
  expect(exported.result.paths).toHaveLength(10);
  for(const path of exported.result.paths)expect((await server.savedFile(path)).length).toBeGreaterThan(100);
  // Saving triggers one explicit catalog refresh, so newly written files are
  // available without reloading the app or generating another design.
  await expect(page.locator('#file-tree [data-file-path]')).toHaveCount(11);
  const treePaths=await page.locator('#file-tree [data-file-path]').evaluateAll(nodes=>nodes.map(node=>node.dataset.filePath.normalize('NFC')));
  expect(treePaths).toEqual(expect.arrayContaining(exported.result.paths.map(path=>path.normalize('NFC'))));
  expect(snapshot.sessions[0].messages.filter(message=>message.role==='user')).toHaveLength(1);
  expect(snapshot.runs.filter(run=>run.kind==='design')).toHaveLength(1);
  expect(violations).toEqual([]);expect(errors).toEqual([]);
});
