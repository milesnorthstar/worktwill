//! 用户明确请求后获取网页标题；会向固定 Cloudflare DoH 查询主机名。
//! 不执行页面脚本、不读取 Cookie、不改系统 DNS，也不提供任意内网代理。
use reqwest::{header, redirect::Policy, Client, Url};
use scraper::{Html, Selector};
use std::{
    collections::HashSet,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    time::Duration,
};
use tokio::time::{timeout_at, Instant};
use worktwill_core::model::{AppError, Result, WebMetadata};

const TOTAL_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_BODY: usize = 256 * 1024;
const MAX_REDIRECTS: usize = 3;
const MAX_TITLE_CHARS: usize = 200;

fn unavailable(message: &str, message_en: &str) -> AppError {
    AppError::new("web_metadata_unavailable", message, message_en)
}

fn validate_url(input: &str) -> Result<Url> {
    if input.len() > 8192 {
        return Err(unavailable(
            "网址过长，请手动填写标题",
            "The URL is too long to fetch. Enter the title manually.",
        ));
    }
    let mut url = Url::parse(input.trim()).map_err(|_| {
        unavailable(
            "请输入有效的 http/https 网址",
            "Enter a valid HTTP/HTTPS URL.",
        )
    })?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(unavailable(
            "仅支持不含登录凭据的 http/https 网址",
            "Only HTTP/HTTPS URLs without login credentials are supported.",
        ));
    }
    url.set_fragment(None);
    Ok(url)
}

fn public_ipv4(ip: Ipv4Addr) -> bool {
    let [a, b, c, _] = ip.octets();
    // 保守拒绝私网、共享/基准/文档/保留网络；不把非公网地址交给 HTTP 连接。
    !(a == 0
        || a == 10
        || a == 127
        || a >= 224
        || (a == 100 && (64..=127).contains(&b))
        || (a == 169 && b == 254)
        || (a == 172 && (16..=31).contains(&b))
        || (a == 192 && (b == 0 || b == 168 || (b == 88 && c == 99)))
        || (a == 198 && (b == 18 || b == 19 || (b == 51 && c == 100)))
        || (a == 203 && b == 0 && c == 113))
}

fn public_ipv6(ip: Ipv6Addr) -> bool {
    let s = ip.segments();
    // 只接受全局单播；IPv4 mapped/compatible、6to4、IETF 特殊和文档前缀均拒绝。
    ip.to_ipv4().is_none()
        && (s[0] & 0xe000) == 0x2000
        && !(s[0] == 0x2001 && s[1] < 0x0200)
        && !(s[0] == 0x2001 && s[1] == 0x0db8)
        && s[0] != 0x2002
        && !(s[0] == 0x3fff && (s[1] & 0xf000) == 0)
}

fn public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => public_ipv4(ip),
        IpAddr::V6(ip) => public_ipv6(ip),
    }
}

fn parse_dns_addresses(body: &[u8], port: u16) -> Result<Vec<SocketAddr>> {
    let response: serde_json::Value = serde_json::from_slice(body).map_err(|_| {
        unavailable(
            "网页域名解析失败，可手动填写标题",
            "The website DNS lookup failed. Enter the title manually.",
        )
    })?;
    if response["Status"].as_u64() != Some(0) || response["TC"].as_bool() != Some(false) {
        return Err(unavailable(
            "网页域名解析失败，可手动填写标题",
            "The website DNS lookup failed. Enter the title manually.",
        ));
    }
    let Some(answers) = response.get("Answer") else {
        return Ok(vec![]);
    };
    let answers = answers.as_array().ok_or_else(|| {
        unavailable(
            "网页域名解析结果无效",
            "The website DNS response is invalid.",
        )
    })?;
    if answers.len() > 64 {
        return Err(unavailable(
            "网页域名解析结果过多",
            "The website DNS response contains too many addresses.",
        ));
    }
    let mut result = Vec::new();
    for answer in answers {
        let record_type = answer["type"].as_u64();
        if !matches!(record_type, Some(1 | 28)) {
            continue;
        }
        let ip: IpAddr = answer["data"]
            .as_str()
            .and_then(|data| data.parse().ok())
            .ok_or_else(|| {
                unavailable(
                    "网页域名解析结果无效",
                    "The website DNS response is invalid.",
                )
            })?;
        if !public_ip(ip)
            || (record_type == Some(1) && !ip.is_ipv4())
            || (record_type == Some(28) && !ip.is_ipv6())
        {
            return Err(unavailable("仅自动获取公网网页标题，其他网址可手动填写","Only public web page titles are fetched automatically. Enter other titles manually."));
        }
        result.push(SocketAddr::new(ip, port));
    }
    Ok(result)
}

