export function matchCatalogPath(path, files) {
  if(typeof path!=='string')return null;
  return files.find(file=>typeof file?.path==='string'&&file.path.normalize('NFC')===path.normalize('NFC')) || null;
}

/** File bodies, tabs, and pending requests live only in this page instance. */
export function createFileTabs({readFile,onChange=()=>{}}) {
  const tabs=new Map();let active='chat',generation=0;
  const notify=()=>onChange({active,tabs:[...tabs.values()]});
  async function open(entry,{refresh=false}={}) {
    let tab=tabs.get(entry.path);
    if(tab&&!refresh){active=entry.path;notify();return tab;}
    if(!tab){tab={path:entry.path,name:entry.name,status:'loading',file:null,error:'',node:null,scroll:0};tabs.set(entry.path,tab);}
    tab.controller?.abort();const controller=new AbortController();tab.controller=controller;const epoch=generation;
    active=entry.path;tab.status='loading';tab.error='';notify();
    try{const file=await readFile(entry.path,controller.signal);if(epoch!==generation||tabs.get(entry.path)!==tab||tab.controller!==controller)return null;if(file.path!==entry.path)throw new Error('파일 응답 경로가 요청과 일치하지 않아요.');tab.file=file;tab.status='ready';notify();return tab;}
    catch(error){if(epoch!==generation||tabs.get(entry.path)!==tab||controller.signal.aborted)return null;tab.file=null;tab.status='error';tab.error=error.message || '파일을 읽지 못했어요.';notify();return tab;}
  }
  return {open,get active(){return active;},get tabs(){return tabs;},activate(path){if(path==='chat'||tabs.has(path)){active=path;notify();}},close(path){const tab=tabs.get(path);tab?.controller?.abort();tab?.node?.replaceChildren?.();tabs.delete(path);if(active===path)active=[...tabs.keys()].at(-1)||'chat';notify();},clear(){generation++;for(const tab of tabs.values()){tab.controller?.abort();tab.node?.replaceChildren?.();}tabs.clear();active='chat';notify();}};
}

