export async function checkDisplay({js, click, assert}) {
 await js(`fixture.displays=[{name:'Virtual-1',rotation:'left'},{name:'Virtual-2',rotation:'inverted'}]`);
 await click('main [data-page=Display]');
 await assert(`document.querySelector('#display-form [name=rotation]')?.value==='left'`,
  'Display starts with the observed portrait orientation');
 await js(`{
  const output=document.querySelector('#display-form [name=output]');
  output.focus();output.value='Virtual-2';output.dispatchEvent(new Event('change',{bubbles:true}));
 }`);
 await assert(`document.querySelector('#display-form [name=rotation]').value==='inverted'&&document.activeElement.name==='output'&&!fixture.rotationRequest`,
  'Selecting another display shows its own orientation and keeps focus without applying a change');
 const apply = rotation => js(`{
  const form=document.querySelector('#display-form');
  form.elements.rotation.value='${rotation}';form.requestSubmit();
 }`);
 await apply('right');
 await assert(`fixture.rotationRequest?.output==='Virtual-2'&&fixture.rotationRequest.rotation==='right'&&!!document.querySelector('.dialog [data-action=keep-rotation]')`,
  'A preview applies the requested orientation to the selected display');
 await click('.dialog [data-action=revert]');
 await assert(`{
  const form=document.querySelector('#display-form');
  form.elements.output.value==='Virtual-2'&&form.elements.rotation.value==='inverted';
 }`, 'Revert restores the observed orientation for that same display');
 await apply('normal');
 await click('.dialog [data-action=keep-rotation]');
 await click('[data-action=settings-back]');
 await assert(`document.activeElement.dataset.page==='Display'`, 'Settings back restores focus');
 await click('main [data-page=Display]');
 await assert(`{
  const form=document.querySelector('#display-form');
  form.elements.output.value==='Virtual-2'&&form.elements.rotation.value==='normal';
 }`, 'Display settings reload the observed orientation after Keep');
 await click('[data-action=settings-back]');
 await js(`delete fixture.displays;delete fixture.rotationRequest`);
}
