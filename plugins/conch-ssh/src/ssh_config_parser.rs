//! Parse `~/.ssh/config` to extract Host entries as ServerEntry values.

use crate::config::ServerEntry;

/// Parse the user's `~/.ssh/config` file and return server entries.
///
/// Skips wildcard patterns (`Host *`) and `Match` blocks. Extracts
/// `Host`, `HostName`, `User`, `Port`, and `IdentityFile`.
pub fn parse_ssh_config() -> Vec<ServerEntry> {
    let Some(home) = dirs::home_dir() else {
        return Vec::new();
    };
    let path = home.join(".ssh").join("config");
    let contents = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    parse_ssh_config_str(&contents)
}

fn parse_ssh_config_str(contents: &str) -> Vec<ServerEntry> {
    let mut entries = Vec::new();
    let mut current: Option<PartialEntry> = None;

    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let (key, value) = match line.split_once(char::is_whitespace) {
            Some((k, v)) => (k.to_lowercase(), v.trim().to_string()),
            None => continue,
        };

        match key.as_str() {
            "host" => {
                // Flush previous entry.
                if let Some(entry) = current.take() {
                    if let Some(se) = entry.into_server_entry() {
                        entries.push(se);
                    }
                }
                // Skip wildcard patterns.
                if value.contains('*') || value.contains('?') {
                    continue;
                }
                current = Some(PartialEntry {
                    alias: value,
                    hostname: None,
                    user: None,
                    port: None,
                    identity_file: None,
                    proxy_command: None,
                    proxy_jump: None,
                });
            }
            "hostname" => {
                if let Some(ref mut e) = current {
                    e.hostname = Some(value);
                }
            }
            "user" => {
                if let Some(ref mut e) = current {
                    e.user = Some(value);
                }
            }
            "port" => {
                if let Some(ref mut e) = current {
                    e.port = value.parse().ok();
                }
            }
            "identityfile" => {
                if let Some(ref mut e) = current {
                    e.identity_file = Some(value);
                }
            }
            "proxycommand" => {
                if let Some(ref mut e) = current {
                    e.proxy_command = Some(value);
                }
            }
            "proxyjump" => {
                if let Some(ref mut e) = current {
                    e.proxy_jump = Some(value);
                }
            }
            "match" => {
                // Flush and stop tracking — Match blocks are complex.
                if let Some(entry) = current.take() {
                    if let Some(se) = entry.into_server_entry() {
                        entries.push(se);
                    }
                }
            }
            _ => {}
        }
    }

    // Flush last entry.
    if let Some(entry) = current.take() {
        if let Some(se) = entry.into_server_entry() {
            entries.push(se);
        }
    }

    entries
}

struct PartialEntry {
    alias: String,
    hostname: Option<String>,
    user: Option<String>,
    port: Option<u16>,
    identity_file: Option<String>,
    proxy_command: Option<String>,
    proxy_jump: Option<String>,
}

impl PartialEntry {
    fn into_server_entry(self) -> Option<ServerEntry> {
        let host = self.hostname.unwrap_or_else(|| self.alias.clone());
        let user = self.user.unwrap_or_else(|| {
            std::env::var("USER").unwrap_or_else(|_| "root".to_string())
        });
        let label = self.alias.clone();

        Some(ServerEntry {
            id: format!("sshconfig_{}", self.alias),
            label,
            host,
            port: self.port.unwrap_or(22),
            user,
            auth_method: "key".to_string(),
            key_path: self.identity_file,
            proxy_command: self.proxy_command,
            proxy_jump: self.proxy_jump,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_basic_hosts() {
        let config = "\
Host server1
    HostName 10.0.0.1
    User deploy
    Port 2222

Host server2
    HostName example.com
    IdentityFile ~/.ssh/id_ed25519
";
        let entries = parse_ssh_config_str(config);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].label, "server1");
        assert_eq!(entries[0].host, "10.0.0.1");
        assert_eq!(entries[0].user, "deploy");
        assert_eq!(entries[0].port, 2222);
        assert_eq!(entries[0].id, "sshconfig_server1");

        assert_eq!(entries[1].label, "server2");
        assert_eq!(entries[1].host, "example.com");
        assert_eq!(entries[1].port, 22);
        assert!(entries[1].key_path.is_some());
    }

    #[test]
    fn skip_wildcard_hosts() {
        let config = "\
Host *
    ServerAliveInterval 60

Host myserver
    HostName 10.0.0.5
";
        let entries = parse_ssh_config_str(config);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].label, "myserver");
    }

    #[test]
    fn host_without_hostname_uses_alias() {
        let config = "\
Host direct.example.com
    User admin
";
        let entries = parse_ssh_config_str(config);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].host, "direct.example.com");
    }

    #[test]
    fn empty_config() {
        let entries = parse_ssh_config_str("");
        assert!(entries.is_empty());
    }

    #[test]
    fn comments_and_blank_lines_ignored() {
        let config = "\
# This is a comment

Host myhost
    # another comment
    HostName 192.168.1.1

";
        let entries = parse_ssh_config_str(config);
        assert_eq!(entries.len(), 1);
    }

    #[test]
    fn proxy_jump_parsed() {
        let config = "\
Host bastion-target
    HostName 10.0.0.99
    ProxyJump bastion.example.com
";
        let entries = parse_ssh_config_str(config);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].proxy_jump.as_deref(), Some("bastion.example.com"));
        assert!(entries[0].proxy_command.is_none());
    }

    #[test]
    fn proxy_command_parsed() {
        let config = "\
Host tunneled
    HostName 192.168.1.1
    ProxyCommand ssh -W %h:%p gateway
";
        let entries = parse_ssh_config_str(config);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].proxy_command.as_deref(), Some("ssh -W %h:%p gateway"));
        assert!(entries[0].proxy_jump.is_none());
    }

    #[test]
    fn match_block_flushes_current() {
        let config = "\
Host server1
    HostName 10.0.0.1

Match host *.internal
    User internal
";
        let entries = parse_ssh_config_str(config);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].label, "server1");
    }
}
