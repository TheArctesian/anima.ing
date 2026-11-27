//! Only the fixed Docker exec operations needed by an application terminal.
use std::{
    io::{self, BufRead as _, BufReader, Read as _, Write as _},
    os::unix::net::UnixStream,
    time::Duration,
};

use rustix::net::sockopt::socket_peercred;
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::{Value, json};

use crate::{Paths, execution::check_socket, forms::is_lower_hex};

use super::{Error, Target, lifetime::START_GATE};

const TIMEOUT: Duration = Duration::from_secs(2);
const MAX_HEADER: u64 = 8192;
const MAX_JSON: u64 = 16 * 1024;
const STREAM: &str = "application/vnd.docker.raw-stream";

pub(super) struct Engine<'a>(&'a Paths);

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Created {
    id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
pub(super) struct Exec {
    #[serde(rename = "ID")]
    pub id: String,
    #[serde(rename = "ContainerID")]
    pub container: String,
    pub running: bool,
    pub pid: i32,
    pub exit_code: Option<i32>,
}

impl<'a> Engine<'a> {
    pub(super) const fn new(paths: &'a Paths) -> Self {
        Self(paths)
    }

    pub(super) fn create(&self, target: &Target, nonce: &str) -> Result<String, Error> {
        let body = json!({
            "AttachStdin":true,"AttachStdout":true,"AttachStderr":true,
            "Tty":true,"Privileged":false,"User":target.user,"WorkingDir":"/",
            "Cmd":["/bin/sh","-c",START_GATE,"anima-application-terminal",nonce],
            "Env":["TERM=xterm-256color","PS1=anima-app $ "]
        });
        let instance: Created = self.json(
            ("POST", 201),
            &format!("/containers/{}/exec", target.container),
            &body,
        )?;
        if !is_lower_hex(&instance.id, 64) {
            return Err(invalid());
        }
        Ok(instance.id)
    }

    pub(super) fn start(
        &self,
        id: &str,
        (rows, columns): (u16, u16),
    ) -> Result<BufReader<UnixStream>, Error> {
        let body = json!({"Detach":false,"Tty":true,"ConsoleSize":[rows,columns]});
        let (header, socket) = self.request(("POST", true), &format!("/exec/{id}/start"), &body)?;
        if header.status != 101 || !header.raw_stream || !header.upgraded {
            return Err(invalid());
        }
        Ok(socket)
    }

    pub(super) fn inspect(&self, id: &str) -> Result<Exec, Error> {
        self.json(("GET", 200), &format!("/exec/{id}/json"), &Value::Null)
    }

    pub(super) fn resize(&self, id: &str, (rows, columns): (u16, u16)) -> Result<(), Error> {
        let path = format!("/exec/{id}/resize?h={rows}&w={columns}");
        let (header, _) = self.request(("POST", false), &path, &Value::Null)?;
        if !matches!(header.status, 200 | 201 | 204) {
            return Err(invalid());
        }
        Ok(())
    }

    fn json<T: DeserializeOwned>(
        &self,
        (method, status): (&str, u16),
        route: &str,
        body: &Value,
    ) -> Result<T, Error> {
        let (header, socket) = self.request((method, false), route, body)?;
        if header.status != status {
            return Err(invalid());
        }
        let mut bytes = Vec::new();
        socket.take(MAX_JSON + 1).read_to_end(&mut bytes)?;
        if bytes.len() as u64 > MAX_JSON {
            return Err(invalid());
        }
        serde_json::from_slice(&bytes).map_err(|_| invalid())
    }

    fn request(
        &self,
        (method, upgrade): (&str, bool),
        route: &str,
        body: &Value,
    ) -> Result<(Header, BufReader<UnixStream>), Error> {
        check_socket(self.0)?;
        let mut socket = UnixStream::connect(&self.0.socket)?;
        if !socket_peercred(&socket)?.uid.is_root() {
            return Err(Error::Terminal(io::ErrorKind::PermissionDenied));
        }
        socket.set_read_timeout(Some(TIMEOUT))?;
        socket.set_write_timeout(Some(TIMEOUT))?;
        let content = if body.is_null() {
            Vec::new()
        } else {
            serde_json::to_vec(body).map_err(|_| invalid())?
        };
        let (version, connection) = if upgrade {
            ("1.1", "Connection: Upgrade\r\nUpgrade: tcp\r\n")
        } else {
            ("1.0", "Connection: close\r\n")
        };
        write!(
            socket,
            "{method} /v1.45{route} HTTP/{version}\r\nHost: localhost\r\n{connection}Content-Type: application/json\r\nContent-Length: {}\r\n\r\n",
            content.len()
        )?;
        socket.write_all(&content)?;
        let mut socket = BufReader::new(socket);
        let header = Header::read(&mut socket)?;
        Ok((header, socket))
    }
}

struct Header {
    status: u16,
    raw_stream: bool,
    upgraded: bool,
}

impl Header {
    fn read(reader: &mut BufReader<UnixStream>) -> Result<Self, Error> {
        let mut headers = reader.take(MAX_HEADER);
        let mut line = String::new();
        headers.read_line(&mut line)?;
        let mut parts = line.split_ascii_whitespace();
        if !matches!(parts.next(), Some("HTTP/1.0" | "HTTP/1.1")) {
            return Err(invalid());
        }
        let status = parts
            .next()
            .and_then(|part| part.parse().ok())
            .ok_or_else(invalid)?;
        let mut result = Self {
            status,
            raw_stream: false,
            upgraded: false,
        };
        loop {
            line.clear();
            if headers.read_line(&mut line)? == 0 {
                return Err(invalid());
            }
            if line == "\r\n" {
                return Ok(result);
            }
            let (name, value) = line.split_once(':').ok_or_else(invalid)?;
            if name.eq_ignore_ascii_case("content-type") {
                result.raw_stream = value.trim() == STREAM;
            }
            if name.eq_ignore_ascii_case("upgrade") {
                result.upgraded = value.trim().eq_ignore_ascii_case("tcp");
            }
        }
    }
}

fn invalid() -> Error {
    Error::Terminal(io::ErrorKind::InvalidData)
}
