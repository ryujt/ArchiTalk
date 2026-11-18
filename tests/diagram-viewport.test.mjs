import test from 'node:test';
import assert from 'node:assert/strict';
import {diagramFit,diagramOffset,diagramZoomLimit} from '../web/diagram-viewport.js';

test('fit contains wide tall and ordinary diagrams without enlarging native content',()=>{
  for(const [width,height,available] of [[6000,200,330],[200,6000,800],[480,240,860],[720,500,310]]){
    const fit=diagramFit(width,height,available);
    assert.ok(fit.scale>0&&fit.scale<=1);
    assert.ok(width*fit.scale<=fit.width-24+0.001);
    assert.ok(height*fit.scale<=fit.height-24+0.001);
    assert.ok(fit.height>=180&&fit.height<=480);
  }
});

test('pan clamps both enlarged edges and centers content which fits',()=>{
  assert.equal(diagramOffset(10000,400,1000),12);
  assert.equal(diagramOffset(-10000,400,1000),-612);
  assert.equal(diagramOffset(-200,400,1000),-200);
  assert.equal(diagramOffset(10000,400,200),100);
  assert.equal(diagramOffset(-10000,400,200),100);
});

test('invalid or initially hidden measurements do not create nonfinite transforms',()=>{
  for(const values of [[0,0,0],[NaN,Infinity,NaN],[-10,-20,-30]]){
    const fit=diagramFit(...values);
    assert.ok(Object.values(fit).every(Number.isFinite));
    assert.ok(fit.scale>0);
  }
  assert.equal(diagramOffset(NaN,400,1000),0);
});

test('very wide graphs can still zoom to native readable text size',()=>{
  for(const scale of [1,0.5,0.01,0.001]){
    assert.ok(diagramZoomLimit(scale)>=16);
    assert.ok(diagramZoomLimit(scale)*scale>=1);
  }
  assert.equal(diagramZoomLimit(NaN),16);
});
