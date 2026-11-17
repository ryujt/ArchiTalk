import {test,expect} from './fixtures.mjs';

const choices={
  claude:{models:['sonnet','opus','fable'],efforts:['low','medium','high','xhigh','max']},
  codex:{models:['gpt-6.2-sol','gpt-6.2-astra','gpt-6.1-terra','gpt-6.1-sol','gpt-6-astra','gpt-6-sol'],efforts:['minimal','low','medium','high','xhigh','max','ultra']}
};
const roles=['conversation','design','critic'];
const names=['claude','codex',...roles];

async function catalog(page,name,backend) {
  for(const [field,items] of [['model',choices[backend].models],['effort',choices[backend].efforts]]){
    const input=page.locator(`#${name}-${field}`);
    await expect(input).toBeEditable();
    expect(await page.locator(`#${name}-${field}-choice option`).evaluateAll(options=>options.map(option=>option.value))).toEqual(['',...items]);
  }
}
async function inputs(page) {
  return page.locator('#settings-form').evaluate((form,names)=>Object.fromEntries(names.flatMap(name=>['model','effort'].map(field=>{
    const id=`${name}-${field}`;return [id,form.querySelector(`#${id}`).value];
  }))),names);
}
async function noOverflow(page) {
  expect(await page.evaluate(()=>document.documentElement.scrollWidth<=window.innerWidth)).toBe(true);
  expect(await page.getByRole('dialog').evaluate(dialog=>dialog.scrollWidth<=dialog.clientWidth)).toBe(true);
}
async function capture(page,testInfo,name,edge) {
  await page.getByRole('dialog').evaluate((dialog,edge)=>{dialog.scrollTop=edge==='top'?0:dialog.scrollHeight;},edge);
  await noOverflow(page);
  await page.screenshot({path:testInfo.outputPath(`${name}.png`),fullPage:true,animations:'disabled'});
}
async function openSettings(page) {
  const response=page.waitForResponse(response=>new URL(response.url()).pathname==='/api/v1/settings'&&response.request().method()==='GET');
  await page.locator('.app-header [data-action="settings"]').click();
  await expect(page.getByRole('dialog')).toBeVisible();
  return (await response).json();
}
async function persistedFields(page) {
  for(const [name,field,value,selection] of [
    ['claude','model','sonnet','sonnet'],['claude','effort','high','high'],
    ['codex','model','custom-codex-model',''],['codex','effort','provider-effort',''],
    ['conversation','model','',''],['conversation','effort','',''],
    ['design','model','fixture-custom-design',''],['design','effort','medium','medium'],
    ['critic','model','opus','opus'],['critic','effort','max','max']
  ]){
    await expect(page.locator(`#${name}-${field}`)).toHaveValue(value);
    await expect(page.locator(`#${name}-${field}-choice`)).toHaveValue(selection);
  }
}

