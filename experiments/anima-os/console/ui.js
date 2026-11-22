
(()=>{
const root=document.getElementById('anima-console'),main=root.querySelector('main'),nav=root.querySelector('nav'),overlay=root.querySelector('.overlay');
const names=['Overview','Network','Storage','Settings'];
const settingsPages=['Display','Updates','Recovery'];
const preference=matchMedia('(prefers-reduced-motion: reduce)');
let motionChoice='system',dialogVersion=0,returnFocus=null,returnSelector=null,closing=false;
const activeMotion=new Set();
const reduced=()=>motionChoice==='reduced'||(motionChoice==='system'&&preference.matches);
const role={press:100,fast:150,base:200};
function animate(element,frames,duration=role.base){
 if(reduced())return Promise.resolve();
 const animation=element.animate(frames,{duration,easing:'cubic-bezier(.2,0,0,1)'});
 activeMotion.add(animation);
 return animation.finished.catch(()=>{}).finally(()=>activeMotion.delete(animation));
}
function syncMotion(){
 root.dataset.reduced=String(reduced());
 if(reduced())for(const animation of activeMotion)animation.finish();
}
preference.addEventListener('change',syncMotion);
syncMotion();
function backgroundInert(value){
 for(const element of root.querySelectorAll('header,nav,main,footer'))element.inert=value;
}
function showPage(html){
 main.querySelectorAll('.page-outgoing').forEach(element=>element.remove());
 const old=main.querySelector('.page-content');
 if(old&&!reduced()){
  old.className='page-outgoing';old.inert=true;old.setAttribute('aria-hidden','true');
  old.querySelectorAll('[id]').forEach(element=>element.removeAttribute('id'));
  animate(old,[{opacity:1},{opacity:0}],role.fast).then(()=>old.remove());
 }else old?.remove();
 const content=document.createElement('div');content.className='page-content';content.innerHTML=html;
 main.append(content);
 animate(content,[{opacity:0},{opacity:1}]);
}

let page='Overview',timer=null;
const icons={"Overview": "data:image/svg+xml;base64,PCEtLSBJQk0gQ2FyYm9uLCBBcGFjaGUtMi4wOyBleGlzdGluZyBBbmltYSBpY29uIGFzc2V0LiAtLT48c3ZnIHhtbG5zPSJodHRwOi8vd3d3LnczLm9yZy8yMDAwL3N2ZyIgdmlld0JveD0iMCAwIDMyIDMyIiBmaWxsPSIjQTRBOEIwIj48cGF0aCBkPSJNMjQgMjFoMnY1aC0yem0tNC01aDJ2MTBoLTJ6bS05IDEwYTUuMDA2IDUuMDA2IDAgMCAxLTUtNWgyYTMgMyAwIDEgMCAzLTN2LTJhNSA1IDAgMCAxIDAgMTAiIC8+PHBhdGggZD0iTTI4IDJINGEyIDIgMCAwIDAtMiAydjI0YTIgMiAwIDAgMCAyIDJoMjRhMi4wMDMgMi4wMDMgMCAwIDAgMi0yVjRhMiAyIDAgMCAwLTItMm0wIDlIMTRWNGgxNFpNMTIgNHY3SDRWNFpNNCAyOFYxM2gyNGwuMDAyIDE1WiIgLz48L3N2Zz4=", "Network": "data:image/svg+xml;base64,PCEtLSBJQk0gQ2FyYm9uLCBBcGFjaGUtMi4wOyBleGlzdGluZyBBbmltYSBpY29uIGFzc2V0LiAtLT48c3ZnIHhtbG5zPSJodHRwOi8vd3d3LnczLm9yZy8yMDAwL3N2ZyIgdmlld0JveD0iMCAwIDMyIDMyIiBmaWxsPSIjQTRBOEIwIj48cGF0aCBkPSJNMzAgMzBoLTh2LThoOFptLTYtMmg0di00aC00Wm0tNC0xSDhhNiA2IDAgMCAxIDAtMTJoMnYySDhhNCA0IDAgMCAwIDAgOGgxMloiIC8+PHBhdGggZD0iTTIwIDIwaC04di04aDhabS02LTJoNHYtNGgtNFoiIC8+PHBhdGggZD0iTTI0IDE3aC0ydi0yaDJhNCA0IDAgMCAwIDAtOEgxMlY1aDEyYTYgNiAwIDAgMSAwIDEybS0xNC03SDJWMmg4Wk00IDhoNFY0SDRaIiAvPjwvc3ZnPg==", "Anima": "data:image/svg+xml;base64,PCEtLSBJQk0gQ2FyYm9uLCBBcGFjaGUtMi4wOyBleGlzdGluZyBBbmltYSBpY29uIGFzc2V0LiAtLT48c3ZnIHhtbG5zPSJodHRwOi8vd3d3LnczLm9yZy8yMDAwL3N2ZyIgdmlld0JveD0iMCAwIDMyIDMyIiBmaWxsPSIjQTRBOEIwIj48cGF0aCBkPSJNMTUgMTJoMnYxOGgtMnptLTMuNjcgNi4yMmE3IDcgMCAwIDEgMC0xMC40NGwxLjM0IDEuNDlhNSA1IDAgMCAwIDAgNy40NlptOS4zNCAwbC0xLjM0LTEuNDlhNSA1IDAgMCAwIDAtNy40NmwxLjM0LTEuNDlhNyA3IDAgMCAxIDAgMTAuNDQiIC8+PHBhdGggZD0iTTguNCAyMS44YTExIDExIDAgMCAxIDAtMTcuNmwxLjIgMS42YTkgOSAwIDAgMCAwIDE0LjRabTE1LjIgMGwtMS4yLTEuNmE5IDkgMCAwIDAgMC0xNC40bDEuMi0xLjZhMTEgMTEgMCAwIDEgMCAxNy42IiAvPjwvc3ZnPg==", "Storage": "data:image/svg+xml;base64,PCEtLSBJQk0gQ2FyYm9uLCBBcGFjaGUtMi4wOyBleGlzdGluZyBBbmltYSBpY29uIGFzc2V0LiAtLT48c3ZnIHhtbG5zPSJodHRwOi8vd3d3LnczLm9yZy8yMDAwL3N2ZyIgdmlld0JveD0iMCAwIDMyIDMyIiBmaWxsPSIjQTRBOEIwIj48cGF0aCBkPSJNMjQgM0g4YTIgMiAwIDAgMC0yIDJ2MjJhMiAyIDAgMCAwIDIgMmgxNmEyIDIgMCAwIDAgMi0yVjVhMiAyIDAgMCAwLTItMm0wIDJ2Nkg4VjVaTTggMTl2LTZoMTZ2NlptMCA4di02aDE2djZaIiAvPjxjaXJjbGUgY3g9IjExIiBjeT0iOCIgcj0iMSIgLz48Y2lyY2xlIGN4PSIxMSIgY3k9IjE2IiByPSIxIiAvPjxjaXJjbGUgY3g9IjExIiBjeT0iMjQiIHI9IjEiIC8+PC9zdmc+", "Display": "data:image/svg+xml;base64,PCEtLSBJQk0gQ2FyYm9uLCBBcGFjaGUtMi4wOyBleGlzdGluZyBBbmltYSBpY29uIGFzc2V0LiAtLT48c3ZnIHhtbG5zPSJodHRwOi8vd3d3LnczLm9yZy8yMDAwL3N2ZyIgdmlld0JveD0iMCAwIDMyIDMyIiBmaWxsPSIjQTRBOEIwIj48cGF0aCBkPSJNMjggNEg0YTIgMiAwIDAgMC0yIDJ2MTZhMiAyIDAgMCAwIDIgMmg4djRIOHYyaDE2di0yaC00di00aDhhMiAyIDAgMCAwIDItMlY2YTIgMiAwIDAgMC0yLTJNMTggMjhoLTR2LTRoNFptMTAtNkg0VjZoMjRaIiAvPjwvc3ZnPg==", "Recovery": "data:image/svg+xml;base64,PCEtLSBJQk0gQ2FyYm9uLCBBcGFjaGUtMi4wOyBleGlzdGluZyBBbmltYSBpY29uIGFzc2V0LiAtLT48c3ZnIHhtbG5zPSJodHRwOi8vd3d3LnczLm9yZy8yMDAwL3N2ZyIgdmlld0JveD0iMCAwIDMyIDMyIiBmaWxsPSIjQTRBOEIwIj48cGF0aCBkPSJNMjcgMTYuNzZ2LTEuNTNsMS45Mi0xLjY4QTIgMiAwIDAgMCAyOS4zIDExbC0yLjM2LTRhMiAyIDAgMCAwLTEuNzMtMWEyIDIgMCAwIDAtLjY0LjFsLTIuNDMuODJhMTEgMTEgMCAwIDAtMS4zMS0uNzVsLS41MS0yLjUyYTIgMiAwIDAgMC0yLTEuNjFoLTQuNjhhMiAyIDAgMCAwLTIgMS42MWwtLjUxIDIuNTJhMTEuNSAxMS41IDAgMCAwLTEuMzIuNzVsLTIuMzgtLjg2QTIgMiAwIDAgMCA2Ljc5IDZhMiAyIDAgMCAwLTEuNzMgMUwyLjcgMTFhMiAyIDAgMCAwIC40MSAyLjUxTDUgMTUuMjR2MS41M2wtMS44OSAxLjY4QTIgMiAwIDAgMCAyLjcgMjFsMi4zNiA0YTIgMiAwIDAgMCAxLjczIDFhMiAyIDAgMCAwIC42NC0uMWwyLjQzLS44MmExMSAxMSAwIDAgMCAxLjMxLjc1bC41MSAyLjUyYTIgMiAwIDAgMCAyIDEuNjFoNC43MmEyIDIgMCAwIDAgMi0xLjYxbC41MS0yLjUyYTExLjUgMTEuNSAwIDAgMCAxLjMyLS43NWwyLjQyLjgyYTIgMiAwIDAgMCAuNjQuMWEyIDIgMCAwIDAgMS43My0xbDIuMjgtNGEyIDIgMCAwIDAtLjQxLTIuNTFaTTI1LjIxIDI0bC0zLjQzLTEuMTZhOC45IDguOSAwIDAgMS0yLjcxIDEuNTdMMTguMzYgMjhoLTQuNzJsLS43MS0zLjU1YTkuNCA5LjQgMCAwIDEtMi43LTEuNTdMNi43OSAyNGwtMi4zNi00bDIuNzItMi40YTguOSA4LjkgMCAwIDEgMC0zLjEzTDQuNDMgMTJsMi4zNi00bDMuNDMgMS4xNmE4LjkgOC45IDAgMCAxIDIuNzEtMS41N0wxMy42NCA0aDQuNzJsLjcxIDMuNTVhOS40IDkuNCAwIDAgMSAyLjcgMS41N0wyNS4yMSA4bDIuMzYgNGwtMi43MiAyLjRhOC45IDguOSAwIDAgMSAwIDMuMTNMMjcuNTcgMjBaIiAvPjxwYXRoIGQ9Ik0xNiAyMmE2IDYgMCAxIDEgNi02YTUuOTQgNS45NCAwIDAgMS02IDZtMC0xMGEzLjkxIDMuOTEgMCAwIDAtNCA0YTMuOTEgMy45MSAwIDAgMCA0IDRhMy45MSAzLjkxIDAgMCAwIDQtNGEzLjkxIDMuOTEgMCAwIDAtNC00IiAvPjwvc3ZnPg=="};
Object.assign(icons,{"locked": "data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAzMiAzMiI+PHBhdGggZD0iTTI0IDE0aC0yVjhhNiA2IDAgMCAwLTEyIDB2Nkg4YTIgMiAwIDAgMC0yIDJ2MTJhMiAyIDAgMCAwIDIgMmgxNmEyIDIgMCAwIDAgMi0yVjE2YTIgMiAwIDAgMC0yLTJNMTIgOGE0IDQgMCAwIDEgOCAwdjZoLThabTEyIDIwSDhWMTZoMTZaIiAvPjwvc3ZnPg==", "password": "data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAzMiAzMiI+PHBhdGggZD0iTTIxIDJhOC45OTggOC45OTggMCAwIDAtOC42MTIgMTEuNjEyTDIgMjR2Nmg2bDEwLjM4OC0xMC4zODhBOSA5IDAgMSAwIDIxIDJtMCAxNmE3IDcgMCAwIDEtMi4wMzItLjMwMmwtMS4xNDctLjM0OGwtLjg0Ny44NDdsLTMuMTgxIDMuMTgxTDEyLjQxNCAyMEwxMSAyMS40MTRsMS4zNzkgMS4zNzlsLTEuNTg2IDEuNTg2TDkuNDE0IDIzTDggMjQuNDE0bDEuMzc5IDEuMzc5TDcuMTcyIDI4SDR2LTMuMTcybDkuODAyLTkuODAybC44NDgtLjg0N2wtLjM0OC0xLjE0N0E3IDcgMCAxIDEgMjEgMTgiIC8+PGNpcmNsZSBjeD0iMjIiIGN5PSIxMCIgcj0iMiIgLz48L3N2Zz4=", "power": "data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAzMiAzMiI+PHBhdGggZD0ibTIyLjUgNS43NGwtMSAxLjczYTExIDExIDAgMSAxLTExIDBsLTEtMS43M2ExMyAxMyAwIDEgMCAxMyAwIiAvPjxwYXRoIGQ9Ik0xNSAyaDJ2MTRoLTJ6IiAvPjwvc3ZnPg==", "renew": "data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAzMiAzMiI+PHBhdGggZD0iTTEyIDEwSDYuNzhBMTEgMTEgMCAwIDEgMjcgMTZoMkExMyAxMyAwIDAgMCA2IDcuNjhWNEg0djhoOFptOCAxMmg1LjIyQTExIDExIDAgMCAxIDUgMTZIM2ExMyAxMyAwIDAgMCAyMyA4LjMyVjI4aDJ2LThoLThaIiAvPjwvc3ZnPg==", "user-avatar": "data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAzMiAzMiI+PHBhdGggZD0iTTE2IDhhNSA1IDAgMSAwIDUgNWE1IDUgMCAwIDAtNS01bTAgOGEzIDMgMCAxIDEgMy0zYTMuMDAzIDMuMDAzIDAgMCAxLTMgMyIgLz48cGF0aCBkPSJNMTYgMmExNCAxNCAwIDEgMCAxNCAxNEExNC4wMTYgMTQuMDE2IDAgMCAwIDE2IDJtLTYgMjQuMzc3VjI1YTMuMDAzIDMuMDAzIDAgMCAxIDMtM2g2YTMuMDAzIDMuMDAzIDAgMCAxIDMgM3YxLjM3N2ExMS45IDExLjkgMCAwIDEtMTIgMG0xMy45OTMtMS40NTFBNSA1IDAgMCAwIDE5IDIwaC02YTUgNSAwIDAgMC00Ljk5MiA0LjkyNmExMiAxMiAwIDEgMSAxNS45ODUgMCIgLz48L3N2Zz4=", "add": "data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAzMiAzMiI+PHBhdGggZD0iTTE3IDE1VjhoLTJ2N0g4djJoN3Y3aDJ2LTdoN3YtMnoiIC8+PC9zdmc+", "arrow-right": "data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAzMiAzMiI+PHBhdGggZD0ibTE4IDZsLTEuNDMgMS4zOTNMMjQuMTUgMTVINHYyaDIwLjE1bC03LjU4IDcuNTczTDE4IDI2bDEwLTEweiIgLz48L3N2Zz4=", "arrow-left": "data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAzMiAzMiI+PHBhdGggZD0ibTE0IDI2bDEuNDEtMS40MUw3LjgzIDE3SDI4di0ySDcuODNsNy41OC03LjU5TDE0IDZMNCAxNnoiIC8+PC9zdmc+", "checkmark": "data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAzMiAzMiI+PHBhdGggZD0ibTEzIDI0bC05LTlsMS40MTQtMS40MTRMMTMgMjEuMTcxTDI2LjU4NiA3LjU4NkwyOCA5eiIgLz48L3N2Zz4=", "close": "data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAzMiAzMiI+PHBhdGggZD0iTTE3LjQxNCAxNkwyNCA5LjQxNEwyMi41ODYgOEwxNiAxNC41ODZMOS40MTQgOEw4IDkuNDE0TDE0LjU4NiAxNkw4IDIyLjU4Nkw5LjQxNCAyNEwxNiAxNy40MTRMMjIuNTg2IDI0TDI0IDIyLjU4NnoiIC8+PC9zdmc+", "security": "data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAzMiAzMiI+PHBhdGggZD0iTTE0IDE2LjU5TDExLjQxIDE0TDEwIDE1LjQxbDQgNGw4LThMMjAuNTkgMTB6IiAvPjxwYXRoIGQ9Im0xNiAzMGwtNi4xNzYtMy4yOTNBMTAuOTggMTAuOTggMCAwIDEgNCAxN1Y0YTIgMiAwIDAgMSAyLTJoMjBhMiAyIDAgMCAxIDIgMnYxM2ExMC45OCAxMC45OCAwIDAgMS01LjgyNCA5LjcwN1pNNiA0djEzYTguOTkgOC45OSAwIDAgMCA0Ljc2NiA3Ljk0MkwxNiAyNy43MzNsNS4yMzQtMi43OUE4Ljk5IDguOTkgMCAwIDAgMjYgMTdWNFoiIC8+PC9zdmc+", "network-3": "data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAzMiAzMiI+PHBhdGggZD0iTTMwIDMwaC04di04aDhabS02LTJoNHYtNGgtNFptLTQtMUg4YTYgNiAwIDAgMSAwLTEyaDJ2Mkg4YTQgNCAwIDAgMCAwIDhoMTJaIiAvPjxwYXRoIGQ9Ik0yMCAyMGgtOHYtOGg4Wm0tNi0yaDR2LTRoLTRaIiAvPjxwYXRoIGQ9Ik0yNCAxN2gtMnYtMmgyYTQgNCAwIDAgMCAwLThIMTJWNWgxMmE2IDYgMCAwIDEgMCAxMm0tMTQtN0gyVjJoOFpNNCA4aDRWNEg0WiIgLz48L3N2Zz4=", "connection-signal": "data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAzMiAzMiI+PHBhdGggZD0iTTE1IDEyaDJ2MThoLTJ6bS0zLjY3IDYuMjJhNyA3IDAgMCAxIDAtMTAuNDRsMS4zNCAxLjQ5YTUgNSAwIDAgMCAwIDcuNDZabTkuMzQgMGwtMS4zNC0xLjQ5YTUgNSAwIDAgMCAwLTcuNDZsMS4zNC0xLjQ5YTcgNyAwIDAgMSAwIDEwLjQ0IiAvPjxwYXRoIGQ9Ik04LjQgMjEuOGExMSAxMSAwIDAgMSAwLTE3LjZsMS4yIDEuNmE5IDkgMCAwIDAgMCAxNC40Wm0xNS4yIDBsLTEuMi0xLjZhOSA5IDAgMCAwIDAtMTQuNGwxLjItMS42YTExIDExIDAgMCAxIDAgMTcuNiIgLz48L3N2Zz4=", "data-base": "data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAzMiAzMiI+PHBhdGggZD0iTTI0IDNIOGEyIDIgMCAwIDAtMiAydjIyYTIgMiAwIDAgMCAyIDJoMTZhMiAyIDAgMCAwIDItMlY1YTIgMiAwIDAgMC0yLTJtMCAydjZIOFY1Wk04IDE5di02aDE2djZabTAgOHYtNmgxNnY2WiIgLz48Y2lyY2xlIGN4PSIxMSIgY3k9IjgiIHI9IjEiIC8+PGNpcmNsZSBjeD0iMTEiIGN5PSIxNiIgcj0iMSIgLz48Y2lyY2xlIGN4PSIxMSIgY3k9IjI0IiByPSIxIiAvPjwvc3ZnPg==", "screen": "data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAzMiAzMiI+PHBhdGggZD0iTTI4IDRINGEyIDIgMCAwIDAtMiAydjE2YTIgMiAwIDAgMCAyIDJoOHY0SDh2MmgxNnYtMmgtNHYtNGg4YTIgMiAwIDAgMCAyLTJWNmEyIDIgMCAwIDAtMi0yTTE4IDI4aC00di00aDRabTEwLTZINFY2aDI0WiIgLz48L3N2Zz4=", "settings": "data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAzMiAzMiI+PHBhdGggZD0iTTI3IDE2Ljc2di0xLjUzbDEuOTItMS42OEEyIDIgMCAwIDAgMjkuMyAxMWwtMi4zNi00YTIgMiAwIDAgMC0xLjczLTFhMiAyIDAgMCAwLS42NC4xbC0yLjQzLjgyYTExIDExIDAgMCAwLTEuMzEtLjc1bC0uNTEtMi41MmEyIDIgMCAwIDAtMi0xLjYxaC00LjY4YTIgMiAwIDAgMC0yIDEuNjFsLS41MSAyLjUyYTExLjUgMTEuNSAwIDAgMC0xLjMyLjc1bC0yLjM4LS44NkEyIDIgMCAwIDAgNi43OSA2YTIgMiAwIDAgMC0xLjczIDFMMi43IDExYTIgMiAwIDAgMCAuNDEgMi41MUw1IDE1LjI0djEuNTNsLTEuODkgMS42OEEyIDIgMCAwIDAgMi43IDIxbDIuMzYgNGEyIDIgMCAwIDAgMS43MyAxYTIgMiAwIDAgMCAuNjQtLjFsMi40My0uODJhMTEgMTEgMCAwIDAgMS4zMS43NWwuNTEgMi41MmEyIDIgMCAwIDAgMiAxLjYxaDQuNzJhMiAyIDAgMCAwIDItMS42MWwuNTEtMi41MmExMS41IDExLjUgMCAwIDAgMS4zMi0uNzVsMi40Mi44MmEyIDIgMCAwIDAgLjY0LjFhMiAyIDAgMCAwIDEuNzMtMWwyLjI4LTRhMiAyIDAgMCAwLS40MS0yLjUxWk0yNS4yMSAyNGwtMy40My0xLjE2YTguOSA4LjkgMCAwIDEtMi43MSAxLjU3TDE4LjM2IDI4aC00LjcybC0uNzEtMy41NWE5LjQgOS40IDAgMCAxLTIuNy0xLjU3TDYuNzkgMjRsLTIuMzYtNGwyLjcyLTIuNGE4LjkgOC45IDAgMCAxIDAtMy4xM0w0LjQzIDEybDIuMzYtNGwzLjQzIDEuMTZhOC45IDguOSAwIDAgMSAyLjcxLTEuNTdMMTMuNjQgNGg0LjcybC43MSAzLjU1YTkuNCA5LjQgMCAwIDEgMi43IDEuNTdMMjUuMjEgOGwyLjM2IDRsLTIuNzIgMi40YTguOSA4LjkgMCAwIDEgMCAzLjEzTDI3LjU3IDIwWiIgLz48cGF0aCBkPSJNMTYgMjJhNiA2IDAgMSAxIDYtNmE1Ljk0IDUuOTQgMCAwIDEtNiA2bTAtMTBhMy45MSAzLjkxIDAgMCAwLTQgNGEzLjkxIDMuOTEgMCAwIDAgNCA0YTMuOTEgMy45MSAwIDAgMCA0LTRhMy45MSAzLjkxIDAgMCAwLTQtNCIgLz48L3N2Zz4=", "activity": "data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAzMiAzMiI+PHBhdGggZD0iTTEyIDI5YTEgMSAwIDAgMS0uOTItLjYyTDYuMzMgMTdIMnYtMmg1YTEgMSAwIDAgMSAuOTIuNjJMMTIgMjUuMjhsOC4wNi0yMS42M0ExIDEgMCAwIDEgMjEgM2ExIDEgMCAwIDEgLjkzLjY4TDI1LjcyIDE1SDMwdjJoLTVhMSAxIDAgMCAxLS45NS0uNjhMMjEgN2wtOC4wNiAyMS4zNUExIDEgMCAwIDEgMTIgMjkiIC8+PC9zdmc+", "document": "data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAzMiAzMiI+PHBhdGggZD0ibTI1LjcgOS4zbC03LTdjLS4yLS4yLS40LS4zLS43LS4zSDhjLTEuMSAwLTIgLjktMiAydjI0YzAgMS4xLjkgMiAyIDJoMTZjMS4xIDAgMi0uOSAyLTJWMTBjMC0uMy0uMS0uNS0uMy0uN00xOCA0LjRsNS42IDUuNkgxOHpNMjQgMjhIOFY0aDh2NmMwIDEuMS45IDIgMiAyaDZ6IiAvPjxwYXRoIGQ9Ik0xMCAyMmgxMnYySDEwem0wLTZoMTJ2MkgxMHoiIC8+PC9zdmc+", "download": "data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAzMiAzMiI+PHBhdGggZD0iTTI2IDI0djRINnYtNEg0djRhMiAyIDAgMCAwIDIgMmgyMGEyIDIgMCAwIDAgMi0ydi00Wm0wLTEwbC0xLjQxLTEuNDFMMTcgMjAuMTdWMmgtMnYxOC4xN2wtNy41OS03LjU4TDYgMTRsMTAgMTB6IiAvPjwvc3ZnPg==", "terminal": "data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAzMiAzMiI+PHBhdGggZD0iTTI2IDQuMDFINmEyIDIgMCAwIDAtMiAydjIwYTIgMiAwIDAgMCAyIDJoMjBhMiAyIDAgMCAwIDItMnYtMjBhMiAyIDAgMCAwLTItMm0wIDJ2NEg2di00Wm0tMjAgMjB2LTE0aDIwdjE0WiIgLz48cGF0aCBkPSJtMTAuNzYgMTYuMThsMi44MiAyLjgzbC0yLjgyIDIuODNsMS40MSAxLjQxbDQuMjQtNC4yNGwtNC4yNC00LjI0eiIgLz48L3N2Zz4=", "bare-metal-server": "data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAzMiAzMiI+PGNpcmNsZSBjeD0iOSIgY3k9IjYiIHI9IjEiIC8+PHBhdGggZD0iTTI2IDEwSDZhMiAyIDAgMCAxLTItMlY0YTIgMiAwIDAgMSAyLTJoMjBhMiAyIDAgMCAxIDIgMnY0YTIgMiAwIDAgMS0yIDJNNiA0djRoMjBWNFoiIC8+PGNpcmNsZSBjeD0iOSIgY3k9IjE2IiByPSIxIiAvPjxwYXRoIGQ9Ik0yNiAyMEg2YTIgMiAwIDAgMS0yLTJ2LTRhMiAyIDAgMCAxIDItMmgyMGEyIDIgMCAwIDEgMiAydjRhMiAyIDAgMCAxLTIgMk02IDE0djRoMjB2LTRaIiAvPjxjaXJjbGUgY3g9IjkiIGN5PSIyNiIgcj0iMSIgLz48cGF0aCBkPSJNMjYgMzBINmEyIDIgMCAwIDEtMi0ydi00YTIgMiAwIDAgMSAyLTJoMjBhMiAyIDAgMCAxIDIgMnY0YTIgMiAwIDAgMS0yIDJNNiAyNHY0aDIwdi00WiIgLz48L3N2Zz4=", "earth": "data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAzMiAzMiI+PHBhdGggZD0iTTE2IDJhMTQgMTQgMCAxIDAgMTQgMTRBMTQuMDE2IDE0LjAxNiAwIDAgMCAxNiAybTUgMy4xMDZhMTIgMTIgMCAwIDEgMi45MTYgMS44OTlMMjMuNTAzIDhIMjFabS03LjYyMiAyMi41OTdBMTEuOTc2IDExLjk3NiAwIDAgMSA4LjExNiA2Ljk3Nkw5LjQ2NSA5aDMuMzQybC0xLjUgNEg3LjI4bC0xLjM4MiA0LjE0OEw4LjQ2NSAyMWg1bDEuNDMyIDIuMTQ4Wk0xNiAyOGMtLjIwMyAwLS40MDItLjAyLS42MDItLjAzbDEuMzk2LTQuMTlhMS45OSAxLjk5IDAgMCAwLS4yMzMtMS43NDFsLTEuNDMyLTIuMTQ4QTIgMiAwIDAgMCAxMy40NjUgMTloLTMuOTNsLTEuNDMyLTIuMTQ4TDguNzIxIDE1SDExdjJoMnYtMi44MTlsMi45MzYtNy44M2wtMS44NzItLjcwMkwxMy41NTcgN2gtMy4wMjJsLS44MDctMS4yMUExMS44IDExLjggMCAwIDEgMTkgNC4zOTRWOGEyLjAwMyAyLjAwMyAwIDAgMCAyIDJoMi41ODZBMiAyIDAgMCAwIDI1IDkuNDE0bC4xNC0uMTRsLjI4Mi0uNjhBMTIgMTIgMCAwIDEgMjcuMyAxMmgtNC43MDFhMS45OSAxLjk5IDAgMCAwLTEuOTcyIDEuNjY1bC0uNTk3IDMuNDQxYTEuOTkgMS45OSAwIDAgMCAuOTkxIDIuMDg2bDIuMTY1IDEuNDY0bDEuNDU4IDMuNjQ2QTExLjk2IDExLjk2IDAgMCAxIDE2IDI4bTguODE1LTguNjU2TDIyLjEgMTcuNTA5bC0uMS0uMDZsLjU5OS0zLjQ0OWg1LjIyYTExLjc0IDExLjc0IDAgMCAxLTEuNzQ0IDguNDk1WiIgLz48L3N2Zz4=", "search": "data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAzMiAzMiI+PHBhdGggZD0ibTI5IDI3LjU4NmwtNy41NTItNy41NTJhMTEuMDE4IDExLjAxOCAwIDEgMC0xLjQxNCAxLjQxNEwyNy41ODYgMjlaTTQgMTNhOSA5IDAgMSAxIDkgOWE5LjAxIDkuMDEgMCAwIDEtOS05IiAvPjwvc3ZnPg==", "unlink": "data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAzMiAzMiI+PHBhdGggZD0ibTMuNTg0IDUuMDAybDEuNDE0LTEuNDE0bDMuNDE2IDMuNDE0TDcgOC40MTd6bTE5Ljk5NyAxOS45OWwxLjQxNS0xLjQxNGwzLjQxNCAzLjQxNmwtMS40MTQgMS40MTR6TTExIDJoMnY0aC0yem0tOSA5aDR2Mkgyem0yNCA4aDR2MmgtNHptLTcgN2gydjRoLTJ6bS0yLjQyLTQuOTNsLTMuNzEgMy43MmE0IDQgMCAxIDEtNS42Ni01LjY2bDMuNzItMy43Mkw5LjUxIDE0TDUuOCAxNy43MmE2IDYgMCAwIDAtLjA2IDguNTRBNiA2IDAgMCAwIDEwIDI4YTYuMDcgNi4wNyAwIDAgMCA0LjMyLTEuOEwxOCAyMi40OVptLTEuMTctMTAuMTRsMy43Mi0zLjcyYTQgNCAwIDEgMSA1LjY2IDUuNjZsLTMuNzIgMy43MkwyMi40OSAxOGwzLjcxLTMuNzJhNiA2IDAgMCAwIC4wNi04LjU0QTYgNiAwIDAgMCAyMiA0YTYuMDcgNi4wNyAwIDAgMC00LjMyIDEuOEwxNCA5LjUxWiIgLz48L3N2Zz4=", "time": "data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAzMiAzMiI+PHBhdGggZD0iTTE2IDMwYTE0IDE0IDAgMSAxIDE0LTE0YTE0IDE0IDAgMCAxLTE0IDE0bTAtMjZhMTIgMTIgMCAxIDAgMTIgMTJBMTIgMTIgMCAwIDAgMTYgNCIgLz48cGF0aCBkPSJNMjAuNTkgMjJMMTUgMTYuNDFWN2gydjguNThsNSA1LjAxeiIgLz48L3N2Zz4=", "chevron-right": "data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAzMiAzMiI+PHBhdGggZD0iTTIyIDE2TDEyIDI2bC0xLjQtMS40bDguNi04LjZsLTguNi04LjZMMTIgNnoiIC8+PC9zdmc+"});
const icon=name=>`<span class="glyph" aria-hidden="true" style="--glyph:url('${icons[name]||icons.settings}')"></span>`;
const utility=(label,action,glyph)=>`<button type="button" class="quiet utility" data-action="${action}" aria-label="${label}" title="${label}">${icon(glyph)}</button>`;
root.querySelector('.identity').innerHTML='<span>Local session · admin</span>'+utility('Lock screen (Ctrl+Alt+L) — node keeps running','lock','locked');
root.querySelector('footer').innerHTML='<span id="node-label">Anima OS</span><div class="actions">'+utility('Turn screen off','screen-off','screen')+utility('Restart computer','restart','renew')+utility('Shut down computer','shutdown','power')+'</div>';

nav.innerHTML='<small>This computer</small>'+names.map(n=>`<button data-page="${n}">${icon(n)}${n}</button>`).join('');
const actionIcons={'screen-off':'screen','update-check':'renew','update-download':'download','update-restart':'renew','update-rollback-prompt':'arrow-left','selfhost':'network-3','sign-in':'user-avatar','signin-complete':'arrow-right','add-computer':'add','switch-account':'user-avatar','overview':'arrow-left','anima':'connection-signal','network':'network-3','close':'close','wifi-toggle':'connection-signal','wifi-password':'connection-signal','guest-password':'connection-signal','wifi-confirm':'checkmark','wifi-details':'settings','network-save':'checkmark','check-hub':'renew','drive-details':'data-base','diagnostics':'activity','report':'document','export':'download','power':'power','restart':'renew','shutdown':'power','power-confirm':'checkmark','lock':'locked','terminal':'terminal','keep-rotation':'checkmark'};
const button=(label,action,primary=false)=>`<button type="button" data-action="${action}" class="${primary?'primary':''}">${icon(/Unlock/.test(label)?'password':label==='Done'?'checkmark':actionIcons[action]||'arrow-right')}<span>${label}</span></button>`;
const rowIcon=name=>name==='Sign in'?'user-avatar':name==='Ready'?'checkmark':/Wi-Fi|network|Wireless/.test(name)?'connection-signal':/Storage|drive|Filesystem|Capacity|Storage/.test(name)?'data-base':/Account|Signed in/.test(name)?'user-avatar':/Workspace|workspace|Anima/.test(name)?'network-3':/Computer|computer|Device/.test(name)?'bare-metal-server':/Security|Encryption/.test(name)?'security':/Internet|Address/.test(name)?'earth':/report/.test(name)?'document':'activity';
const row=(name,detail,right)=>`<div class="row"><div class="row-title">${icon(rowIcon(name))}<div>${name}<small>${detail}</small></div></div><div class="value">${right}</div></div>`;

const head=(title,body)=>`<h1 tabindex="-1">${title}</h1><p>${body}</p>`;
const subhead=(title,body)=>button('Settings','settings-back')+head(title,body);
const pageLink=(name,title,detail)=>`<button type="button" class="setting-link" data-page="${name}">${icon(name)}<span><strong>${title}</strong><small>${detail}</small></span>${icon('arrow-right')}</button>`;
let state=null,available=[],displays=[],signedIn=false,email='',locked=true,pending=0,bridge=null,poll=null,powerAction='',selected=null,updateState=null,updateOffer=null,scanState='not-scanned',scanVersion=0,networkScans=0,refreshFailed=false,updateWorking=false,updatePoll=null,updatePolling=false,downloadPending=false,selfhostHub='';
const waiting=new Map();
const esc=value=>String(value??'').replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
const size=value=>(value/1024**3).toFixed(1)+' GB';
const unitLabel=value=>({active:'Running',inactive:'Stopped',failed:'Needs attention',activating:'Starting',deactivating:'Stopping',unknown:'Unavailable'}[value]||'Not yet checked');
const networkLabel=()=>state?.devices?.find(d=>d.state==='connected')?.connection||'No active connection';
const phaseLabel=value=>({idle:'Not checked',current:'Up to date',staged:'Ready to restart',booting:'Awaiting boot checks','rolled-back':'Previous system restored',checking:'Checking release',downloading:'Downloading',verifying:'Verifying download',importing:'Importing system',staging:'Preparing next boot',interrupted:'Needs review'}[value]||'Not yet checked');
function connectionPanel(){
 if(state?.enrolled)return `<section class="connection"><div><h2>Enrollment saved</h2><p>Monitoring is ${esc(unitLabel(state.daemon).toLowerCase())}. Hub reachability has not been verified.</p></div>${button('Connection details','connection-details')}</section>`;
 if(signedIn)return `<section class="connection"><div><h2>Add this computer</h2><p>Signed in as ${esc(email)}. Add ${esc(state?.hostname||'this computer')} to your account’s hub.</p></div><div class="actions">${button('Add this computer','add-computer',true)}${button('Use another account','switch-account')}</div></section><p class="hint">This enrolls monitoring and the private link. It does not grant new workload or shell permissions.</p>`;
 return `<section class="connection"><div><h2>Connect to Anima</h2><p>Sign in, then confirm that you want to add this computer.</p></div><div class="actions">${button('Sign in to Anima','sign-in',true)}${button('Paste enrollment code','selfhost')}</div></section>`;
}
const wifiReady=()=>state?.devices?.some(d=>d.type==='wifi')&&state.radio==='enabled'&&state.supplicant!=='failed';
const networkSecurity=n=>!n.security||n.security==='--'?'Open network':/802\.1X|EAP/.test(n.security)?'Enterprise network':'Secured network';
function wifiNotice(){
 if(!state)return ['Network information unavailable','Try opening Network again in a moment.'];
 if(!state.devices.some(d=>d.type==='wifi'))return ['No Wi-Fi adapter detected','Use Ethernet, or connect a Wi-Fi adapter to join a wireless network.'];
 if(state.radio==='disabled')return ['Wi-Fi is off','Turn on Wi-Fi to see available networks.'];
 if(state.supplicant==='failed')return ['Wi-Fi needs attention','Open Settings → Diagnostics & recovery to check the Wi-Fi service.'];
 if(state.radio!=='enabled')return ['Wi-Fi status unavailable','Try opening Network again in a moment.'];
 if(scanState==='scanning')return ['Looking for networks…','Available networks will appear here.'];
 if(scanState==='failed')return ['Couldn’t refresh networks','Try again, or check Diagnostics & recovery in Settings.'];
 if(scanState==='empty')return ['No networks found','Move closer to your router, then refresh.'];
 return ['Looking for networks…','Available networks will appear here.'];
}
function wifiRow(n,i){
 const signal=Math.max(0,Math.min(100,Number(n.signal)||0)),level=Math.max(1,Math.ceil(signal/25));
 return `<button class="wifi-network" data-action="wifi-${i}"><span class="wifi-signal" aria-hidden="true">${[1,2,3,4].map(bar=>`<i class="${bar<=level?'lit':''}"></i>`).join('')}</span><span class="network-name"><strong>${esc(n.ssid||'Hidden network')}</strong><small>${networkSecurity(n)} · ${signal}% signal</small></span><span class="network-connect">${networkSecurity(n)==='Enterprise network'?'Details':'Connect'}${icon('arrow-right')}</span></button>`;
}
function networkScreen(){
 const ethernet=(state?.devices||[]).filter(d=>d.type==='ethernet'),wifi=(state?.devices||[]).filter(d=>d.type==='wifi');
 const connected=wifi.some(d=>d.state==='connected'),scanning=scanState==='scanning',notice=wifiNotice();
 const wired=ethernet.map(d=>`<section class="network-card wired"><div class="network-heading">${icon('network-3')}<div><h2>Ethernet</h2><small>${esc(d.connection&&d.connection!=='--'?d.connection:d.device)}</small></div><span class="connection-state ${d.state==='connected'?'connected':''}">${d.state==='connected'?'Connected':d.state.startsWith('connecting')?'Connecting…':'Not connected'}</span></div></section>`).join('');
 const control=wifiReady()?`<button class="quiet" data-action="scan" ${scanning?'disabled':''}>${icon('renew')}${scanning?'Refreshing…':'Refresh'}</button>`:wifi.length&&state?.radio==='disabled'?button('Turn on','radio-on'):'';
 const list=wifiReady()&&available.length&&!scanning?`<div class="wifi-list" aria-label="Available Wi-Fi networks">${available.map(wifiRow).join('')}</div>`:`<div class="network-empty" role="status"><strong>${notice[0]}</strong><p>${notice[1]}</p></div>`;
 return head('Network','Manage your Ethernet and Wi-Fi connections.')+`<div class="network-settings">${wired}<section class="network-card"><div class="network-heading">${icon('connection-signal')}<div><h2>Wi-Fi</h2><small>${connected?'Connected':wifi.length?state?.radio==='disabled'?'Off':'Wireless networks':'Not available'}</small></div>${control}</div>${list}</section></div>`;
}
async function scanNetworks(){
 if(!wifiReady()||scanState==='scanning'||locked)return;
 const version=++scanVersion;networkScans++;scanState='scanning';available=[];if(page==='Network')draw();
 try{
  const networks=await call('scan');if(locked||version!==scanVersion||!wifiReady())return;
  const strongest=new Map();
  for(const n of networks.sort((a,b)=>Number(b.signal)-Number(a.signal))){const key=JSON.stringify([n.device,n.ssid||n.bssid,n.security]);if(!strongest.has(key))strongest.set(key,n);}
  available=[...strongest.values()];scanState=available.length?'found':'empty';
 }catch(error){if(locked||version!==scanVersion)return;scanState='failed';available=[];}
 finally{networkScans--;if(!locked&&version===scanVersion&&page==='Network')draw();}
}
function wifiDialog(network){
 selected=network;if(!selected)return;
 const security=networkSecurity(selected),title=esc(selected.ssid||'Hidden network');
 if(security==='Enterprise network'){modal('Enterprise Wi-Fi',`<p>${title}</p><p>This network requires advanced configuration. Open Settings → Diagnostics & recovery for recovery options.</p>`,button('Close','close'));return;}
 modal('Connect to Wi-Fi',`<p class="wifi-dialog-name">${title}</p><form id="wifi-form">${security==='Open network'?'<p>This is an open network. No password is required.</p>':'<label>Network password<input name="password" type="password" autocomplete="off"></label>'}<button class="primary">${icon('checkmark')}Connect</button></form>`,button('Cancel','close'));
}
function renderOperation(){
 const panel=main.querySelector('#operation-status');if(!panel)return;
 const operation=updateState?.operation;
 panel.hidden=!updateWorking&&!operation;
 if(panel.hidden)return;
 const measured=Number.isFinite(operation?.bytes)&&Number.isFinite(operation?.totalBytes)&&operation.totalBytes>0;
 panel.innerHTML=`<strong>OS update · ${phaseLabel(operation?.phase||'downloading')}</strong>${measured?`<span>${size(operation.bytes)} of ${size(operation.totalBytes)}</span><progress max="${operation.totalBytes}" value="${operation.bytes}" aria-label="Update transfer"></progress>`:`<span>${operation?.phase==='interrupted'?'The update did not finish. Review its status before trying again.':'Working. You can keep using local controls.'}</span>`}${page!=='Updates'?pageLink('Updates','View update','Progress and restart options'):''}`;
}
async function pollUpdate(){
 if(updatePolling||locked)return;updatePolling=true;
 try{updateState=await call('update-status');if(!downloadPending&&(!updateState.operation||updateState.operation.phase==='interrupted')){clearInterval(updatePoll);updatePoll=null;updateWorking=false;draw();}else renderOperation();}catch(error){root.querySelector('#node-label').textContent='Update progress unavailable · retrying';}finally{updatePolling=false;}
}
async function downloadUpdate(){
 if(updateWorking)return;updateWorking=true;downloadPending=true;draw();
 updatePoll=setInterval(pollUpdate,1000);pollUpdate();
 try{updateState=await call('update-download');updateOffer=null;}
 finally{clearInterval(updatePoll);updatePoll=null;updateWorking=false;downloadPending=false;if(!locked)draw();}
}
function draw(){
 root.querySelector('#node-label').textContent=state?.hostname?state.hostname+' · Anima OS':'Anima OS';
 nav.querySelectorAll('button').forEach(b=>b.setAttribute('aria-current',b.dataset.page===(settingsPages.includes(page)?'Settings':page)?'page':'false'));
 const screens={
 Overview:()=>head(esc(state?.hostname||'This computer'),'Local management · Anima OS')+connectionPanel()+(state?.daemon==='failed'?`<div class="notice warning">Monitoring needs attention. ${pageLink('Recovery','Open diagnostics','Check the local service')}</div>`:'')+(updateState?.phase==='staged'?`<div class="notice">An OS update is ready. ${pageLink('Updates','Review update','Restart only when you are ready')}</div>`:'')+`<section class="section"><h2>Local resources</h2><div id="telemetry-readings"></div></section><div class="summary-grid"><section><h2>Network</h2><p>${esc(networkLabel())}</p>${pageLink('Network','Network settings','Adapters and Wi-Fi')}</section><section><h2>Persistent storage</h2><p>${state?size(state.storage.total-state.storage.used)+' available of '+size(state.storage.total):'Not yet checked'}</p>${pageLink('Storage','Storage details','Disks and capacity')}</section></div>`,
 Network:networkScreen,
 Storage:()=>head('Storage','Disks and persistent capacity on this computer.')+(state?`<div class="numbers"><div><div class="overline">Used</div><div class="number">${size(state.storage.used)}</div></div><div><div class="overline">Available</div><div class="number">${size(state.storage.total-state.storage.used)}</div></div><div><div class="overline">Total</div><div class="number">${size(state.storage.total)}</div></div></div>`:'<p>Storage information is not yet available.</p>')+(state?.drives||[]).filter(d=>d.type==='disk').map(d=>row(esc(d.model||d.name),esc('/dev/'+d.name),size(d.size))).join('')+'<div class="notice">Persistent data is separate from the OS. An OS rollback does not restore older workload data.</div>',
 Settings:()=>head('Settings','Display preferences, system updates and local recovery.')+`<div class="settings-list">${pageLink('Display','Display & accessibility','Orientation and reduced motion')}${pageLink('Updates','OS updates','Check, stage or roll back a verified update')}${pageLink('Recovery','Diagnostics & recovery','Check services and find recovery options')}</div>`,
 Display:()=>subhead('Display & accessibility','Keep this screen readable without interrupting background services.')+`<form id="display-form"><label>Display<select name="output">${displays.map(d=>`<option value="${esc(d.name)}">${esc(d.name)}</option>`).join('')}</select></label><label>Orientation<select name="rotation"><option value="normal">Landscape</option><option value="left">Portrait left</option><option value="right">Portrait right</option><option value="inverted">Landscape flipped</option></select></label><button class="primary" ${displays.length?'':'disabled'}>${icon('checkmark')}Apply display settings</button></form>${displays.length?'':'<p>No configurable display is available. Return to Settings and try again.</p>'}<div class="notice">Unconfirmed changes revert after 15 seconds. Confirmed orientation is saved for future boots.</div><label>Interface motion<select id="motion-choice"><option value="system" ${motionChoice==='system'?'selected':''}>Follow system</option><option value="reduced" ${motionChoice==='reduced'?'selected':''}>Reduced motion</option></select></label><section class="section"><h2>Screen power</h2><p>Turn the screen off and lock this console. Press a key, move the mouse or touch the screen to wake it. Background services keep running.</p>${button('Turn screen off','screen-off')}</section>`,
 Updates:()=>subhead('OS updates','Verified system updates retain your persistent data.')+row('Update status','This computer',phaseLabel(updateState?.phase))+(updateState?.version?row('Release',esc(updateState.version),phaseLabel(updateState.phase)):'')+`<div class="actions">${button('Check for updates','update-check')}${updateState?.phase==='staged'?button('Restart to update','update-restart',true):''}${updateState?.previous?button('Roll back OS','update-rollback-prompt'):''}</div>`+(updateOffer?.available?`<div class="section"><h2>Update ${esc(updateOffer.release.version)}</h2><p>${size(updateOffer.release.bytes)} download. The current system is retained for recovery.</p>${button('Download update','update-download',true)}</div>`:updateOffer?'<p class="empty-state">No newer system is available.</p>':'')+'<div class="notice">Downloading keeps services running. Restarting interrupts work; finish it first. OS rollback does not rewind your data.</div>',
 Recovery:()=>subhead('Diagnostics & recovery','Check this computer even when the hub is unavailable.')+row('System diagnostics','Radio, network and monitoring checks',button('Run checks','diagnostics'))+pageLink('Updates','OS rollback','Review the previous system and restart deliberately')+'<details><summary>Advanced recovery</summary><p>Press Ctrl+Alt+F2 and log in as admin. Return with Ctrl+Alt+F7. OS rollback is also available from the boot menu.</p><p>These options do not restore backups or erase your data.</p></details>'};
 const scroll=main.scrollTop,active=document.activeElement,action=main.contains(active)?active?.dataset?.action:null;
 showPage('<section id="operation-status" class="operation-status" role="status" hidden></section>'+(refreshFailed?'<p class="notice warning" role="status">The last refresh failed. Shown readings may be out of date.</p>':'')+screens[page]());
 main.scrollTop=scroll;
 if(action)main.querySelector(`[data-action="${CSS.escape(action)}"]`)?.focus({preventScroll:true});
 renderTelemetry();renderOperation();
}
function close(immediate=false){
 clearInterval(timer);timer=null;
 if(overlay.hidden||closing)return;
 closing=true;const version=++dialogVersion;
 const finish=()=>{
  if(version!==dialogVersion)return;
  overlay.hidden=true;overlay.innerHTML='';closing=false;
  root.querySelector('.shell').removeAttribute('data-locked');
  backgroundInert(false);
  if(returnFocus?.isConnected)returnFocus.focus();else (root.querySelector(returnSelector||'.page-content h1')||main.querySelector('h1'))?.focus({preventScroll:true});
 };
 if(immediate||reduced()){finish();return;}
 animate(overlay,[{opacity:getComputedStyle(overlay).opacity},{opacity:0}],role.fast).then(finish);
}
function modal(title,body,actions){
 ++dialogVersion;closing=false;
 if(overlay.hidden){returnFocus=document.activeElement;const action=returnFocus?.dataset?.action,route=returnFocus?.dataset?.page;returnSelector=action?`[data-action="${CSS.escape(action)}"]`:route?`[data-page="${CSS.escape(route)}"]`:null;}
 overlay.getAnimations({subtree:true}).forEach(animation=>animation.cancel());
 clearInterval(timer);timer=null;
 overlay.innerHTML=`<section class="dialog" role="dialog" aria-modal="true" aria-label="${title}"><h2>${icon(/locked|sign-in/i.test(title)?'locked':/Restart|Shut down|Power/.test(title)?'power':/Wi-Fi|network/i.test(title)?'connection-signal':'settings')}${title}</h2>${body}<div class="actions">${actions}</div></section>`;
 overlay.hidden=false;backgroundInert(true);
 const locked=title==='Local session locked';
 root.querySelector('.shell').toggleAttribute('data-locked',locked);
 if(!locked){
  animate(overlay,[{opacity:0},{opacity:1}],role.fast);
  animate(overlay.querySelector('.dialog'),[{transform:'translateY(6px)',opacity:0},{transform:'translateY(0)',opacity:1}]);
 }
 overlay.querySelector('input,button')?.focus();
}
function call(op,args={}){
 if(!bridge)return Promise.reject(Error('The local service is starting.'));
 const id=op==='unlock'?'unlock':crypto.randomUUID();
 if(waiting.has(id))return Promise.reject(Error('Wait for the current request.'));
 pending++;
 return new Promise((resolve,reject)=>{waiting.set(id,{resolve,reject});bridge.request(JSON.stringify({id,op,args}));});
}
function errorMessage(error){
 let message=overlay.querySelector('.message');
 if(overlay.hidden){modal('Unable to finish','<p class="message"></p>',button('Close','close'));message=overlay.querySelector('.message');}
 if(!message){message=document.createElement('p');message.className='message';overlay.querySelector('.dialog').append(message);}
 message.textContent=error.message;
 if(!locked&&!overlay.querySelector('[data-action=close]'))overlay.querySelector('.actions').innerHTML=button('Close','close');
}
function lock(){
 locked=true;signedIn=false;email='';state=null;available=[];scanState='not-scanned';scanVersion++;selfhostHub='';
 clearInterval(poll);poll=null;clearInterval(updatePoll);updatePoll=null;updateWorking=false;
 close(true);
 modal('Local session locked','<p>Your node stays awake. Apps, monitoring and authorized remote access continue running.</p><form id="unlock-form"><label>Local admin password<input type="password" name="password" autocomplete="current-password" required></label><button class="primary" type="submit">'+icon('password')+'Unlock</button></form>','');
}
const networkSignature=value=>JSON.stringify([value?.radio,value?.supplicant,(value?.devices||[]).filter(d=>d.type==='wifi').map(d=>d.device).sort()]);
async function refresh(redraw=true){
 try{
  const next=await call('status');if(locked)return;
  if(networkSignature(state)!==networkSignature(next)){scanVersion++;scanState='not-scanned';available=[];}
  state=next;refreshFailed=false;
 }catch(error){refreshFailed=true;throw error;}
 finally{if(redraw&&!locked){draw();if(page==='Network'&&scanState==='not-scanned')void scanNetworks();}}
}
async function go(n,focus='h1'){
 if(!names.includes(n)&&!settingsPages.includes(n))return;page=n;close(true);main.scrollTop=0;draw();
 if(n==='Updates'){updateState=await call('update-status');draw();}
 if(n==='Display'){displays=await call('displays');draw();}
 if(n==='Network'){await refresh(false);draw();if(scanState==='not-scanned')void scanNetworks();}
 main.querySelector(focus)?.focus({preventScroll:true});
}
async function busy(op,args,label){
 modal(label,'<p>Please wait. This is a real operation on this computer.</p>','');
 try{const result=await call(op,args);close(true);return result;}catch(error){errorMessage(error);throw error;}
}
root.addEventListener('click',async e=>{
 const b=e.target.closest('button');if(!b)return;
 const a=b.dataset.action;
 if(a==='lock'){bridge.lock();return;}
 if(a==='screen-off'){bridge.screenOff();return;}
 if(locked)return;
 if(pending&&!updateWorking&&!(networkScans&&b.dataset.page))return;
 try{
  if(b.dataset.page){await go(b.dataset.page);return;}
  if(a==='close'){close();return;}
  if(a==='settings-back'){const previous=page;await go('Settings',`[data-page="${previous}"]`);return;}
  if(a==='connection-details'){modal('Connection details',row('Enrollment','Saved on this computer','Saved')+row('Monitoring','Local process state',unitLabel(state?.daemon))+'<p>Hub reachability and work readiness have not been verified. Locking the screen keeps background services running.</p>',button('Done','close',true));return;}
  if(a==='scan'){await scanNetworks();return;}
  if(a==='radio-on'){await busy('radio-on',{},'Enabling Wi-Fi');state=await call('status');scanState='not-scanned';await go('Network');return;}
  if(a?.startsWith('wifi-')){wifiDialog(available[Number(a.slice(5))]);return;}
  if(a==='update-check'){updateOffer=await busy('update-check',{},'Checking for updates');updateState=updateOffer.state;draw();}
  if(a==='update-download'){await downloadUpdate();}
  if(a==='update-restart'||a==='update-rollback-prompt'){powerAction=a==='update-restart'?'update-activate':'update-rollback';modal('Restart this computer?', '<p>Finish or stop running jobs before continuing. Services and remote access stop during restart. Persistent data is retained.</p>',button('Cancel','close')+button('Restart now','power-confirm',true));}
  if(a==='selfhost'){modal('Connect with an enrollment code',`<p>On another machine, create this device in your hub and copy its enrollment code. Paste the code and hub address below. The hub must be reachable over your private network with a valid HTTPS certificate.</p><form id="selfhost-form"><label>Hub address<input type="url" name="hub" placeholder="https://hub.example" value="${esc(selfhostHub)}" required></label><label>Enrollment code<input type="password" name="code" autocomplete="off" required></label><button class="primary">${icon('connection-signal')}Connect this computer</button></form>`,button('Cancel','close'));}
  if(a==='switch-account'){await call('cancel-sign-in');signedIn=false;email='';draw();}
  if(a==='sign-in'||a==='switch-account'){
   await busy('sign-in',{},'Opening secure sign-in');
   modal('Continue in your browser','<p>Sign in here, or choose QR / code in the secure sign-in window to use your phone or another computer. Anima will return here automatically.</p>',button('Cancel sign-in','cancel-sign-in'));
   poll=setInterval(async()=>{if(pending||locked)return;try{const result=await call('sign-in-poll');if(result.signedIn){clearInterval(poll);poll=null;signedIn=true;email=result.email;close(true);draw();}}catch(error){clearInterval(poll);poll=null;errorMessage(error);}},1000);
  }
  if(a==='cancel-sign-in'){clearInterval(poll);poll=null;await call('cancel-sign-in');close();draw();}
  if(a==='add-computer'){await busy('enroll',{},'Adding this computer');signedIn=false;await refresh();}
  if(a==='diagnostics'){const result=await busy('diagnostics',{},'Checking the system');modal('System diagnostics',Object.entries(result).map(([k,v])=>row(esc(k),'',esc(v))).join(''),button('Done','close',true));}
  if(a==='restart'||a==='shutdown'){powerAction=a;modal(a==='restart'?'Restart this computer?':'Shut down this computer?','<p>This stops services and disconnects remote access. Running work may be interrupted.</p>',button('Cancel','close')+button(a==='restart'?'Restart':'Shut down','power-confirm',true));}
  if(a==='power-confirm'){await busy(powerAction,{confirmed:true},'Stopping this computer');}
  if(a==='keep-rotation'){await call('keep-rotation');close();}
  if(a==='revert'){await call('revert');close();}
 }catch(error){if(a==='add-computer'){signedIn=false;email='';draw();}errorMessage(error);}
});
root.addEventListener('submit',async e=>{
 e.preventDefault();const form=e.target;if(pending&&!updateWorking)return;
 try{
  if(form.id==='unlock-form'){
   const password=form.elements.password.value;form.elements.password.value='';
   await call('unlock',{password});locked=false;close(true);await refresh();
   try{updateState=await call('update-status');if(updateState.operation&&updateState.operation.phase!=='interrupted'){updateWorking=true;updatePoll=setInterval(pollUpdate,1000);}draw();}catch(error){root.querySelector('#node-label').textContent='Update status unavailable · open Settings to retry';}return;
  }
  if(locked)return;
  if(form.id==='selfhost-form'){
   const hub=form.elements.hub.value,code=form.elements.code.value;selfhostHub=hub;form.elements.code.value='';
   await busy('enroll-selfhost',{hub,code},'Connecting to your hub');await refresh();
  }
  if(form.id==='wifi-form'){
   const password=form.elements.password?.value||'';if(form.elements.password)form.elements.password.value='';
   await busy('connect',{device:selected.device,bssid:selected.bssid,password},'Connecting to Wi-Fi');await refresh();
  }
  if(form.id==='display-form'){
   await call('rotate',{output:form.elements.output.value,rotation:form.elements.rotation.value});
   const deadline=Date.now()+15000;
   modal('Keep these display settings?','<p id="revert-count">Reverting in 15 seconds.</p>',button('Revert','revert')+button('Keep settings','keep-rotation',true));
   timer=setInterval(()=>{const left=Math.max(0,Math.ceil((deadline-Date.now())/1000));if(!left){close();return;}overlay.querySelector('#revert-count').textContent=`Reverting in ${left} seconds.`;},200);
  }
 }catch(error){errorMessage(error);}
});
overlay.addEventListener('keydown',e=>{
 if(e.key==='Escape'&&!locked&&!pending&&!overlay.querySelector('#revert-count'))close();
 if(e.key==='Tab'){const controls=[...overlay.querySelectorAll('button,input,select')];const first=controls[0],last=controls.at(-1);if(e.shiftKey&&document.activeElement===first){e.preventDefault();last?.focus();}else if(!e.shiftKey&&document.activeElement===last){e.preventDefault();first?.focus();}}
});
document.addEventListener('keydown',e=>{if(e.ctrlKey&&e.altKey&&e.key.toLowerCase()==='l'&&bridge){e.preventDefault();bridge.lock();}});
root.addEventListener('change',e=>{if(e.target.id==='motion-choice'){motionChoice=e.target.value;localStorage.setItem('motion',motionChoice);syncMotion();}});
root.addEventListener('toggle',e=>{if(e.target.tagName==='DETAILS'&&e.target.open)for(const child of e.target.children)if(child.tagName!=='SUMMARY')animate(child,[{opacity:0},{opacity:1}]);},true);
motionChoice=localStorage.getItem('motion')==='reduced'?'reduced':'system';syncMotion();draw();lock();
new QWebChannel(qt.webChannelTransport,channel=>{
 bridge=channel.objects.console;
 bridge.telemetryChanged.connect(text=>updateTelemetry(JSON.parse(text)));
 bridge.telemetry();
 bridge.answer.connect(text=>{const answer=JSON.parse(text),task=waiting.get(answer.id);if(!task)return;waiting.delete(answer.id);pending=Math.max(0,pending-1);answer.ok?task.resolve(answer.value):task.reject(Error(answer.error));});
 bridge.locked.connect(()=>{for(const task of waiting.values())task.reject(Error('The local session is locked.'));waiting.clear();pending=0;if(!locked)lock();});
 setInterval(()=>{if(!locked&&(!pending||pending===networkScans)&&overlay.hidden)refresh(page==='Overview'||page==='Network').catch(error=>{root.querySelector('#node-label').textContent='Status unavailable · retrying';});},10000);
});

})();
