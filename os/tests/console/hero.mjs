export async function checkHero({js,assert,cmd,shot,until}){
 await cmd('Emulation.setDeviceMetricsOverride',{width:1280,height:800,deviceScaleFactor:1,mobile:false});
 for(const motion of ['no-preference','reduce']){
  await cmd('Emulation.setEmulatedMedia',{features:[{name:'prefers-reduced-motion',value:motion}]});
  await js(`document.querySelector('[name=password]').value='synthetic';document.querySelector('#unlock-form').requestSubmit()`);
  await until(`fixture.pending===0&&document.querySelector('.overlay').hidden`);
  await js(`document.querySelector('[data-action=lock]').click();fixture.heroTransition=!!document.querySelector('.lock-art canvas')`);
  await assert(`fixture.heroTransition===${motion==='no-preference'}`,'Normal lock starts particle motion; reduced motion uses the still');
  await assert(`(()=>{const image=document.querySelector('.lock-art img');return image?.complete&&image.naturalWidth>0&&image.checkVisibility({visibilityProperty:true})&&image.getBoundingClientRect().width>100})()`,'Lock restores a loaded, visible hero after unlock');
  await assert(`!document.querySelector('.lock-art canvas')`,'Hero transition ends without an idle render loop');
  await shot('lock-hero-'+motion);
 }
 for(const [width,height] of [[800,1280],[800,600],[1280,600]]){
  await cmd('Emulation.setDeviceMetricsOverride',{width,height,deviceScaleFactor:1,mobile:false});
  await assert(`(()=>{const r=document.querySelector('.overlay .dialog').getBoundingClientRect();return r.top>=0&&r.bottom<=innerHeight&&r.left>=0&&r.right<=innerWidth})()`,'Lock controls remain within the viewport with the reframed hero');
  await shot('lock-hero-'+width+'x'+height);
 }
 await cmd('Emulation.setDeviceMetricsOverride',{width:1280,height:800,deviceScaleFactor:1,mobile:false});
 await cmd('Emulation.setEmulatedMedia',{features:[{name:'prefers-reduced-motion',value:'no-preference'}]});
 await js(`document.querySelector('[name=password]').value='synthetic';document.querySelector('#unlock-form').requestSubmit()`);
 await until(`fixture.pending===0&&document.querySelector('.overlay').hidden`);
 await js(`document.querySelector('[data-action=lock]').click()`);
 const reversal=await js(`(async()=>{
  const canvas=document.querySelector('.lock-art canvas');
  const form=document.querySelector('#unlock-form');
  form.elements.password.value='synthetic';form.requestSubmit();
  while(!form.closest('.dialog').inert&&canvas?.isConnected)await new Promise(requestAnimationFrame);
  const opening=!!canvas&&canvas===document.querySelector('.lock-art canvas');
  consoleBridge.lock();
  return {opening,relocked:!!canvas&&canvas===document.querySelector('.lock-art canvas'),
    passwordCleared:document.querySelector('#unlock-form')?.elements.password.value===''};
 })()`);
 if(!reversal.opening||!reversal.relocked||!reversal.passwordCleared)throw Error('Rapid unlock and relock must retarget the current hero: '+JSON.stringify(reversal));
 await cmd('Emulation.setEmulatedMedia',{features:[{name:'prefers-reduced-motion',value:'reduce'}]});
 await assert(`!document.querySelector('.lock-art canvas')&&document.querySelector('.lock-art img').checkVisibility({visibilityProperty:true})`,'Reducing motion during lock leaves the still hero visible');
}

export async function checkInstalledMotionPreference({js,assert,cmd,click,carryPreference}){
 await cmd('Emulation.setEmulatedMedia',{features:[{name:'prefers-reduced-motion',value:'no-preference'}]});
 carryPreference();
 await cmd('Page.reload',{ignoreCache:true});
 await assert(`window.animaInstallerReducedMotion===true`,'The synthetic browser received the installer choice');
 await assert(`!!document.querySelector('#unlock-form')&&document.querySelector('#anima-console').dataset.reduced==='true'&&!document.querySelector('.lock-art canvas')`,'The installer choice reduces motion on the first installed lock screen');
 await js(`document.querySelector('[name=password]').value='synthetic';document.querySelector('#unlock-form').requestSubmit()`);
 await assert(`document.querySelector('.overlay').hidden`,'The installed console still unlocks');
 await click('nav [data-page=Settings]');
 await click('main [data-page=Display]');
 await assert(`document.querySelector('#motion-choice').value==='reduced'`,'Display settings reflect the carried preference');
 await js(`const choice=document.querySelector('#motion-choice');choice.value='system';choice.dispatchEvent(new Event('change',{bubbles:true}));`);
 await cmd('Page.reload');
 await assert(`!!document.querySelector('#unlock-form')&&document.querySelector('#anima-console').dataset.reduced==='false'`,'Follow system overrides the installer choice after a reload');
}