test('known model choices and free input survive backend changes, save, reload and the next fixture runs',async({server,productPage},testInfo)=>{
  const {page,violations,errors}=productPage;
  let diagnosticsRequests=0;
  page.on('request',request=>{if(new URL(request.url()).pathname==='/api/v1/settings/diagnostics')diagnosticsRequests++;});
  await page.goto(server.origin,{waitUntil:'domcontentloaded'});
  await expect(page.locator('#start-form')).toBeVisible();
  const initial=await server.workspace();
  const response=await openSettings(page);
  expect(response.choices).toEqual(choices);
  expect(response.diagnostics).toEqual([]);

  // Both CLI defaults and every role expose the catalog before any diagnosis.
  for(const name of names){
    const backend=name==='codex'?'codex':'claude';
    await catalog(page,name,backend);
    for(const [field,known] of [['model',choices[backend].models[0]],['effort',choices[backend].efforts.at(-1)]]){
      const input=page.locator(`#${name}-${field}`),choice=page.locator(`#${name}-${field}-choice`);
      await choice.selectOption(known);await expect(input).toHaveValue(known);
      const custom=`custom-${name}-${field}`;
      await input.fill(custom);await expect(choice).toHaveValue('');
      await choice.selectOption('');await expect(input).toHaveValue(custom);
      await input.fill(known);await expect(choice).toHaveValue(known);
    }
  }

  for(const role of roles){
    await page.locator(`#${role}-model`).fill(`kept-${role}-model`);
    await page.locator(`#${role}-effort`).fill(`kept-${role}-effort`);
  }
  const preserved=await inputs(page);
  await page.locator('#conversation-backend').selectOption('claude');
  await page.locator('#critic-backend').selectOption('codex');
  await catalog(page,'critic','codex');expect(await inputs(page)).toEqual(preserved);
  await page.locator('#default-backend').selectOption('codex');
  for(const [name,backend] of [['claude','claude'],['codex','codex'],['conversation','claude'],['design','codex'],['critic','codex']])await catalog(page,name,backend);
  expect(await inputs(page)).toEqual(preserved);
  await page.locator('#critic-backend').selectOption('');await catalog(page,'critic','codex');
  await page.locator('#default-backend').selectOption('claude');
  for(const role of roles)await catalog(page,role,'claude');
  expect(await inputs(page)).toEqual(preserved);
  await page.locator('#design-backend').selectOption('codex');await catalog(page,'design','codex');
  expect(await inputs(page)).toEqual(preserved);

  // Persist a mixture of list choices, custom values and blank inheritance.
  await page.locator('#claude-model-choice').selectOption('sonnet');
  await page.locator('#claude-effort-choice').selectOption('high');
  await page.locator('#codex-model').fill('custom-codex-model');
  await page.locator('#codex-effort').fill('provider-effort');
  await page.locator('#conversation-backend').selectOption('');
  await page.locator('#conversation-model').fill('');await page.locator('#conversation-effort').fill('');
  await page.locator('#design-backend').selectOption('claude');
  await page.locator('#design-model').fill('fixture-custom-design');await page.locator('#design-effort').fill('medium');
  await page.locator('#critic-backend').selectOption('claude');
  await page.locator('#critic-model-choice').selectOption('opus');await page.locator('#critic-effort-choice').selectOption('max');
  await persistedFields(page);
  await capture(page,testInfo,'01-settings-choices','top');
  await capture(page,testInfo,'02-role-choices','bottom');
  expect(diagnosticsRequests).toBe(0);
  await page.locator('#settings-form button[type="submit"]').click();
  await expect(page.getByRole('dialog')).not.toBeVisible();
  const saved=await server.workspace();
  expect(saved.settings_revision).toBeGreaterThan(initial.settings_revision);
  expect(saved.runs).toEqual(initial.runs);
  expect(saved.settings.ai.default_backend).toBe('claude');
  expect(saved.settings.ai.backends.claude).toMatchObject({model:'sonnet',effort:'high'});
  expect(saved.settings.ai.backends.codex).toMatchObject({model:'custom-codex-model',effort:'provider-effort'});
  expect(saved.settings.ai.roles).toEqual({
    conversation:{backend:null,model:null,effort:null},
    design:{backend:'claude',model:'fixture-custom-design',effort:'medium'},
    critic:{backend:'claude',model:'opus',effort:'max'}
  });
  await openSettings(page);await persistedFields(page);
  await page.keyboard.press('Escape');await expect(page.getByRole('dialog')).not.toBeVisible();
  await page.reload({waitUntil:'domcontentloaded'});
  await expect(page.locator('#start-form')).toBeVisible();
  await openSettings(page);await persistedFields(page);
  expect((await server.workspace()).runs).toEqual(initial.runs);
  await capture(page,testInfo,'03-reloaded-choices','bottom');
  await page.keyboard.press('Escape');await expect(page.getByRole('dialog')).not.toBeVisible();

  // These subprocesses are the existing isolated fixture, never a real model.
  await page.locator('#start-form textarea').fill('가입 안내 알림을 보내는 작은 서비스를 만들고 싶어요.');
  await page.locator('#start-form button[type="submit"]').click();
  await expect(page.locator('.chat-thread')).toContainText('현재 말씀하신 내용은 제안과 미정으로 나누어 기록했습니다.');
  await page.locator('[data-action="generate"]').click();
  await expect.poll(async()=>{
    const snapshot=await server.workspace();
    return snapshot.runs.filter(run=>['conversation','design','critic'].includes(run.kind)&&run.status==='completed').length===3&&snapshot.active_run===null;
  }).toBe(true);
  const completed=await server.workspace();
  for(const [kind,model,effort] of [['conversation','sonnet','high'],['design','fixture-custom-design','medium'],['critic','opus','max']]){
    const run=completed.runs.find(run=>run.kind===kind);
    expect(run.effective_settings).toMatchObject({backend:'claude',model,effort});
  }
  expect(completed.settings).toEqual(saved.settings);
  expect(diagnosticsRequests).toBe(0);
  expect(violations).toEqual([]);expect(errors).toEqual([]);
});
