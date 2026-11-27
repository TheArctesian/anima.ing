window.AnimaEnrollment={
 create({modal,button,icon,escape,busy,refresh}){
  let hub='';
  function show(direct){
   const connect='<button class="primary">'+icon('connection-signal')+'Connect this computer</button>';
   const cancel=button('Cancel','close');
   if(direct){
    modal('Connect for monitoring',`
     <p>Create this device in your hub and copy its enrollment code. Use the hub’s reachable HTTPS address.</p>
     <form id="selfhost-form">
      <label>Hub address<input type="url" name="hub" placeholder="https://hub.example" value="${escape(hub)}" required></label>
      <label>Enrollment code<input type="password" name="code" autocomplete="off" required></label>
      ${connect}
     </form>`,button('Back','selfhost')+cancel);
    return;
   }
   modal('Connect to your hub',`
    <p>On a computer signed in to your hub, run <code>anima link install &lt;name&gt;</code>. Paste the install code to set up private SSH, RDP and VNC.</p>
    <form id="install-code-form">
     <label>Install code<input type="password" name="installCode" autocomplete="off" maxlength="4096" required></label>
     ${connect}
    </form>`,button('Monitoring with a hub address','selfhost-direct')+cancel);
  }
  async function submit(form){
   const direct=form.id==='selfhost-form';
   const field=direct?form.elements.code:form.elements.installCode;
   const code=field.value;
   field.value='';
   if(direct){
    hub=form.elements.hub.value;
    await busy('enroll-selfhost',{hub,code},'Connecting to your hub');
   }else{
    await busy('enroll-install',{code},'Setting up private access');
   }
   await refresh();
  }
  return {show,submit,clear:()=>{hub='';}};
 }
};