async fn resolve_public_host(host: &str, port: u16, deadline: Instant) -> Result<Vec<SocketAddr>> {
    let normalized = host.trim_end_matches('.').to_ascii_lowercase();
    if !normalized.contains('.')
        || [
            "localhost",
            "local",
            "internal",
            "test",
            "invalid",
            "home.arpa",
        ]
        .iter()
        .any(|suffix| normalized == *suffix || normalized.ends_with(&format!(".{suffix}")))
    {
        return Err(unavailable(
            "仅自动获取公网网页标题，其他网址可手动填写",
            "Only public web page titles are fetched automatically. Enter other titles manually.",
        ));
    }
    let remaining = deadline
        .checked_duration_since(Instant::now())
        .ok_or_else(|| {
            unavailable(
                "获取标题超时，可手动填写",
                "Fetching the title timed out. Enter it manually.",
            )
        })?;
    // 此解析服务固定 TLS 主机名和公开地址，不让系统 Fake-IP 或二次 DNS 改变连接目标。
    let resolver = Client::builder()
        .no_proxy()
        .redirect(Policy::none())
        .timeout(remaining)
        .resolve("cloudflare-dns.com", SocketAddr::from(([1, 1, 1, 1], 443)))
        .build()
        .map_err(|_| {
            unavailable(
                "无法建立网页域名解析请求",
                "The website DNS request could not be created.",
            )
        })?;
    let mut addresses = HashSet::new();
    for record_type in ["A", "AAAA"] {
        let mut response = timeout_at(
            deadline,
            resolver
                .get("https://cloudflare-dns.com/dns-query")
                .query(&[("name", normalized.as_str()), ("type", record_type)])
                .header(header::ACCEPT, "application/dns-json")
                .send(),
        )
        .await
        .map_err(|_| {
            unavailable(
                "获取标题超时，可手动填写",
                "Fetching the title timed out. Enter it manually.",
            )
        })?
        .map_err(|_| {
            unavailable(
                "无法解析网页域名，可手动填写标题",
                "The website domain could not be resolved. Enter the title manually.",
            )
        })?;
        if !response.status().is_success() {
            return Err(unavailable(
                "网页域名解析服务不可用，可手动填写标题",
                "The website DNS service is unavailable. Enter the title manually.",
            ));
        }
        let mime = response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .split(';')
            .next()
            .unwrap_or("")
            .trim();
        if !matches!(mime, "application/dns-json" | "application/json") {
            return Err(unavailable(
                "网页域名解析结果格式无效",
                "The website DNS response format is invalid.",
            ));
        }
        let mut body = Vec::new();
        loop {
            let chunk = timeout_at(deadline, response.chunk())
                .await
                .map_err(|_| {
                    unavailable(
                        "获取标题超时，可手动填写",
                        "Fetching the title timed out. Enter it manually.",
                    )
                })?
                .map_err(|_| {
                    unavailable(
                        "读取域名解析结果失败",
                        "The website DNS response could not be read.",
                    )
                })?;
            let Some(chunk) = chunk else {
                break;
            };
            if body.len().saturating_add(chunk.len()) > 16 * 1024 {
                return Err(unavailable(
                    "网页域名解析结果过大",
                    "The website DNS response is too large.",
                ));
            }
            body.extend_from_slice(&chunk);
        }
        addresses.extend(parse_dns_addresses(&body, port)?);
    }
    let mut addresses: Vec<_> = addresses.into_iter().collect();
    // 没有 IPv6 出口的机器先尝试 IPv4；全部地址仍已单独校验。
    addresses.sort_by_key(|address| address.is_ipv6());
    Ok(addresses)
}

async fn pinned_client(url: &Url, deadline: Instant) -> Result<Client> {
    let host = url
        .host_str()
        .ok_or_else(|| unavailable("网址缺少主机名称", "The URL has no hostname."))?;
    let bare_host = host.trim_start_matches('[').trim_end_matches(']');
    let port = url
        .port_or_known_default()
        .ok_or_else(|| unavailable("网址端口无效", "The URL port is invalid."))?;
    let addresses: Vec<SocketAddr> = if let Ok(ip) = bare_host.parse::<IpAddr>() {
        vec![SocketAddr::new(ip, port)]
    } else {
        // 每跳重新通过固定 DoH 解析，之后固定连接地址；不会退回系统 DNS 或代理。
        resolve_public_host(bare_host, port, deadline).await?
    };
    if addresses.is_empty()
        || addresses.len() > 32
        || addresses.iter().any(|address| !public_ip(address.ip()))
    {
        return Err(unavailable(
            "仅自动获取公网网页标题，其他网址可手动填写",
            "Only public web page titles are fetched automatically. Enter other titles manually.",
        ));
    }
    let remaining = deadline
        .checked_duration_since(Instant::now())
        .ok_or_else(|| {
            unavailable(
                "获取标题超时，可手动填写",
                "Fetching the title timed out. Enter it manually.",
            )
        })?;
    Client::builder()
        .no_proxy()
        .redirect(Policy::none())
        .timeout(remaining)
        .connect_timeout(remaining)
        .resolve_to_addrs(bare_host, &addresses)
        .user_agent("WorkTwill/0.1 (+local-bookmark-title)")
        .build()
        .map_err(|_| {
            unavailable(
                "无法建立网页请求，可手动填写标题",
                "The web request could not be created. Enter the title manually.",
            )
        })
}

