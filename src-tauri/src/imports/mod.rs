use crate::models::{ConnectionProfile, ProviderKind};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct ImportCandidate {
    pub profile: ConnectionProfile,
    pub secret: Option<String>,
    pub source: String,
}

pub fn discover_connections() -> Vec<ImportCandidate> {
    let mut out = Vec::new();
    for path in filezilla_paths() {
        if let Ok(content) = fs::read_to_string(&path) {
            out.extend(parse_filezilla(&content, &format!("FileZilla · {}", path.display())));
        }
    }
    for path in remmina_paths() {
        if let Ok(content) = fs::read_to_string(&path) {
            if let Some(candidate) = parse_remmina(&content, &format!("Remmina · {}", path.display())) {
                out.push(candidate);
            }
        }
    }
    if let Some(path) = openssh_path() {
        if let Ok(content) = fs::read_to_string(&path) {
            out.extend(parse_openssh(&content, &format!("OpenSSH · {}", path.display())));
        }
    }
    out
}

fn base_profile(
    name: String,
    kind: ProviderKind,
    host: String,
    port: u16,
    username: Option<String>,
) -> ConnectionProfile {
    ConnectionProfile {
        id: Uuid::new_v4().to_string(),
        name,
        kind,
        host: Some(host),
        port: Some(port),
        username,
        initial_path: Some("/".into()),
        timeout_seconds: 30,
        keep_alive_seconds: 30,
        max_connections: 4,
        group_name: Some("Importados".into()),
        tags: vec!["importado".into()],
        favorite: false,
        extra: Map::new(),
    }
}

fn filezilla_paths() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(config) = dirs::config_dir() {
        roots.push(config.join("filezilla"));
        roots.push(config.join("FileZilla"));
    }
    if let Some(home) = dirs::home_dir() {
        roots.push(home.join(".config").join("filezilla"));
        roots.push(home.join(".filezilla"));
    }
    let mut files = Vec::new();
    for root in roots {
        for name in ["sitemanager.xml", "recentservers.xml"] {
            let path = root.join(name);
            if path.is_file() && !files.contains(&path) {
                files.push(path);
            }
        }
    }
    files
}

fn remmina_paths() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(home) = dirs::home_dir() {
        roots.push(home.join(".local").join("share").join("remmina"));
        roots.push(home.join(".remmina"));
    }
    if let Some(data) = dirs::data_dir() {
        roots.push(data.join("remmina"));
    }
    let mut files = Vec::new();
    for root in roots {
        if let Ok(entries) = fs::read_dir(root) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|value| value.to_str()) == Some("remmina")
                    && !files.contains(&path)
                {
                    files.push(path);
                }
            }
        }
    }
    files
}

fn openssh_path() -> Option<PathBuf> {
    dirs::home_dir()
        .map(|home| home.join(".ssh").join("config"))
        .filter(|path| path.is_file())
}

fn xml_field(block: &str, tag: &str) -> Option<(String, String)> {
    let start_needle = format!("<{tag}");
    let start = block.find(&start_needle)?;
    let open_end_relative = block[start..].find('>')?;
    let open_end = start + open_end_relative;
    let attrs = block[start + start_needle.len()..open_end]
        .trim()
        .to_string();
    let close_needle = format!("</{tag}>");
    let close_relative = block[open_end + 1..].find(&close_needle)?;
    let close = open_end + 1 + close_relative;
    let value = decode_xml_entities(block[open_end + 1..close].trim());
    Some((value, attrs))
}

fn xml_text(block: &str, tag: &str) -> Option<String> {
    xml_field(block, tag).map(|(value, _)| value)
}

fn decode_xml_entities(value: &str) -> String {
    value
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
}

fn xml_blocks<'a>(input: &'a str, tag: &str) -> Vec<&'a str> {
    let start_needle = format!("<{tag}>");
    let close_needle = format!("</{tag}>");
    let mut remaining = input;
    let mut blocks = Vec::new();
    while let Some(start) = remaining.find(&start_needle) {
        let after_start = &remaining[start + start_needle.len()..];
        let Some(end) = after_start.find(&close_needle) else {
            break;
        };
        blocks.push(&after_start[..end]);
        remaining = &after_start[end + close_needle.len()..];
    }
    blocks
}

fn filezilla_protocol(value: Option<String>) -> Option<ProviderKind> {
    match value.as_deref().unwrap_or("0") {
        "0" => Some(ProviderKind::Ftp),
        "1" => Some(ProviderKind::Sftp),
        "3" => Some(ProviderKind::Ftps),
        "4" => Some(ProviderKind::Ftpes),
        _ => None,
    }
}

