import test from 'node:test';
import assert from 'node:assert/strict';
import { documentsForDraft } from '../web/app.js';
import { headingSlug, resolveDocumentTarget } from '../web/document-renderer.js';

const catalog = [
  {kind:'architecture',filename:'아키텍처-설계.md',title:'아키텍처 설계'},
  {kind:'drivers',filename:'요구사항과-아키텍처-드라이버.md',title:'요구사항과 아키텍처 드라이버'},
  {kind:'navigation',filename:'네비게이션.md',title:'네비게이션'},
  {kind:'screen_layout',filename:'화면설계.md',title:'화면설계'},
];

test('the server catalog owns package document order, filenames, and labels', () => {
  const draft = {markdown:'# Main',documents:[
    {kind:'screen_layout',markdown:'# Screen',filename:'../../escape.md',title:'model title'},
    {kind:'navigation',markdown:'# Navigation'},
    {kind:'drivers',markdown:'# Drivers'},
    {kind:'unknown',markdown:'# Ignore me',filename:'evil.md'},
  ]};
  const before = JSON.stringify(draft);
  const documents = documentsForDraft({document_types:catalog},draft);
  assert.deepEqual(documents.map(({kind,filename,title}) => ({kind,filename,title})),catalog);
  assert.deepEqual(documents.map(({markdown}) => markdown),['# Main','# Drivers','# Navigation','# Screen']);
  assert.equal(JSON.stringify(draft),before,'view selection must not mutate the immutable draft');
});

test('legacy drafts expose just the principal document, with or without a catalog', () => {
  const old = {markdown:'# Historical draft'};
  assert.deepEqual(documentsForDraft({document_types:catalog},old).map(({kind})=>kind),['architecture']);
  assert.deepEqual(documentsForDraft({},old),[{...catalog[0],markdown:'# Historical draft'}]);
  assert.deepEqual(documentsForDraft({},null),[]);
});

test('model-only kinds, invalid entries, duplicate kinds and paths cannot create document targets', () => {
  const workspace = {document_types:[...catalog,{kind:'navigation',filename:'duplicate.md',title:'duplicate'},
    {kind:'escape',filename:'../escape.md',title:'escape'},
    {kind:'blank',filename:'blank.md',title:42},
  ]};
  const draft = {markdown:'main',documents:[{kind:'navigation',markdown:'safe'},{kind:'escape',markdown:'escape'},
    {kind:'blank',markdown:'blank'},{kind:'drivers',markdown:null},{kind:'model_only',markdown:'bad'}]};
  assert.deepEqual(documentsForDraft(workspace,draft).map(({kind})=>kind),['architecture','navigation']);
});

test('package links resolve only catalog basenames and keep Unicode anchors local', () => {
  assert.deepEqual(resolveDocumentTarget('네비게이션.md#화면-목록',catalog),{kind:'navigation',anchor:'화면-목록'});
  assert.deepEqual(resolveDocumentTarget('./네비게이션.md',catalog),{kind:'navigation',anchor:null});
  assert.deepEqual(resolveDocumentTarget(`${encodeURIComponent('화면설계.md')}#${encodeURIComponent('주요-화면')}`,catalog),{kind:'screen_layout',anchor:'주요-화면'});
  assert.deepEqual(resolveDocumentTarget('네비게이션.md'.normalize('NFD')+'#'+ '화면-목록'.normalize('NFD'),catalog),{kind:'navigation',anchor:'화면-목록'});
  assert.deepEqual(resolveDocumentTarget('#한글-제목',catalog,'drivers'),{kind:'drivers',anchor:'한글-제목'});
  assert.deepEqual(resolveDocumentTarget('#SAME-DOCUMENT',catalog,'drivers'),{kind:'drivers',anchor:'same-document'});
});

test('file traversal, schemes, unknown filenames, queries and malformed encodings are inert', () => {
  for (const href of ['../네비게이션.md','/네비게이션.md','docs/네비게이션.md','네비게이션.md?token=secret',
    '%2f네비게이션.md','..%2f네비게이션.md','..%5c네비게이션.md','%252f네비게이션.md','//host/네비게이션.md',
    'file:네비게이션.md','javascript:alert(1)','data:text/html,hi','https://host/네비게이션.md','unknown.md',
    '네비게이션.md#%GG','네비게이션.md#%00heading','네비게이션.md\u0000','%00네비게이션.md','네비게이션.md\\other']) {
    assert.equal(resolveDocumentTarget(href,catalog),null,href);
  }
});

test('heading slugs match Unicode/NFC punctuation and whitespace contracts', () => {
  assert.equal(headingSlug('화면 목록'.normalize('NFD')),'화면-목록');
  assert.equal(headingSlug('ADR-001: Cache (Proposed)'),'adr-001-cache-proposed');
  assert.equal(headingSlug('  A  B '),'--a--b-');
  assert.equal(headingSlug('강조 원문_값'),'강조-원문_값');
  assert.equal(headingSlug('𠀀 데이터'),'𠀀-데이터');
});
