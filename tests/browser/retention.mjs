import {readdir,stat,rm} from 'node:fs/promises';
import {resolve,join} from 'node:path';

export default async function pruneExpiredBrowserArtifacts() {
  const root=resolve('target/browser-artifacts');
  const cutoff=Date.now()-7*24*60*60*1000;
  let entries;
  try { entries=await readdir(root,{withFileTypes:true}); }
  catch(error) { if(error.code==='ENOENT')return;throw error; }
  for(const entry of entries) {
    if(!entry.isDirectory() || !/^run-\d+$/.test(entry.name))continue;
    const path=join(root,entry.name);
    if((await stat(path)).mtimeMs<cutoff)await rm(path,{recursive:true,force:true});
  }
}