fn parse_filezilla(content: &str, source: &str) -> Vec<ImportCandidate> {
    let mut out = Vec::new();
    for block in xml_blocks(content, "Server") {
        let Some(host) = xml_text(block, "Host").filter(|value| !value.is_empty()) else {
            continue;
        };
        let Some(kind) = filezilla_protocol(xml_text(block, "Protocol")) else {
            continue;
        };
        let default_port = match kind {
            ProviderKind::Sftp => 22,
            ProviderKind::Ftps => 990,
            _ => 21,
        };
        let port = xml_text(block, "Port")
            .and_then(|value| value.parse::<u16>().ok())
            .unwrap_or(default_port);
        let username = xml_text(block, "User").filter(|value| !value.is_empty());
        let name = xml_text(block, "Name")
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| host.clone());
        let mut profile = base_profile(name, kind, host, port, username);
        profile.group_name = Some("FileZilla".into());
        if let Some(remote) = xml_text(block, "RemoteDir").filter(|value| value.starts_with('/')) {
            profile.initial_path = Some(remote);
        }
        let key_path = xml_text(block, "Keyfile").filter(|value| !value.is_empty());
        if let Some(key) = key_path.as_ref() {
            profile
                .extra
                .insert("keyPath".into(), Value::String(key.clone()));
        }
        let secret = xml_field(block, "Pass").and_then(|(value, attrs)| {
            if value.is_empty() {
                return None;
            }
            if attrs.contains("encoding=\"base64\"") || attrs.contains("encoding='base64'") {
                STANDARD
                    .decode(value.as_bytes())
                    .ok()
                    .and_then(|bytes| String::from_utf8(bytes).ok())
            } else {
                Some(value)
            }
        });
        let auth = if key_path.is_some() && secret.is_none() {
            "key"
        } else if secret.is_some() && key_path.is_none() {
            "password"
        } else {
            "auto"
        };
        profile
            .extra
            .insert("authMethod".into(), Value::String(auth.into()));
        out.push(ImportCandidate {
            profile,
            secret,
            source: source.into(),
        });
    }
    out
}

fn parse_ini(content: &str) -> HashMap<String, String> {
    let mut values = HashMap::new();
    for raw in content.lines() {
        let line = raw.trim();
        if line.is_empty()
            || line.starts_with('#')
            || line.starts_with(';')
            || line.starts_with('[')
        {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            values.insert(key.trim().to_ascii_lowercase(), value.trim().to_string());
        }
    }
    values
}

fn split_server(value: &str, default_port: u16) -> (String, u16) {
    let value = value.trim();
    if let Some(rest) = value.strip_prefix('[') {
        if let Some(end) = rest.find(']') {
            let host = rest[..end].to_string();
            let port = rest[end + 1..]
                .strip_prefix(':')
                .and_then(|value| value.parse::<u16>().ok())
                .unwrap_or(default_port);
            return (host, port);
        }
    }
    if let Some((host, port)) = value.rsplit_once(':') {
        if !host.contains(':') {
            if let Ok(port) = port.parse::<u16>() {
                return (host.to_string(), port);
            }
        }
    }
    (value.to_string(), default_port)
}

fn parse_remmina(content: &str, source: &str) -> Option<ImportCandidate> {
    let values = parse_ini(content);
    let protocol = values.get("protocol")?.to_ascii_uppercase();
    let kind = match protocol.as_str() {
        "SFTP" | "SSH" => ProviderKind::Sftp,
        "FTP" => ProviderKind::Ftp,
        "FTPS" => ProviderKind::Ftps,
        _ => return None,
    };
    let default_port = match kind {
        ProviderKind::Sftp => 22,
        ProviderKind::Ftps => 990,
        _ => 21,
    };
    let server = values.get("server").or_else(|| values.get("ssh_server"))?;
    let (host, port) = split_server(server, default_port);
    if host.is_empty() {
        return None;
    }
    let username = values
        .get("username")
        .or_else(|| values.get("ssh_username"))
        .cloned()
        .filter(|value| !value.is_empty());
    let name = values
        .get("name")
        .cloned()
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| host.clone());
    let mut profile = base_profile(name, kind, host, port, username);
    profile.group_name = values
        .get("group")
        .cloned()
        .filter(|value| !value.is_empty())
        .or_else(|| Some("Remmina".into()));
    let key_path = values
        .get("ssh_privatekey")
        .or_else(|| values.get("privatekey"))
        .cloned()
        .filter(|value| !value.is_empty());
    if let Some(key) = key_path.as_ref() {
        profile
            .extra
            .insert("keyPath".into(), Value::String(key.clone()));
    }
    let secret = values
        .get("password")
        .cloned()
        .filter(|value| !value.is_empty() && value != "." && value != "null");
    let auth = if key_path.is_some() && secret.is_none() {
        "key"
    } else if secret.is_some() && key_path.is_none() {
        "password"
    } else {
        "auto"
    };
    profile
        .extra
        .insert("authMethod".into(), Value::String(auth.into()));
    Some(ImportCandidate {
        profile,
        secret,
        source: source.into(),
    })
}