export function createFileViewer({doc=document,win=window,api,menu,onActive=()=>{},announce=()=>{}}) {
  const tree=doc.getElementById('file-tree'),bar=doc.getElementById('workspace-tabs');
  const panel=doc.getElementById('file-workspace'),chat=doc.getElementById('chat-workspace'),content=doc.getElementById('file-content'),status=doc.getElementById('file-load-state');
  let catalog=[],root='',epoch=0,treeRequest=null,renderer=null,activePath='chat',chatScroll=0,tabKey='';
  const tabs=createFileTabs({readFile:(path,signal)=>api(`/api/v1/files/read?path=${encodeURIComponent(path)}`,{signal}),onChange:render});
  const module=()=>renderer ||= import('./document-renderer.js');
  function remember(){if(activePath==='chat')chatScroll=win.scrollY||0;else{const tab=tabs.tabs.get(activePath);if(tab)tab.scroll=win.scrollY||0;}}
  function renderMarkdown(tab,node,file) {
    if(tab.renderingFile===file)return;
    tab.renderingFile=file;
    tab.ready=module().then(({renderDocument})=>{
      if(tabs.tabs.get(tab.path)!==tab||tab.node!==node||tab.file!==file)return;
      return renderDocument(node,file.content,{currentFile:tab.path,fileLinks:catalog.map(entry=>entry.path),onFileLink:({path,anchor})=>void open(path,anchor)});
    }).catch(()=>{if(tabs.tabs.get(tab.path)===tab&&tab.node===node&&tab.file===file){node.dataset.renderState='error';node.textContent='문서를 표시하지 못했어요. 파일을 다시 읽어 주세요.';}}).finally(()=>{if(tab.renderingFile===file)tab.renderingFile=null;});
  }
  function makeTab(path,name) {
    const item=doc.createElement('div');item.className='workspace-tab';
    const button=doc.createElement('button');button.type='button';button.textContent=name;button.setAttribute('role','tab');button.setAttribute('aria-selected',String(tabs.active===path));button.setAttribute('aria-controls',path==='chat'?'chat-workspace':'file-workspace');button.tabIndex=tabs.active===path?0:-1;button.dataset.tabPath=path;button.addEventListener('click',()=>tabs.activate(path));item.append(button);
    if(path!=='chat'){const close=doc.createElement('button');close.type='button';close.textContent='×';close.className='tab-close';close.setAttribute('aria-label',`${name} 탭 닫기`);close.addEventListener('click',()=>tabs.close(path));item.append(close);}bar.append(item);
  }
  function render() {
    const changed=activePath!==tabs.active;if(changed)remember();activePath=tabs.active;
    const structure=JSON.stringify([...tabs.tabs.values()].map(tab=>[tab.path,tab.name]));
    if(structure!==tabKey||!bar.children.length){tabKey=structure;bar.replaceChildren();makeTab('chat','대화');for(const tab of tabs.tabs.values())makeTab(tab.path,tab.name);}
    for(const button of bar.querySelectorAll('[role="tab"]')){button.setAttribute('aria-selected',String(button.dataset.tabPath===tabs.active));button.tabIndex=button.dataset.tabPath===tabs.active?0:-1;}
    const isFile=tabs.active!=='chat';chat.hidden=isFile;panel.hidden=!isFile;
    // Inactive ready previews stay connected but hidden. Async diagram work can
    // settle while another tab is visible; close/reset removes and clears them.
    for(const child of [...content.children])if(![...tabs.tabs.values()].some(tab=>tab.node===child))child.remove();
    for(const tab of tabs.tabs.values()){
      if(tab.status!=='ready'){tab.node?.remove();tab.node=null;tab.renderingFile=null;continue;}
      if(!tab.node){const node=doc.createElement('article');tab.node=node;node.dataset.filePreview='';node.className=tab.file.kind==='markdown'?'markdown document-markdown file-preview':'file-text-preview';
        if(tab.file.kind!=='markdown'){const pre=doc.createElement('pre');pre.textContent=tab.file.content;node.append(pre);node.dataset.renderState='ready';}content.append(node);
      }
      tab.node.hidden=tab.path!==tabs.active;
      if(tab.file.kind==='markdown'&&tab.node.dataset.renderState!=='ready')renderMarkdown(tab,tab.node,tab.file);
    }
    if(isFile){const tab=tabs.tabs.get(tabs.active);doc.getElementById('file-title').textContent=tab.name;doc.getElementById('file-path').textContent=tab.path;status.hidden=tab.status==='ready';status.className=tab.status==='error'?'notice error':'loading';status.textContent=tab.status==='error'?tab.error:tab.status==='ready'?'':'파일을 읽고 있어요…';
      if(tab.status==='ready'&&tab.anchor!==undefined){const anchor=tab.anchor;delete tab.anchor;void Promise.resolve(tab.ready).then(()=>module()).then(({scrollToDocumentAnchor})=>{if(tabs.active===tab.path&&tabs.tabs.get(tab.path)===tab&&tab.node?.isConnected)scrollToDocumentAnchor(tab.node,anchor);});}
    }
    for(const button of tree.querySelectorAll('[data-file-path]'))button.classList.toggle('active',button.dataset.filePath===tabs.active);
    onActive(tabs.active,{changed});
    if(changed){const current=epoch,path=tabs.active,top=isFile?tabs.tabs.get(path)?.scroll||0:chatScroll;win.requestAnimationFrame?.(()=>{if(current===epoch&&tabs.active===path)win.scrollTo?.({top,behavior:'instant'});});}
  }
  async function open(path,anchor=null){const entry=matchCatalogPath(path,catalog);if(!entry){announce('이 프로젝트에서 볼 수 있는 파일이 아니에요.');return;}remember();const pending=tabs.open(entry);const tab=tabs.tabs.get(entry.path);if(anchor!==null&&tab)tab.anchor=anchor;await pending;if(tab?.status==='ready'&&tabs.active===tab.path)render();}
  function build(entries,parent){for(const entry of entries){const row=doc.createElement('li');if(entry.type==='directory'){const details=doc.createElement('details'),summary=doc.createElement('summary');summary.textContent=entry.name;details.append(summary);const list=doc.createElement('ul');build(entry.children||[],list);details.append(list);row.append(details);summary.addEventListener('contextmenu',event=>menu.open(event,entry,root));}else{const button=doc.createElement('button');button.type='button';button.className='tree-file';button.textContent=entry.name;button.dataset.filePath=entry.path;button.addEventListener('click',()=>void open(entry.path));button.addEventListener('contextmenu',event=>menu.open(event,entry,root));row.append(button);}parent.append(row);}}
  async function load(){treeRequest?.abort();const controller=new AbortController();treeRequest=controller;const current=++epoch;tree.textContent='파일 목록을 불러오는 중…';try{const data=await api('/api/v1/files/tree',{signal:controller.signal});if(current!==epoch)return;catalog=Array.isArray(data.files)?data.files:[];root=data.root_display||'';tree.replaceChildren();const list=doc.createElement('ul');build(data.entries||[],list);tree.append(list);if(data.truncated){const note=doc.createElement('p');note.className='small muted';note.textContent='파일 목록이 많아 일부만 표시했어요.';tree.append(note);}if(!catalog.length)tree.textContent='볼 수 있는 문서나 텍스트 파일이 없어요.';render();}catch(error){if(current===epoch&&!controller.signal.aborted)tree.textContent=error.message||'파일 목록을 불러오지 못했어요.';}}
  doc.getElementById('file-tree-refresh').addEventListener('click',()=>void load());
  doc.getElementById('file-reload').addEventListener('click',()=>{const entry=matchCatalogPath(tabs.active,catalog);if(entry)void tabs.open(entry,{refresh:true});});
  bar.addEventListener('keydown',event=>{if(!event.target.matches('[role="tab"]')||!['ArrowLeft','ArrowRight','Home','End'].includes(event.key))return;event.preventDefault();const paths=['chat',...tabs.tabs.keys()],current=paths.indexOf(tabs.active),index=event.key==='Home'?0:event.key==='End'?paths.length-1:(current+(event.key==='ArrowRight'?1:-1)+paths.length)%paths.length;tabs.activate(paths[index]);[...bar.querySelectorAll('[role="tab"]')].find(button=>button.dataset.tabPath===paths[index])?.focus();});
  render();
  return {load,open,showChat(){tabs.activate('chat');},get active(){return tabs.active;},get context(){const tab=tabs.tabs.get(tabs.active);return tab?.status==='ready'?{file:tab.path,name:tab.name,kind:tab.file.kind,source:'file',container:tab.node}:null;},clear(){epoch++;treeRequest?.abort();catalog=[];root='';chatScroll=0;tabs.clear();tree.replaceChildren();content.replaceChildren();doc.getElementById('file-title').textContent='';doc.getElementById('file-path').textContent='';status.textContent='';status.hidden=true;},dispose(){this.clear();},tabs};
}
