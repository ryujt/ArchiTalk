export function pathCopyValues(entry, root = '') {
  const path=String(entry?.path || '');
  return {relative:path,absolute:entry?.absolute_path || `${String(root).replace(/\/$/,'')}/${path}`,name:entry?.name || path.split('/').at(-1) || ''};
}

function browserCopyReason(error,doc,win) {
  if(win.isSecureContext===false)return '현재 접속 환경에서 브라우저 클립보드 API를 사용할 수 없어요.';
  if(!win.navigator?.clipboard?.writeText)return '이 브라우저에서 클립보드 API를 사용할 수 없어요.';
  if(doc.hasFocus?.()===false)return '브라우저 화면에 포커스가 없어 복사가 거절됐어요.';
  if(error?.name==='NotAllowedError')return '브라우저가 복사를 허용하지 않았어요. 전사 대기 중 클릭 권한이 만료됐거나 클립보드 권한이 제한됐을 수 있어요.';
  return '브라우저 클립보드에 기록하지 못했어요.';
}

function localCopyReason(error) {
  const messages={
    clipboard_unsupported:'이 운영체제에서는 로컬 클립보드 복사를 지원하지 않아요.',
    clipboard_unavailable:'로컬 클립보드 도구를 사용할 수 없어요.',
    clipboard_failed:'로컬 클립보드 도구가 복사를 완료하지 못했어요.',
    clipboard_timeout:'로컬 클립보드 복사 시간이 초과됐어요.',
    clipboard_busy:'다른 로컬 복사 작업이 진행 중이에요.',
    invalid_clipboard_text:'복사할 텍스트의 형식이나 크기를 확인해 주세요.'
  };
  return messages[error?.code] || (error?.status===413?'복사할 텍스트가 로컬 복사 크기 제한을 넘었어요.':'로컬 클립보드 복사도 완료하지 못했어요.');
}

export function createViewerMenu({doc=document,win=window,announce=()=>{},api=null}={}) {
  const menu=doc.createElement('div');menu.className='viewer-context-menu';menu.setAttribute('role','menu');menu.hidden=true;doc.body.append(menu);
  const fallback=doc.createElement('dialog');fallback.className='copy-dialog';fallback.setAttribute('aria-label','텍스트 직접 복사');
  fallback.innerHTML='<div class="dialog-header"><h2>텍스트 복사</h2><button type="button" data-close aria-label="복사 창 닫기">×</button></div><div class="dialog-body"><p data-copy-reason></p><p>복사할 내용은 아래에 남아 있어요. 복사 버튼을 다시 누르거나 텍스트를 선택해 ⌘C / Ctrl+C로 복사해 주세요.</p><label for="manual-copy-text">복사할 텍스트</label><textarea id="manual-copy-text" readonly></textarea></div><div class="dialog-footer"><button type="button" data-close class="secondary">닫기</button><button type="button" data-copy class="primary">복사</button></div>';
  doc.body.append(fallback);let generation=0,restore=null,disposed=false;
  const pending=new Set();
  const invalidate=()=>{generation++;for(const controller of pending)controller.abort();pending.clear();};
  const closeMenu=()=>{menu.hidden=true;menu.replaceChildren();};
  const closeFallback=()=>{fallback.querySelector('textarea').value='';fallback.querySelector('[data-copy-reason]').textContent='';if(fallback.open)fallback.close();restore?.focus?.({preventScroll:true});};
  async function copy(text,label='텍스트') {
    if(disposed)return false;
    invalidate();
    const value=String(text),epoch=generation,origin=fallback.open?restore:menu.hidden?doc.activeElement:restore;
    const current=()=>!disposed&&epoch===generation;
    const complete=()=>{if(!current())return false;if(fallback.open)closeFallback();else origin?.focus?.({preventScroll:true});announce(`${label}를 복사했어요.`);return true;};
    closeMenu();
    let reason;
    // Keep this call before any await so direct clicks retain browser activation.
    try{if(!win.navigator?.clipboard?.writeText)throw new Error('unavailable');await win.navigator.clipboard.writeText(value);return complete();}
    catch(error){if(!current())return false;reason=browserCopyReason(error,doc,win);}
    if(api){
      const controller=new (win.AbortController || globalThis.AbortController)();pending.add(controller);
      try{
        const result=await api('/api/v1/clipboard',{method:'POST',body:{text:value},signal:controller.signal});
        if(!current()||controller.signal.aborted)return false;
        if(result?.copied!==true)throw new Error('invalid clipboard response');
        return complete();
      }catch(error){if(!current()||controller.signal.aborted)return false;reason+=` ${localCopyReason(error)}`;}
      finally{pending.delete(controller);}
    }
    if(!current())return false;
    restore=origin;fallback.querySelector('[data-copy-reason]').textContent=reason;fallback.querySelector('textarea').value=value;
    if(!fallback.open)fallback.showModal();fallback.querySelector('textarea').focus();fallback.querySelector('textarea').select();return false;
  }
  fallback.addEventListener('click',event=>{if(event.target.closest('[data-close]')){invalidate();closeFallback();}if(event.target.closest('[data-copy]'))void copy(fallback.querySelector('textarea').value);});
  fallback.addEventListener('cancel',event=>{event.preventDefault();invalidate();closeFallback();});
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
    clear(){invalidate();closeMenu();closeFallback();restore=null;},
    dispose(){disposed=true;invalidate();closeMenu();closeFallback();menu.remove();fallback.remove();doc.removeEventListener('pointerdown',outside);doc.removeEventListener('keydown',key);}
  };
}
