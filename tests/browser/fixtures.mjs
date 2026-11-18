import {test as base,expect} from '@playwright/test';
import {mkdtemp,mkdir,copyFile,chmod,writeFile,rm,readFile} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join,resolve} from 'node:path';
import {spawn} from 'node:child_process';
import {once} from 'node:events';

const repo=resolve(new URL('../..',import.meta.url).pathname);
// A frozen build makes every case use the same binary while other agents run
// Rust checks. This is a test-runner override, never product configuration.
const binary=process.env.ARCHITALK_BROWSER_BINARY?resolve(process.env.ARCHITALK_BROWSER_BINARY):join(repo,'target/debug/architalk');
const redact=text=>String(text).replace(/#token=[^\s]+/g,'#token=[redacted]').replace(/[a-f0-9]{64}/g,'[redacted]');

async function startServer() {
  const temporary=await mkdtemp(join(tmpdir(),'architalk-browser-'));
  const project=join(temporary,'browser-project');
  const tools=join(temporary,'trusted-tools');
  await mkdir(project);await mkdir(tools);
  const executable=join(tools,'fixture-cli.py');
  await copyFile(join(repo,'tests/fixtures/claude-workflow.py'),executable);
  await copyFile(join(repo,'tests/fixtures/architecture.smart.md'),join(tools,'architecture.smart.md'));
  await copyFile(join(repo,'tests/fixtures/design-package.json'),join(tools,'design-package.json'));
  await chmod(executable,0o700);
  await writeFile(join(project,'README.md'),'# Browser test project\nSynthetic local notification design.\n');
  const config=join(tools,'config.toml');
  await writeFile(config,`[ai]\ndefault_backend = "claude"\ntimeout_seconds = 30\n[ai.backends.claude]\nexecutable = ${JSON.stringify(executable)}\n[ai.backends.codex]\nexecutable = ${JSON.stringify(executable)}\n[documents]\ndirectory = "docs/architecture"\nlanguage = "ko"\n`);
  // Both configured executables are the copied fixture. Browser tests cannot
  // reach a real model even if a backend selection regresses.
  const child=spawn(binary,['--no-open','--port','0','--project',project,'--config',config],{
    cwd:repo,env:{...process.env,RUST_LOG:'architalk=warn'},stdio:['ignore','pipe','pipe']
  });
  let output='',errors='';
  child.stderr.on('data',chunk=>{errors=(errors+chunk).slice(-16000);});
  const closed=once(child,'exit').catch(()=>[]);
  let endpoint;
  try {
    endpoint=await new Promise((resolveEndpoint,reject)=>{
      const timer=setTimeout(()=>reject(new Error('The isolated test server did not publish a local address.')),15_000);
      const fail=()=>{clearTimeout(timer);reject(new Error(`The isolated test server could not start: ${redact(errors)}`));};
      child.once('error',fail);child.once('exit',fail);
      child.stdout.on('data',chunk=>{
        output=(output+chunk).slice(-16000);
        const match=output.match(/http:\/\/127\.0\.0\.1:(\d+)\/#token=([a-f0-9]{64})/);
        if(match){clearTimeout(timer);child.off('error',fail);child.off('exit',fail);resolveEndpoint({origin:`http://127.0.0.1:${match[1]}`,token:match[2]});}
      });
    });
  } catch(error) {
    if(child.exitCode===null&&child.signalCode===null)child.kill('SIGKILL');
    await closed;await rm(temporary,{recursive:true,force:true});throw error;
  }
  return {
    ...endpoint,project,
    async configureDocumentFixture(markdown){
      if(typeof markdown!=='string'||Buffer.byteLength(markdown)>500_000)throw new Error('Invalid isolated document fixture.');
      const packagePath=join(tools,'design-package.json');
      const candidate=JSON.parse(await readFile(packagePath,'utf8'));
      candidate.markdown=markdown;
      await writeFile(packagePath,JSON.stringify(candidate));
    },
    async configurePackageFixture(candidate){
      const text=JSON.stringify(candidate);
      if(Buffer.byteLength(text)>900_000)throw new Error('Isolated package fixture is too large.');
      await writeFile(join(tools,'design-package.json'),text);
    },
    async configureSlowFixture(seconds=2){
      if(!Number.isFinite(seconds)||seconds<=0||seconds>30)throw new Error('Fixture delay must be between 0 and 30 seconds.');
      const source=await readFile(executable,'utf8');
      if(!source.includes('time.sleep(30)'))throw new Error('The isolated slow fixture was already configured or changed.');
      await writeFile(executable,source.replace('time.sleep(30)',`time.sleep(${seconds})`));
    },
    async workspace(){
      const response=await fetch(`${endpoint.origin}/api/v1/workspace`,{headers:{Authorization:`Bearer ${endpoint.token}`},redirect:'error'});
      if(!response.ok)throw new Error(`Workspace snapshot returned HTTP ${response.status}.`);
      return response.json();
    },
    async savedFile(path){return readFile(join(project,path),'utf8');},
    async stop(){
      if(child.exitCode===null&&child.signalCode===null)child.kill('SIGTERM');
      let timer;
      await Promise.race([closed,new Promise(resolve=>{timer=setTimeout(resolve,8000);})]);
      clearTimeout(timer);
      if(child.exitCode===null&&child.signalCode===null){child.kill('SIGKILL');await closed;}
      await rm(temporary,{recursive:true,force:true});
    }
  };
}

export const test=base.extend({
  server:async({},use)=>{
    const server=await startServer();
    try{await use(server);}finally{await server.stop();}
  },
  productPage:async({browser,server},use,testInfo)=>{
    const context=await browser.newContext({viewport:testInfo.project.use.viewport,isMobile:Boolean(testInfo.project.use.isMobile),hasTouch:Boolean(testInfo.project.use.hasTouch),locale:'ko-KR',timezoneId:'Asia/Seoul',reducedMotion:'reduce',serviceWorkers:'block'});
    const violations=[],errors=[];
    // Never connect to, reuse, or modify the user's running browser surfaces.
    await context.route('**/*',async route=>{
      if(new URL(route.request().url()).origin===server.origin)await route.continue();
      else {violations.push('product attempted an external network request');await route.abort('blockedbyclient');}
    });
    const page=await context.newPage();
    // Navigation logs contain only the public origin. The capability enters the
    // real fragment path before application code and is never persisted by the
    // runner as trace/HAR/storageState. Reload uses the app's sessionStorage.
    await page.addInitScript(({origin,capability})=>{
      if(location.origin===origin&&!sessionStorage.getItem('architalk.authorization'))history.replaceState(null,'',`${location.pathname}#token=${capability}`);
    },{origin:server.origin,capability:server.token});
    page.on('request',request=>{
      if(!request.url().startsWith(`${server.origin}/api/v1/`))return;
      if(request.url().includes(server.token))violations.push('capability appeared in an API URL');
      if(request.headers().authorization!==`Bearer ${server.token}`)violations.push('API request lacked the expected capability header');
    });
    page.on('pageerror',error=>errors.push(redact(error.message)));
    try{await use({page,violations,errors,context});}
    finally{await context.close();}
  }
});
export {expect};
