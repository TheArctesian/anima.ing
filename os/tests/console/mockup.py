"""Build the synthetic design reference from the same console presentation files."""
import base64
from pathlib import Path

root = Path(__file__).resolve().parents[3]
source = root / 'os/console'
html = (source / 'index.html').read_text()
start = html.index('<meta http-equiv="Content-Security-Policy"')
html = html[:start] + html[html.index('>', start) + 1:]
html = html.replace('<title>Anima</title>', '<title>Anima OS · synthetic graphical preview</title>')
for name in ('style.css', 'lock-screen.css'):
    html = html.replace(f'<link rel="stylesheet" href="{name}">', '<style>\n' + (source / name).read_text() + '\n</style>')
wordmark = base64.b64encode((source.parent / 'branding/wordmark.svg').read_bytes()).decode()
html = html.replace('src="wordmark.svg"', 'src="data:image/svg+xml;base64,' + wordmark + '"')
fixture = Path(__file__).with_name('fixture.js').read_text()
fixture += "\nfixture.scan=[{device:'wlan0',bssid:'00:11:22:33:44:01',ssid:'Workshop',signal:'91',security:'WPA3'},{device:'wlan0',bssid:'00:11:22:33:44:02',ssid:'Guest',signal:'64',security:'--'}];\nsetTimeout(()=>{fixture.signedIn=true;document.querySelector('[name=password]').value='preview';document.querySelector('#unlock-form').requestSubmit();},100);\n"
fixture += """
fixture.builds=[{attempt:'a7320a3c-0000-4000-8000-000000000001',workspace:'abc12345-0000-4000-8000-000000000001',
 phase:'failed',strategy:'railpack',workerProgress:{schema:1,stage:'detecting',failed:true}}];
fixture.workAdmission='hosting-pressure';
fixture.dataVolumes=[{uuid:'6318ea8b-0bfa-44d1-8d7a-92408bbbf178',model:'Workshop SSD',
 mountpoint:'/persist/volumes/6318ea8b-0bfa-44d1-8d7a-92408bbbf178',state:'mounted',
 capacity:{total:42949672960,available:25769803776,used:17179869184}}];
fixture.hosting={setup:'settings-saved',service:'active',socket:true};
fixture.containers=[
 {id:'a'.repeat(64),name:'workshop-web',service:'web',state:'running',status:'Up 1 minute (unhealthy)',health:'unhealthy',image:'example/web',project:'anima-0192f6b07c1e7a3b9d2f4e5a6b7c8d9e',application:'0192f6b0-7c1e-7a3b-9d2f-4e5a6b7c8d9e'},
 {id:'b'.repeat(64),name:'workshop-db',service:'db',state:'exited',status:'Exited',image:'example/db',project:'anima-0192f6b07c1e7a3b9d2f4e5a6b7c8d9e',application:'0192f6b0-7c1e-7a3b-9d2f-4e5a6b7c8d9e'},
 {id:'c'.repeat(64),name:'backup',state:'running',status:'Up',image:'example/backup',project:''}
];
"""
fixture += "\nfixture.attention={entries:[{workspace:'abc12345-0000-4000-8000-000000000001',generation:3,waitingThreads:1}],complete:true};\n"
html = html.replace('<script src="qrc:///qtwebchannel/qwebchannel.js"></script>', '<script>\n' + fixture + '</script>')
for name in ('telemetry.js', 'workloads.js', 'activity.js', 'hero-points.js', 'hero-motion.js',
             'dialog.js', 'remote-access.js', 'maintenance.js', 'presentation.js', 'pages.js',
             'display-settings.js', 'storage.js', 'work-admission.js', 'log-snapshot.js', 'updates.js', 'enrollment.js', 'ui.js'):
    html = html.replace(f'<script src="{name}"></script>', '<script>\n' + (source / name).read_text() + '</script>')
hero = base64.b64encode((source.parent / 'branding/hero.png').read_bytes()).decode()
html = html.replace('src="hero.png"', 'src="data:image/png;base64,' + hero + '"')
html = html.replace('Local session · admin', 'Design preview · synthetic data')
(root / 'docs/mockup/anima-os-console.html').write_text(html + '\n')
