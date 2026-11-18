export function pathCopyValues(entry, root = '') {
  const path=String(entry?.path || '');
  return {relative:path,absolute:entry?.absolute_path || `${String(root).replace(/\/$/,'')}/${path}`,name:entry?.name || path.split('/').at(-1) || ''};
}

export function createViewerMenu({doc=document,win=window,announce=()=>{}}={}) {
  const menu=doc.createElement('div');menu.className='viewer-context-menu';menu.setAttribute('role','menu');menu.hidden=true;doc.body.append(menu);
  const fallback=doc.createElement('dialog');fallback.className='copy-dialog';fallback.setAttribute('aria-label','텍스트 직접 복사');
  fallback.innerHTML='<div class="dialog-header"><h2>텍스트 복사</h2><button type="button" data-close aria-label="복사 창 닫기">×</button></div><div class="dialog-body"><p>자동 복사를 사용할 수 없어요. 아래 텍스트를 선택해 복사하거나 복사 버튼을 눌러 주세요.</p><label for="manual-copy-text">복사할 텍스트</label><textarea id="manual-copy-text" readonly></textarea></div><div class="dialog-footer"><button type="button" data-close class="secondary">닫기</button><button type="button" data-copy class="primary">복사</button></div>';
  doc.body.append(fallback);let generation=0,restore=null;
  const closeMenu=()=>{menu.hidden=true;menu.replaceChildren();};
  const closeFallback=()=>{fallback.querySelector('textarea').value='';if(fallback.open)fallback.close();restore?.focus?.({preventScroll:true});};
  async function copy(text,label='텍스트') {
    const value=String(text),epoch=generation,origin=fallback.open?restore:menu.hidden?doc.activeElement:restore;closeMenu();
    try{if(!win.navigator?.clipboard?.writeText)throw new Error('unavailable');await win.navigator.clipboard.writeText(value);if(epoch!==generation)return false;if(!fallback.open)origin?.focus?.({preventScroll:true});announce(`${label}를 복사했어요.`);return true;}
    catch{if(epoch!==generation)return false;restore=origin;fallback.querySelector('textarea').value=value;if(!fallback.open)fallback.showModal();fallback.querySelector('textarea').focus();fallback.querySelector('textarea').select();return false;}
  }
  fallback.addEventListener('click',event=>{if(event.target.closest('[data-close]'))closeFallback();if(event.target.closest('[data-copy]'))void copy(fallback.querySelector('textarea').value).then(ok=>{if(ok)closeFallback();});});
  fallback.addEventListener('cancel',event=>{event.preventDefault();closeFallback();});
  const outside=event=>{if(!menu.contains(event.target))closeMenu();};
  const key=event=>{
    if(menu.hidden)return;
    const items=[...menu.querySelectorAll('button')],index=items.indexOf(doc.activeElement);
    if(event.key==='Escape'){event.preventDefault();closeMenu();restore?.focus?.();}
    if(['ArrowDown','ArrowUp','Home','End'].includes(event.key)){event.preventDefault();const next=event.key==='Home'?0:event.key==='End'?items.length-1:(index+(event.key==='ArrowDown'?1:-1)+items.length)%items.length;items[next]?.focus();}
  };
  doc.addEventListener('pointerdown',outside);doc.addEventListener('keydown',key);
  return {
    copy,
    open(event,entry,root){event.preventDefault();closeMenu();restore=event.target;const values=pathCopyValues(entry,root);
      for(const [field,label] of [['relative','상대 경로 복사'],['absolute','절대 경로 복사'],['name','이름 복사']]){const button=doc.createElement('button');button.type='button';button.setAttribute('role','menuitem');button.textContent=label;button.addEventListener('click',()=>void copy(values[field],label.replace(' 복사','')));menu.append(button);}
      menu.hidden=false;const rect=menu.getBoundingClientRect();menu.style.left=`${Math.max(8,Math.min(event.clientX,win.innerWidth-rect.width-8))}px`;menu.style.top=`${Math.max(8,Math.min(event.clientY,win.innerHeight-rect.height-8))}px`;menu.querySelector('button')?.focus();
    },
    clear(){generation++;closeMenu();closeFallback();restore=null;},
    dispose(){generation++;closeMenu();closeFallback();menu.remove();fallback.remove();doc.removeEventListener('pointerdown',outside);doc.removeEventListener('keydown',key);}
  };
}
