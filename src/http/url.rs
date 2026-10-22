#[derive(Debug, Clone)]
pub struct SimpleUrl {
    pub host: String,
    pub port: u16,
    pub path_and_query: String,
}

impl SimpleUrl {
    pub fn parse(raw: &str) -> Result<Self, String> {
        let trimmed = raw.trim();
        let without_proto = if let Some(rest) = trimmed.strip_prefix("http://") {
            rest
        } else if let Some(rest) = trimmed.strip_prefix("https://") {
            // kiwix-serve is standard HTTP. If someone specifies HTTPS without TLS, warn.
            rest
        } else {
            trimmed
        };

        let (host_port, path) = match without_proto.find('/') {
            Some(idx) => (&without_proto[..idx], &without_proto[idx..]),
            None => (without_proto, "/"),
        };

        let (host, port) = if let Some(colon_idx) = host_port.rfind(':') {
            let host_part = &host_port[..colon_idx];
            let port_part = &host_port[colon_idx + 1..];
            let port = port_part.parse::<u16>().map_err(|e| format!("Invalid port in URL: {}", e))?;
            (host_part.to_string(), port)
        } else {
            (host_port.to_string(), 80)
        };

        if host.is_empty() {
            return Err("URL host is empty".to_string());
        }

        let path_and_query = if path.is_empty() {
            "/".to_string()
        } else {
            path.to_string()
        };

        Ok(Self {
            host,
            port,
            path_and_query,
        })
    }

    pub fn to_string_url(&self) -> String {
        if self.port == 80 {
            format!("http://{}{}", self.host, self.path_and_query)
        } else {
            format!("http://{}:{}{}", self.host, self.port, self.path_and_query)
        }
    }
}

pub fn url_encode(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for b in input.bytes() {
        match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            b' ' => out.push_str("%20"),
            _ => {
                out.push_str(&format!("%{:02X}", b));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_url_parse() {
        let u1 = SimpleUrl::parse("http://127.0.0.1:8080/catalog/v2/entries").unwrap();
        assert_eq!(u1.host, "127.0.0.1");
        assert_eq!(u1.port, 8080);
        assert_eq!(u1.path_and_query, "/catalog/v2/entries");

        let u2 = SimpleUrl::parse("localhost:8000/").unwrap();
        assert_eq!(u2.host, "localhost");
        assert_eq!(u2.port, 8000);
        assert_eq!(u2.path_and_query, "/");

        let u3 = SimpleUrl::parse("http://example.com").unwrap();
        assert_eq!(u3.host, "example.com");
        assert_eq!(u3.port, 80);
        assert_eq!(u3.path_and_query, "/");
    }

    #[test]
    fn test_url_encode() {
        assert_eq!(url_encode("hello world"), "hello%20world");
        assert_eq!(url_encode("a+b=c&d"), "a%2Bb%3Dc%26d");
    }
}
