import {test,expect} from './fixtures.mjs';

async function toggle(page,id,open){const button=page.locator(`#${id}`);if(await button.getAttribute('aria-expanded')!==String(open))await button.click();await expect(button).toHaveAttribute('aria-expanded',String(open));}
async function hittable(page,id){
  const button=page.locator(`#${id}`);await expect(button).toBeVisible();const rect=await button.boundingBox();
  expect(rect.width).toBeGreaterThanOrEqual(32);expect(rect.height).toBeGreaterThanOrEqual(32);
  expect(rect.x).toBeGreaterThanOrEqual(0);expect(rect.x+rect.width).toBeLessThanOrEqual(page.viewportSize().width+1);
  expect(await button.evaluate(node=>{const r=node.getBoundingClientRect();return node.contains(document.elementFromPoint(r.x+r.width/2,r.y+r.height/2));})).toBe(true);
  return rect;
}
async function boundary(page,id,panel,side){
  const button=await hittable(page,id),rect=await page.locator(`#${panel}`).boundingBox();
  const edge=side==='left'?rect.x:rect.x+rect.width;
  expect(Math.abs(button.x+button.width/2-edge)).toBeLessThanOrEqual(button.width+8);
}

test('project files follow the steps and both panel controls sit at their boundary',async({server,productPage},testInfo)=>{
  const {page,errors,violations}=productPage;await page.goto(server.origin,{waitUntil:'domcontentloaded'});
  await expect(page.locator('[data-file-path="README.md"]')).toBeAttached();
  await toggle(page,'sidebar-toggle',true);
  await page.screenshot({path:testInfo.outputPath('sidebar-placement.png'),fullPage:true});
  expect(await page.locator('.app-header #sidebar-toggle,.app-header #info-toggle').count()).toBe(0);
  const steps=await page.locator('.steps').boundingBox(),files=await page.getByRole('heading',{name:'프로젝트 파일',exact:true}).boundingBox();
  expect(files.y-steps.y-steps.height).toBeGreaterThanOrEqual(0);expect(files.y-steps.y-steps.height).toBeLessThan(48);
  await boundary(page,'sidebar-toggle','project-sidebar','right');
  await toggle(page,'sidebar-toggle',false);await hittable(page,'sidebar-toggle');
  await toggle(page,'info-toggle',true);await boundary(page,'info-toggle','aux-panel','left');
  await toggle(page,'info-toggle',false);await hittable(page,'info-toggle');
  expect(errors).toEqual([]);expect(violations).toEqual([]);
});

test('edge controls stay reachable through resizing, scrolling and keyboard toggles',async({server,productPage},testInfo)=>{
  const {page,errors,violations}=productPage;await page.goto(server.origin,{waitUntil:'domcontentloaded'});
  await expect(page.locator('[data-file-path="README.md"]')).toBeAttached();await toggle(page,'sidebar-toggle',true);await page.locator('[data-file-path="README.md"]').click();
  await expect(page.locator('#file-content [data-file-preview]')).toHaveAttribute('data-render-state','ready');await expect(page.locator('#document-recorder')).toBeVisible();
  for(const viewport of [{width:1365,height:900},{width:1024,height:768},{width:768,height:900},{width:390,height:844},{width:320,height:568}]){
    await page.setViewportSize(viewport);await toggle(page,'sidebar-toggle',false);await toggle(page,'info-toggle',false);
    await hittable(page,'sidebar-toggle');await hittable(page,'info-toggle');
    const tabs=await page.locator('#workspace-tabs').boundingBox(),recorder=await page.locator('#document-recorder').boundingBox();
    expect(recorder.y).toBeGreaterThanOrEqual(tabs.y+tabs.height-.5);
    expect(await page.locator('.workspace').evaluate(node=>node.getBoundingClientRect().width)).toBeGreaterThan(viewport.width*.8);
    await toggle(page,'sidebar-toggle',true);await boundary(page,'sidebar-toggle','project-sidebar','right');await hittable(page,'info-toggle');
    await toggle(page,'info-toggle',true);await boundary(page,'info-toggle','aux-panel','left');await hittable(page,'sidebar-toggle');
    const left=await page.locator('#sidebar-toggle').boundingBox(),right=await page.locator('#info-toggle').boundingBox();
    expect(left.x+left.width<=right.x||right.x+right.width<=left.x||left.y+left.height<=right.y||right.y+right.height<=left.y).toBe(true);
    await toggle(page,'info-toggle',false);const button=page.locator('#sidebar-toggle');await button.focus();const was=await button.getAttribute('aria-expanded');await page.keyboard.press('Space');await expect(button).toHaveAttribute('aria-expanded',String(was!=='true'));
    await page.evaluate(()=>{document.querySelector('#main').style.minHeight='1800px';window.scrollTo(0,700);});
    await hittable(page,'sidebar-toggle');await hittable(page,'info-toggle');
    expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true);
    await page.evaluate(()=>{document.querySelector('#main').style.minHeight='';window.scrollTo(0,0);});
  }
  await page.screenshot({path:testInfo.outputPath('narrow-edge-controls.png'),fullPage:true});expect(errors).toEqual([]);expect(violations).toEqual([]);
});