fn extract_title(body: &[u8], content_type: &str) -> Result<Option<String>> {
    // 不猜测未知字符集，否则自动标题可能变成乱码；失败保持用户可手填。
    let lowered = content_type.to_ascii_lowercase();
    if let Some(charset) = lowered
        .split(';')
        .map(str::trim)
        .find_map(|part| part.strip_prefix("charset="))
    {
        let charset = charset.trim_matches(['\'', '"']);
        if !matches!(charset, "utf-8" | "utf8" | "us-ascii") {
            return Err(unavailable(
                "该网页编码暂不支持，可手动填写标题",
                "This page encoding is not supported. Enter the title manually.",
            ));
        }
    }
    let text = std::str::from_utf8(body).map_err(|_| {
        unavailable(
            "该网页编码暂不支持，可手动填写标题",
            "This page encoding is not supported. Enter the title manually.",
        )
    })?;
    let html = Html::parse_document(text);
    let selector = Selector::parse("title").expect("static title selector");
    let Some(title) = html.select(&selector).next() else {
        return Ok(None);
    };
    let decoded = title.text().collect::<String>();
    // title 是最终的纯文本值；过滤控制字符并压缩空白，不把 HTML 回传为可执行内容。
    let title = decoded
        .chars()
        .filter(|c| !c.is_control() || c.is_whitespace())
        .collect::<String>();
    let title = title.split_whitespace().collect::<Vec<_>>().join(" ");
    let title: String = title.chars().take(MAX_TITLE_CHARS).collect();
    Ok((!title.is_empty()).then_some(title))
}

