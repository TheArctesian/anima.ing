import http.server
import json
import os
import socket
import socketserver
import ssl
import sys
from pathlib import Path
from urllib.parse import parse_qs

ROOT = '/run/account-fixture'

def record(event):
    with open(ROOT + '/events', 'a') as stream:
        stream.write(json.dumps(event) + '\n')


def link_name():
    return 'anima-' + Path('/etc/machine-id').read_text().strip()[:12]


class Handler(http.server.BaseHTTPRequestHandler):
    def log_message(self, *_):
        pass

    def do_GET(self):
        assert self.server.address_family == socket.AF_UNIX
        assert self.path == '/health'
        self.send_response(200)
        self.end_headers()

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
            assert data['device'] == link_name()
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
        variables = data['variables']
        if 'claimComputerEnrolment' in query:
            assert self.server.address_family == socket.AF_UNIX
            assert self.headers.get('Authorization') is None
            assert 'os:LINUX' in query
            if variables['claim'] != 'B' * 43:
                return {'errors': [{'message': variables['claim']}]}
            return dict(data=dict(claimComputerEnrolment=dict(code='anima_enrol_synthetic')))
        if 'claimDeviceToken' in query:
            assert variables['claim'] == 'synthetic-claim'
            return dict(data=dict(claimDeviceToken=dict(token='synthetic-hub-token')))
        assert self.headers['Authorization'] == 'Bearer synthetic-hub-token'
        creating = Path(ROOT, 'create-device').exists()
        if 'linkDevices' in query:
            value = dict(linkDevices=[dict(name=link_name(), address='fd41:4e49:4d41::42')])
        elif 'devices(' in query:
            if creating:
                assert variables['after'] is None
                value = dict(devices=dict(edges=[], pageInfo=dict(hasNextPage=False)))
            elif variables['after'] is None:
                value = dict(devices=dict(edges=[], pageInfo=dict(hasNextPage=True, endCursor='next')))
            else:
                assert variables['after'] == 'next'
                edge = dict(node=dict(id='existing-device', host='fd41:4e49:4d41:0:0:0:0:42'))
                value = dict(devices=dict(edges=[edge], pageInfo=dict(hasNextPage=False)))
        elif 'sites{' in query:
            value = dict(sites=[dict(id='link-site', name='Link', subnet='fd41:4e49:4d41::/48')])
        elif 'createDevice' in query:
            assert creating
            hostname = Path('/proc/sys/kernel/hostname').read_text().strip()
            assert variables['input']['name'] == hostname
            assert variables['input']['siteId'] == 'link-site'
            assert variables['input']['host'] == 'fd41:4e49:4d41::42'
            for protocol, port in (('ssh', 22), ('rdp', 3389), ('vnc', 5900)):
                settings = variables['input'][protocol]
                assert settings['enabled'] is True
                assert settings['username'] == 'admin' and settings['port'] == port
            assert variables['input']['rdp']['security'] == 'TLS'
            value = dict(createDevice=dict(id='new-device'))
        elif 'enrolDaemon' in query:
            assert variables['id'] == ('new-device' if creating else 'existing-device')
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