fn parse_openssh(content: &str, source: &str) -> Vec<ImportCandidate> {
    #[derive(Default)]
    struct Block {
        aliases: Vec<String>,
        hostname: Option<String>,
        user: Option<String>,
        port: Option<u16>,
        identity: Option<String>,
    }

    fn flush(block: &mut Block, source: &str, out: &mut Vec<ImportCandidate>) {
        if block.aliases.is_empty() {
            return;
        }
        for alias in block.aliases.drain(..) {
            if alias.contains('*') || alias.contains('?') || alias.starts_with('!') {
                continue;
            }
            let host = block.hostname.clone().unwrap_or_else(|| alias.clone());
            if host.is_empty() {
                continue;
            }
            let mut profile = base_profile(
                alias,
                ProviderKind::Sftp,
                host,
                block.port.unwrap_or(22),
                block.user.clone(),
            );
            profile.group_name = Some("OpenSSH".into());
            if let Some(identity) = block.identity.clone() {
                profile
                    .extra
                    .insert("keyPath".into(), Value::String(identity));
                profile
                    .extra
                    .insert("authMethod".into(), Value::String("key".into()));
            } else {
                profile
                    .extra
                    .insert("authMethod".into(), Value::String("auto".into()));
            }
            out.push(ImportCandidate {
                profile,
                secret: None,
                source: source.into(),
            });
        }
        block.hostname = None;
        block.user = None;
        block.port = None;
        block.identity = None;
    }

    let mut block = Block::default();
    let mut out = Vec::new();
    for raw in content.lines() {
        let line = raw.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let mut parts = line.split_whitespace();
        let Some(key) = parts.next() else {
            continue;
        };
        let value = parts.collect::<Vec<_>>().join(" ");
        match key.to_ascii_lowercase().as_str() {
            "host" => {
                flush(&mut block, source, &mut out);
                block.aliases = value.split_whitespace().map(ToOwned::to_owned).collect();
            }
            "hostname" => block.hostname = Some(value),
            "user" => block.user = Some(value),
            "port" => block.port = value.parse::<u16>().ok(),
            "identityfile" => block.identity = Some(value),
            _ => {}
        }
    }
    flush(&mut block, source, &mut out);
    out
}

pub fn same_endpoint(left: &ConnectionProfile, right: &ConnectionProfile) -> bool {
    left.kind == right.kind
        && left
            .host
            .as_deref()
            .unwrap_or("")
            .eq_ignore_ascii_case(right.host.as_deref().unwrap_or(""))
        && left.port == right.port
        && left
            .username
            .as_deref()
            .unwrap_or("")
            .eq_ignore_ascii_case(right.username.as_deref().unwrap_or(""))
}

#[cfg(test)]
mod tests {
    use super::{parse_filezilla, parse_openssh, parse_remmina};
    use crate::models::ProviderKind;

    #[test]
    fn imports_filezilla_sftp_password() {
        let xml = r#"<FileZilla3><Servers><Server><Host>10.0.0.2</Host><Port>22</Port><Protocol>1</Protocol><User>root</User><Pass encoding="base64">c2VjcmV0</Pass><Name>NAS</Name></Server></Servers></FileZilla3>"#;
        let rows = parse_filezilla(xml, "FileZilla");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].profile.kind, ProviderKind::Sftp);
        assert_eq!(rows[0].secret.as_deref(), Some("secret"));
    }

    #[test]
    fn imports_remmina_sftp() {
        let ini = "[remmina]\nname=Servidor\nprotocol=SFTP\nserver=192.168.1.10:2222\nusername=danilo\nssh_privatekey=~/.ssh/id_ed25519\n";
        let row = parse_remmina(ini, "Remmina").unwrap();
        assert_eq!(row.profile.port, Some(2222));
        assert_eq!(
            row.profile.extra.get("keyPath").and_then(|value| value.as_str()),
            Some("~/.ssh/id_ed25519")
        );
    }

    #[test]
    fn imports_openssh_alias() {
        let config = "Host aerocool\n  HostName 192.168.30.10\n  User root\n  IdentityFile ~/.ssh/id_ed25519\n";
        let rows = parse_openssh(config, "OpenSSH");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].profile.name, "aerocool");
        assert_eq!(rows[0].profile.host.as_deref(), Some("192.168.30.10"));
    }
}