pub async fn fetch_metadata(url: String) -> Result<WebMetadata> {
    let deadline = Instant::now() + TOTAL_TIMEOUT;
    let mut url = validate_url(&url)?;
    for redirects in 0..=MAX_REDIRECTS {
        let client = pinned_client(&url, deadline).await?;
        let mut response = timeout_at(
            deadline,
            client
                .get(url.clone())
                .header(header::ACCEPT, "text/html, application/xhtml+xml")
                .send(),
        )
        .await
        .map_err(|_| {
            unavailable(
                "获取标题超时，可手动填写",
                "Fetching the title timed out. Enter it manually.",
            )
        })?
        .map_err(|_| {
            unavailable(
                "无法获取网页，可手动填写标题",
                "The web page could not be fetched. Enter the title manually.",
            )
        })?;
        if matches!(response.status().as_u16(), 301 | 302 | 303 | 307 | 308) {
            if redirects == MAX_REDIRECTS {
                return Err(unavailable(
                    "网页跳转过多，可手动填写标题",
                    "The website redirected too many times. Enter the title manually.",
                ));
            }
            let location = response
                .headers()
                .get(header::LOCATION)
                .and_then(|v| v.to_str().ok())
                .ok_or_else(|| {
                    unavailable(
                        "网页跳转地址无效，可手动填写标题",
                        "The website redirect URL is invalid. Enter the title manually.",
                    )
                })?;
            let next = url.join(location).map_err(|_| {
                unavailable(
                    "网页跳转地址无效，可手动填写标题",
                    "The website redirect URL is invalid. Enter the title manually.",
                )
            })?;
            url = validate_url(next.as_str())?;
            continue;
        }
        if !response.status().is_success() {
            return Err(unavailable(
                "网页没有返回可用内容，可手动填写标题",
                "The website returned no usable content. Enter the title manually.",
            ));
        }
        let content_type = response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_owned();
        let mime = content_type
            .split(';')
            .next()
            .unwrap_or("")
            .trim()
            .to_ascii_lowercase();
        if !matches!(mime.as_str(), "text/html" | "application/xhtml+xml") {
            return Err(unavailable(
                "该网址不是 HTML 网页，可手动填写标题",
                "This URL is not an HTML page. Enter the title manually.",
            ));
        }
        if response
            .content_length()
            .is_some_and(|size| size > MAX_BODY as u64)
        {
            return Err(unavailable(
                "网页内容过大，可手动填写标题",
                "The web page is too large. Enter the title manually.",
            ));
        }
        let mut body = Vec::new();
        loop {
            let chunk = timeout_at(deadline, response.chunk())
                .await
                .map_err(|_| {
                    unavailable(
                        "获取标题超时，可手动填写",
                        "Fetching the title timed out. Enter it manually.",
                    )
                })?
                .map_err(|_| {
                    unavailable(
                        "读取网页失败，可手动填写标题",
                        "The web page could not be read. Enter the title manually.",
                    )
                })?;
            let Some(chunk) = chunk else {
                break;
            };
            if body.len().saturating_add(chunk.len()) > MAX_BODY {
                return Err(unavailable(
                    "网页内容过大，可手动填写标题",
                    "The web page is too large. Enter the title manually.",
                ));
            }
            body.extend_from_slice(&chunk);
        }
        return Ok(WebMetadata {
            title: extract_title(&body, &content_type)?,
            final_url: url.into(),
        });
    }
    Err(unavailable(
        "无法获取标题，可手动填写",
        "The title could not be fetched. Enter it manually.",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_private_special_and_mapped_addresses() {
        for value in [
            "0.0.0.0",
            "10.1.2.3",
            "127.0.0.1",
            "100.64.0.1",
            "169.254.1.1",
            "172.16.1.1",
            "192.168.1.1",
            "192.0.2.1",
            "198.18.0.1",
            "198.51.100.1",
            "203.0.113.1",
            "224.0.0.1",
            "255.255.255.255",
            "::",
            "::1",
            "fc00::1",
            "fe80::1",
            "ff02::1",
            "::ffff:8.8.8.8",
            "::8.8.8.8",
            "2001:db8::1",
            "2002:0808:0808::1",
            "3fff::1",
        ] {
            assert!(!public_ip(value.parse().unwrap()), "must reject {value}");
        }
        for value in [
            "8.8.8.8",
            "1.1.1.1",
            "2606:4700:4700::1111",
            "2001:4860:4860::8888",
        ] {
            assert!(public_ip(value.parse().unwrap()), "must allow {value}");
        }
    }

    #[test]
    fn dns_answers_cannot_smuggle_private_or_malformed_addresses() {
        assert_eq!(parse_dns_addresses(br#"{"Status":0,"TC":false,"Answer":[{"type":5,"data":"cdn.example.com"},{"type":1,"data":"1.1.1.1"}]}"#,443).unwrap(),vec!["1.1.1.1:443".parse::<SocketAddr>().unwrap()]);
        for body in [
            br#"{"Status":0,"TC":false,"Answer":[{"type":1,"data":"1.1.1.1"},{"type":1,"data":"127.0.0.1"}]}"#.as_slice(),
            br#"{"Status":0,"TC":false,"Answer":[{"type":28,"data":"::ffff:1.1.1.1"}]}"#.as_slice(),
            br#"{"Status":0,"TC":false,"Answer":[{"type":28,"data":"1.1.1.1"}]}"#.as_slice(),
            br#"{"Status":3,"TC":false}"#.as_slice(),
            br#"{"Status":0,"TC":true}"#.as_slice(),
            br#"{"Status":0,"TC":false,"Answer":{}}"#.as_slice(),
        ] {assert!(parse_dns_addresses(body,443).is_err());}
    }

    #[test]
    fn accepts_only_credential_free_http_urls_and_removes_fragment() {
        for value in [
            "file:///etc/passwd",
            "javascript:alert(1)",
            "https://user:secret@example.com/",
            "http://user@example.com/",
        ] {
            let error = validate_url(value).unwrap_err();
            assert_eq!(error.code, "web_metadata_unavailable");
            assert!(!error.message.is_empty());
            assert!(!error.message_en.is_empty());
            let payload = serde_json::to_value(error).unwrap();
            assert!(payload["message_en"].as_str().unwrap().is_ascii());
        }
        assert_eq!(
            validate_url("https://example.com/a#private")
                .unwrap()
                .as_str(),
            "https://example.com/a"
        );
    }

    #[test]
    fn title_is_decoded_bounded_and_plain_text() {
        assert_eq!(
            extract_title(
                b"<title> A &amp; B\n  Example </title>",
                "text/html; charset=UTF-8"
            )
            .unwrap(),
            Some("A & B Example".into())
        );
        assert_eq!(
            extract_title(b"<body>No title</body>", "text/html").unwrap(),
            None
        );
        assert_eq!(
            extract_title(
                format!("<title>{}</title>", "a".repeat(300)).as_bytes(),
                "text/html"
            )
            .unwrap()
            .unwrap()
            .chars()
            .count(),
            200
        );
        assert!(extract_title(b"<title>x</title>", "text/html; charset=gbk").is_err());
        assert!(extract_title(&[0xff, 0xfe], "text/html").is_err());
    }
}
