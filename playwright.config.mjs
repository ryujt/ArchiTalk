import {defineConfig} from '@playwright/test';

// Every browser is launched by this repository's test runner. No CDP endpoint,
// existing browser profile, external tab, trace, or authentication file is used.
const run=process.env.ARCHITALK_BROWSER_RUN ||= `run-${Date.now()}`;
export default defineConfig({
  testDir:'./tests/browser',
  testMatch:'**/*.spec.mjs',
  fullyParallel:false,
  workers:1,
  retries:0,
  timeout:60_000,
  expect:{timeout:15_000},
  reporter:[['line']],
  outputDir:`target/browser-artifacts/${run}`,
  preserveOutput:'always',
  globalSetup:'./tests/browser/retention.mjs',
  use:{
    browserName:'chromium',
    ...(process.env.ARCHITALK_BROWSER_CHANNEL === 'chromium' ? {} : {channel:'chrome'}),
    headless:true,
    trace:'off',
    video:'off',
    screenshot:'only-on-failure',
    locale:'ko-KR',
    timezoneId:'Asia/Seoul',
    reducedMotion:'reduce',
    serviceWorkers:'block'
  },
  projects:[
    {name:'desktop',use:{viewport:{width:1365,height:900}}},
    {name:'mobile',use:{viewport:{width:390,height:844},isMobile:true,hasTouch:true}}
  ]
});
