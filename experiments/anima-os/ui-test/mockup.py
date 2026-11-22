"""Build the synthetic design reference from the same console presentation files."""
import base64
from pathlib import Path

root = Path(__file__).resolve().parents[3]
source = root / 'experiments/anima-os/console'
html = (source / 'index.html').read_text()
start = html.index('<meta http-equiv="Content-Security-Policy"')
html = html[:start] + html[html.index('>', start) + 1:]
html = html.replace('<title>Anima</title>', '<title>Anima OS · synthetic graphical preview</title>')
html = html.replace('<link rel="stylesheet" href="style.css">', '<style>\n' + (source / 'style.css').read_text() + '\n</style>')
wordmark = base64.b64encode((source.parent / 'branding/wordmark.svg').read_bytes()).decode()
html = html.replace('src="wordmark.svg"', 'src="data:image/svg+xml;base64,' + wordmark + '"')
fixture = Path(__file__).with_name('fixture.js').read_text()
fixture += "\nsetTimeout(()=>{fixture.signedIn=true;document.querySelector('[name=password]').value='preview';document.querySelector('#unlock-form').requestSubmit();},100);\n"
html = html.replace('<script src="qrc:///qtwebchannel/qwebchannel.js"></script>', '<script>\n' + fixture + '</script>')
for name in ('telemetry.js', 'ui.js'):
    html = html.replace(f'<script src="{name}"></script>', '<script>\n' + (source / name).read_text() + '</script>')
html = html.replace('Local session · admin', 'Design preview · synthetic data')
(root / 'docs/mockup/anima-os-console.html').write_text(html + '\n')
