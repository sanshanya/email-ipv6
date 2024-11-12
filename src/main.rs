use lettre::message::{header::ContentType, Message};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{SmtpTransport, Transport};
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fs;
use std::net::{SocketAddr, UdpSocket};
use std::path::Path;

#[derive(Serialize, Deserialize)]
struct Config {
    smtp: SmtpConfig,
    #[serde(default)]
    ipv6: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct SmtpConfig {
    server: String,
    port: u16,
    login: String,
    password: String,
    from_addr: String,
    to_addr: String,
}

const DEFAULT_CONFIG: &str = r#"# SMTP服务器配置
[smtp]
server = "smtp.qq.com"    # SMTP服务器地址
port = 587               # SMTP端口
login = ""              # 邮箱账号
password = ""           # 邮箱授权码
from_addr = ""         # 发件人地址
to_addr = ""          # 收件人地址

# 上次检测到的IPv6地址（程序自动维护，请勿手动修改）
ipv6 = ""
"#;

fn load_or_create_config(path: &str) -> Result<Config, Box<dyn Error>> {
    if !Path::new(path).exists() {
        fs::write(path, DEFAULT_CONFIG)?;
        println!("已创建配置文件模板：{}", path);
        println!("请填写配置文件中的SMTP信息后重新运行程序");
        return Err("配置文件未填写完整".into());
    }

    let content = fs::read_to_string(path)?;
    let config: Config = toml::from_str(&content)?;

    // 验证SMTP配置是否完整
    if config.smtp.login.is_empty() || config.smtp.password.is_empty() {
        println!("请在配置文件中填写完整的SMTP信息");
        return Err("配置文件未填写完整".into());
    }

    Ok(config)
}

fn save_config(path: &str, config: &Config) -> Result<(), Box<dyn Error>> {
    let content = toml::to_string(config)?;
    fs::write(path, content)?;
    Ok(())
}

fn get_ipv6() -> Option<String> {
    let addresses = [
        "[2001:4860:4860::8888]:80", // Google DNS
        "[2001:4860:4860::8844]:80", // Google DNS备用
        "[2606:4700:4700::1111]:80", // Cloudflare DNS
        "[2400:3200::1]:80",         // 阿里云 DNS
    ];

    for addr_str in &addresses {
        if let Ok(socket) = UdpSocket::bind("[::]:0") {
            if let Ok(addr) = addr_str.parse::<SocketAddr>() {
                if socket.connect(addr).is_ok() {
                    if let Ok(local_addr) = socket.local_addr() {
                        if let SocketAddr::V6(ipv6) = local_addr {
                            return Some(ipv6.ip().to_string());
                        }
                    }
                } else {
                    eprintln!("连接到公共地址失败: {}", addr_str);
                }
            }
        } else {
            eprintln!("创建socket失败");
        }
    }

    None
}

fn send_email(smtp_config: &SmtpConfig, subject: &str, body: &str) -> Result<(), Box<dyn Error>> {
    let email = Message::builder()
        .from(format!("IPv6监控 <{}>", smtp_config.from_addr).parse()?)
        .to(smtp_config.to_addr.parse()?)
        .subject(subject)
        .header(ContentType::TEXT_PLAIN)
        .body(body.to_string())?;

    let creds = Credentials::new(smtp_config.login.clone(), smtp_config.password.clone());

    let mailer = SmtpTransport::starttls_relay(&smtp_config.server)?
        .port(smtp_config.port)
        .credentials(creds)
        .build();

    mailer.send(&email)?;
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let config_path = "config.toml";
    let result = (|| -> Result<(), Box<dyn Error>> {
        let mut config = load_or_create_config(config_path)?;

        let current_ipv6 = get_ipv6().ok_or("无法获取IPv6地址")?;
        let last_ipv6 = config.ipv6.as_deref();

        if last_ipv6 != Some(&current_ipv6) {
            println!("IPv6地址已更改: {}", current_ipv6);

            let subject = "IPv6地址更新通知";
            let body = format!(
                "IPv6地址已更新\n旧地址: {}\n新地址: {}",
                last_ipv6.unwrap_or("无"),
                current_ipv6
            );

            send_email(&config.smtp, subject, &body)?;
            config.ipv6 = Some(current_ipv6);
            save_config(config_path, &config)?;
            println!("地址已更新并发送通知");
        } else {
            println!("IPv6地址未发生变化");
        }

        Ok(())
    })();

    if let Err(e) = result {
        eprintln!("发生错误: {}", e);
    }

    println!("\n按任意键退出...");
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    Ok(())
}