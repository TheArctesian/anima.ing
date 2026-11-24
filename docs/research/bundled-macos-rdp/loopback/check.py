import hashlib
import os
import secrets
import socket
import ssl
import subprocess
import time

def request(connection, *fields):
    wire = ",".join(f"{len(field)}.{field}" for field in fields) + ";"
    connection.sendall(wire.encode("ascii"))

def instruction(stream):
    fields = []
    while True:
        length = bytearray()
        while (byte := stream.read(1)) != b".":
            if not byte:
                return None
            length += byte
        fields.append(stream.read(int(length)).decode("ascii"))
        if stream.read(1) == b";":
            return fields

def open_session(gateway_port, host_port, password, fingerprint, security="nla"):
    with socket.create_connection(("127.0.0.1", gateway_port), timeout=8) as connection:
        connection.settimeout(8)
        stream = connection.makefile("rb")
        request(connection, "select", "rdp")
        args = instruction(stream)
        assert args[:2] == ["args", "VERSION_1_5_0"], args
        values = {
            "hostname": "127.0.0.1", "port": str(host_port), "username": "synthetic-probe",
            "password": password, "security": security, "cert-fingerprints": fingerprint,
            "disable-copy": "true", "disable-paste": "true",
        }
        request(connection, "size", "64", "64", "96")
        request(connection, "audio")
        request(connection, "video")
        request(connection, "image", "image/png")
        request(connection, "timezone", "UTC")
        request(connection, "name", "synthetic-probe")
        request(connection, "connect", args[1], *(values.get(arg, "") for arg in args[2:]))
        ready = False
        while (result := instruction(stream)) is not None:
            opcode, *values = result
            if opcode == "ready":
                ready = True
            if opcode == "error":
                return ("error", tuple(values))
            if opcode == "cfill" and values[-4:] == ["200", "30", "90", "255"]:
                assert ready
                request(connection, "key", "97", "1")
                request(connection, "key", "97", "0")
                request(connection, "mouse", "8", "8", "0")
                time.sleep(.25)
                return ("painted", tuple(values[-4:]))
            if opcode == "sync":
                request(connection, "sync", *values)
        return ("closed", ())

subprocess.run(["openssl", "req", "-x509", "-newkey", "rsa:2048", "-nodes",
                "-keyout", "key.pem", "-out", "cert.pem", "-days", "1",
                "-subj", "/CN=localhost"], check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
with open("cert.pem", encoding="ascii") as cert:
    digest = hashlib.sha256(ssl.PEM_cert_to_DER_cert(cert.read())).digest()
fingerprint = "sha256:" + digest.hex(":")
with socket.socket() as port:
    port.bind(("127.0.0.1", 0))
    host_port = port.getsockname()[1]
synthetic_password = secrets.token_urlsafe(20)
env = dict(os.environ, ANIMA_RDP_PROBE_PASSWORD=synthetic_password)
with subprocess.Popen(["target/debug/anima-rdp-loopback-probe", str(host_port)], env=env) as host:
    with subprocess.Popen(
        ["/Applications/Anima.ing.app/Contents/MacOS/anima-gateway", "listen", "127.0.0.1:0"],
        stderr=subprocess.PIPE, text=True,
    ) as gateway:
        try:
            gateway_port = int(gateway.stderr.readline().rsplit(":", maxsplit=1)[1])
            for _ in range(40):
                if host.poll() is not None:
                    raise RuntimeError("Synthetic RDP host exited")
                try:
                    with socket.create_connection(("127.0.0.1", host_port), timeout=.25):
                        break
                except ConnectionRefusedError:
                    time.sleep(.05)
            wrong = open_session(gateway_port, host_port, "synthetic-wrong-password", fingerprint)
            print("Incorrect synthetic password:", wrong)
            assert wrong[0] == "error"
            unpinned = open_session(gateway_port, host_port, synthetic_password, "sha256:" + bytes(32).hex(":"))
            print("Wrong synthetic certificate pin:", unpinned)
            assert unpinned[0] == "error"
            tls_only = open_session(gateway_port, host_port, synthetic_password, fingerprint, security="tls")
            print("TLS without NLA:", tls_only)
            assert tls_only[0] == "error"
            correct = open_session(gateway_port, host_port, synthetic_password, fingerprint)
            print("Expected synthetic frame:", correct)
            assert correct[0] == "painted"
        finally:
            gateway.terminate()
            host.terminate()
