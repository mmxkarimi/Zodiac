use super::url::SimpleUrl;
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct HttpResponse {
    pub status: u16,
    pub status_text: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    pub final_url: String,
}

impl HttpResponse {
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }

    pub fn get_header(&self, name: &str) -> Option<&str> {
        let lower = name.to_ascii_lowercase();
        for (k, v) in &self.headers {
            if k.to_ascii_lowercase() == lower {
                return Some(v.as_str());
            }
        }
        None
    }
}

#[derive(Debug, Clone)]
pub struct HttpClient {
    pub timeout: Duration,
}

impl Default for HttpClient {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(10),
        }
    }
}

impl HttpClient {
    #[allow(dead_code)]
    pub fn new(timeout: Duration) -> Self {
        Self { timeout }
    }

    pub fn get(&self, initial_url: &str) -> Result<HttpResponse, String> {
        let mut current_url = initial_url.to_string();
        let mut redirects = 0;
        const MAX_REDIRECTS: usize = 5;

        loop {
            let parsed = SimpleUrl::parse(&current_url)?;
            let resp = self.execute_single_get(&parsed)?;

            if (300..=399).contains(&resp.status) {
                if let Some(loc) = resp.get_header("location") {
                    redirects += 1;
                    if redirects > MAX_REDIRECTS {
                        return Err(format!("Too many redirects (> {})", MAX_REDIRECTS));
                    }

                    if loc.starts_with("http://") || loc.starts_with("https://") {
                        current_url = loc.to_string();
                    } else if loc.starts_with('/') {
                        current_url = format!("http://{}:{}{}", parsed.host, parsed.port, loc);
                    } else {
                        // Relative to current path
                        let mut base = parsed.path_and_query.clone();
                        if let Some(last_slash) = base.rfind('/') {
                            base.truncate(last_slash + 1);
                        } else {
                            base = "/".to_string();
                        }
                        current_url = format!("http://{}:{}{}{}", parsed.host, parsed.port, base, loc);
                    }
                    continue;
                }
            }

            return Ok(HttpResponse {
                final_url: current_url,
                ..resp
            });
        }
    }

    fn execute_single_get(&self, url: &SimpleUrl) -> Result<HttpResponse, String> {
        let addr = format!("{}:{}", url.host, url.port);
        let socket_addrs: Vec<_> = addr
            .to_socket_addrs()
            .map_err(|e| format!("Failed to resolve host '{}': {}", url.host, e))?
            .collect();

        if socket_addrs.is_empty() {
            return Err(format!("Could not resolve host: {}", url.host));
        }

        let mut stream = TcpStream::connect_timeout(&socket_addrs[0], self.timeout)
            .map_err(|e| format!("Failed to connect to {}: {}", addr, e))?;

        let _ = stream.set_read_timeout(Some(self.timeout));
        let _ = stream.set_write_timeout(Some(self.timeout));

        let request = format!(
            "GET {} HTTP/1.1\r\n\
             Host: {}:{}\r\n\
             User-Agent: zodiac/0.1.0\r\n\
             Accept: text/html,application/xhtml+xml,application/xml;q=0.9,application/json,*/*;q=0.8\r\n\
             Connection: close\r\n\
             \r\n",
            url.path_and_query, url.host, url.port
        );

        stream
            .write_all(request.as_bytes())
            .map_err(|e| format!("Failed to send HTTP request: {}", e))?;

        let mut raw_response = Vec::new();
        let mut buffer = [0u8; 8192];
        loop {
            match stream.read(&mut buffer) {
                Ok(0) => break,
                Ok(n) => raw_response.extend_from_slice(&buffer[..n]),
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock || e.kind() == std::io::ErrorKind::TimedOut => {
                    break;
                }
                Err(e) => return Err(format!("Failed reading HTTP response: {}", e)),
            }
        }

        Self::parse_raw_http(&raw_response, &url.to_string_url())
    }

    pub fn parse_raw_http(raw: &[u8], url: &str) -> Result<HttpResponse, String> {
        if raw.is_empty() {
            return Err("Empty response from server".to_string());
        }

        let split_pos = raw
            .windows(4)
            .position(|w| w == b"\r\n\r\n")
            .map(|pos| (pos, 4))
            .or_else(|| {
                raw.windows(2)
                    .position(|w| w == b"\n\n")
                    .map(|pos| (pos, 2))
            });

        let (header_bytes, body_bytes) = match split_pos {
            Some((pos, sep_len)) => (&raw[..pos], &raw[pos + sep_len..]),
            None => (raw, &[][..]),
        };

        let header_str = String::from_utf8_lossy(header_bytes);
        let mut lines = header_str.lines();

        let status_line = lines.next().ok_or_else(|| "Missing status line in response".to_string())?;
        let mut parts = status_line.splitn(3, ' ');
        let _http_version = parts.next().unwrap_or("HTTP/1.1");
        let status_code_str = parts.next().ok_or_else(|| "Missing status code".to_string())?;
        let status_code = status_code_str
            .parse::<u16>()
            .map_err(|e| format!("Invalid status code '{}': {}", status_code_str, e))?;
        let status_text = parts.next().unwrap_or("").trim().to_string();

        let mut headers = Vec::new();
        for line in lines {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if let Some(colon_pos) = line.find(':') {
                let name = line[..colon_pos].trim().to_string();
                let value = line[colon_pos + 1..].trim().to_string();
                headers.push((name, value));
            }
        }

        let is_chunked = headers.iter().any(|(k, v)| {
            k.eq_ignore_ascii_case("transfer-encoding") && v.to_ascii_lowercase().contains("chunked")
        });

        let body = if is_chunked {
            Self::decode_chunked(body_bytes)
        } else {
            body_bytes.to_vec()
        };

        Ok(HttpResponse {
            status: status_code,
            status_text,
            headers,
            body,
            final_url: url.to_string(),
        })
    }

    fn decode_chunked(body: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut idx = 0;
        while idx < body.len() {
            let line_end = match body[idx..].windows(2).position(|w| w == b"\r\n") {
                Some(pos) => idx + pos,
                None => break,
            };
            let line_str = match std::str::from_utf8(&body[idx..line_end]) {
                Ok(s) => s.trim(),
                Err(_) => break,
            };
            let size_hex = line_str.split(';').next().unwrap_or("").trim();
            let chunk_size = match usize::from_str_radix(size_hex, 16) {
                Ok(sz) => sz,
                Err(_) => break,
            };
            if chunk_size == 0 {
                break;
            }
            let data_start = line_end + 2;
            let data_end = data_start + chunk_size;
            if data_start > body.len() {
                break;
            }
            if data_end > body.len() {
                out.extend_from_slice(&body[data_start..]);
                break;
            }
            out.extend_from_slice(&body[data_start..data_end]);
            idx = data_end;
            if idx + 2 <= body.len() && &body[idx..idx + 2] == b"\r\n" {
                idx += 2;
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_raw_http() {
        let raw = b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 5\r\n\r\nHello";
        let resp = HttpClient::parse_raw_http(raw, "http://test").unwrap();
        assert_eq!(resp.status, 200);
        assert_eq!(resp.get_header("content-type"), Some("text/plain"));
        assert_eq!(resp.text(), "Hello");
    }

    #[test]
    fn test_chunked_decode() {
        let raw_chunked = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nHello\r\n6\r\n World\r\n0\r\n\r\n";
        let resp = HttpClient::parse_raw_http(raw_chunked, "http://test").unwrap();
        assert_eq!(resp.status, 200);
        assert_eq!(resp.text(), "Hello World");
    }
}
