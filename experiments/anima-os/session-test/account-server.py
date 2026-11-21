import http.server
import json
import os
import socketserver
import ssl
import sys
import threading
from urllib.parse import parse_qs

ROOT = '/run/account-fixture'

def record(event):
    with open(ROOT + '/events', 'a') as stream:
        stream.write(json.dumps(event) + '\n')

class Handler(http.server.BaseHTTPRequestHandler):
    def log_message(self, *_):
        pass

    def do_POST(self):
        raw = self.rfile.read(int(self.headers['Content-Length']))
        data = parse_qs(raw.decode()) if 'form-urlencoded' in self.headers.get('Content-Type', '') else json.loads(raw)
        record(dict(path=self.path, data=data, authorization=self.headers.get('Authorization')))
        if self.path == '/graphql':
            answer = self.graph(data)
        elif self.path.endswith('/authorize/device'):
            answer = dict(device_code='synthetic-device-secret', user_code='TEST-CODE', verification_uri_complete='https://login.invalid/verify', interval=5, expires_in=120)
        elif self.path.endswith('/authenticate'):
            assert data['device_code'] == ['synthetic-device-secret']
            answer = dict(access_token='synthetic-access-secret', user=dict(email='owner@example.invalid'))
        elif self.path == '/v1/hub':
            assert self.headers['Authorization'] == 'Bearer synthetic-access-secret'
            answer = dict(state='ready')
        elif self.path == '/v1/hub/access':
            assert self.headers['Authorization'] == 'Bearer synthetic-access-secret'
            answer = dict(invite='synthetic-link-invite', claim='synthetic-claim')
        else:
            raise AssertionError(self.path)
        body = json.dumps(answer).encode()
        self.send_response(200)
        self.send_header('Content-Length', str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def graph(self, data):
        query = data['query']
        if 'claimDeviceToken' in query:
            assert data['variables']['claim'] == 'synthetic-claim'
            return dict(data=dict(claimDeviceToken=dict(token='synthetic-hub-token')))
        assert self.headers['Authorization'] == 'Bearer synthetic-hub-token'
        if 'linkDevices' in query:
            name = 'anima-' + open('/etc/machine-id').read().strip()[:12]
            value = dict(linkDevices=[dict(name=name, address='fd41:4e49:4d41::42')])
        elif 'devices(' in query:
            value = dict(devices=dict(edges=[dict(node=dict(id='existing-device', host='fd41:4e49:4d41::42'))], pageInfo=dict(hasNextPage=False)))
        elif 'enrolDaemon' in query:
            assert data['variables']['id'] == 'existing-device'
            value = dict(enrolDaemon=dict(code='anima_enrol_synthetic'))
        elif 'logout' in query:
            value = dict(logout=True)
        else:
            raise AssertionError(query)
        return dict(data=value)

class UnixServer(socketserver.UnixStreamServer):
    allow_reuse_address = True

os.makedirs(ROOT, exist_ok=True)
if sys.argv[1] == 'https':
    server = http.server.ThreadingHTTPServer(('127.0.0.1', 443), Handler)
    context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    context.load_cert_chain(ROOT + '/leaf.pem', ROOT + '/leaf.key')
    server.socket = context.wrap_socket(server.socket, server_side=True)
else:
    path = '/var/lib/anima-daemon/link.sock'
    if os.path.exists(path):
        os.unlink(path)
    server = UnixServer(path, Handler)
server.serve_forever()
