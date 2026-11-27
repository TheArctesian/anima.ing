// The lock hero changes direction on the same canvas and settles to the still.
(()=>{
 let active=null;
 const stop=()=>active?.finish();
 const artwork={width:3840,height:2160};

 function paint(context,canvas,image,points,position){
  const scale=Math.max(canvas.width/artwork.width,canvas.height/artwork.height);
  context.setTransform(scale,0,0,scale,(canvas.width-artwork.width*scale)/2,(canvas.height-artwork.height*scale)/2);
  context.globalAlpha=1;
  context.fillStyle='#1B1D21';
  context.fillRect(0,0,artwork.width,artwork.height);
  if(image.complete&&image.naturalWidth)context.drawImage(image,0,1780,3840,380,0,1780,3840,380);
  for(let index=0;index<points.length;index++){
   const [x,y,size,color]=points[index];
   const stagger=(index%23)/23*.14;
   const opacity=Math.max(0,Math.min(1,(position-stagger)/(1-stagger)));
   const spread=1-opacity;
   context.globalAlpha=opacity;
   context.fillStyle=color;
   context.fillRect(x+(x-1920)*spread*.24,y+(y-980)*spread*.24-spread*36,size,size);
  }
 }

 function particleMotion(element,image,{canvas,context,points},reduced){
  let request=0,transition;
  const finish=()=>{
   if(active!==motion)return;
   active=null;
   cancelAnimationFrame(request);
   image.style.visibility=transition.target===0?'hidden':'';
   canvas.remove();
   transition.resolve();
  };
  function sample(now){
   const {start,target,position,velocity,rate}=transition;
   const seconds=Math.max(0,now-start)/1000;
   const displacement=position-target;
   const slope=velocity+rate*displacement;
   const decay=Math.exp(-rate*seconds);
   return {position:target+(displacement+slope*seconds)*decay,velocity:(velocity-rate*slope*seconds)*decay};
  }
  function frame(now){
   if(!element.isConnected||!element.checkVisibility()||document.hidden||reduced()||now>=transition.end){finish();return;}
   paint(context,canvas,image,points,sample(now).position);
   request=requestAnimationFrame(frame);
  }
  function retarget(direction){
   const target=direction==='out'?0:1;
   if(transition?.target===target)return transition.done;
   const now=performance.now();
   const start=transition?sample(now):{position:1-target,velocity:0};
   transition?.resolve();
   const duration=target?620:300;
   const done=new Promise(resolve=>{
    transition={...start,start:now,end:now+duration,target,rate:target?16:30,resolve};
   });
   transition.done=done;
   return done;
  }
  const motion={element,finish,retarget,start:()=>{request=requestAnimationFrame(frame);}};
  return motion;
 }

 function play(element,direction,reduced){
  const points=window.AnimaHeroPoints,image=element?.querySelector('img');
  if(direction==='in'&&image)image.style.visibility='';
  if(!element||!image||reduced()||document.hidden||!element.checkVisibility()||!points?.length){stop();return Promise.resolve();}
  if(active?.element===element)return active.retarget(direction);
  stop();
  const width=element.clientWidth,height=element.clientHeight;
  if(!width||!height)return Promise.resolve();
  const canvas=document.createElement('canvas'),ratio=Math.min(devicePixelRatio||1,1.5);
  canvas.width=Math.round(width*ratio);canvas.height=Math.round(height*ratio);
  canvas.setAttribute('aria-hidden','true');
  const context=canvas.getContext('2d',{alpha:false});
  if(!context)return Promise.resolve();
  element.append(canvas);
  active=particleMotion(element,image,{canvas,context,points},reduced);
  const done=active.retarget(direction);
  active.start();
  return done;
 }
 document.addEventListener('visibilitychange',()=>{if(document.hidden)stop();});
 window.AnimaHero={play,stop};
})();
