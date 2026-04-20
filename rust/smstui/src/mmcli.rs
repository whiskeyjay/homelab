use anyhow::{Context, Result, bail};
use std::process::Command;

#[derive(Debug, Clone)]
pub struct SmsMessage {
    pub index: u32,
    pub number: String,
    pub text: String,
    pub timestamp: String,
    pub state: SmsState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SmsState {
    Received,
    Sent,
    Sending,
    Unknown(String),
}

impl SmsState {
    fn parse(s: &str) -> Self {
        match s.trim().to_lowercase().as_str() {
            "received" => Self::Received,
            "sent" => Self::Sent,
            "sending" => Self::Sending,
            other => Self::Unknown(other.to_string()),
        }
    }

    pub fn is_outgoing(&self) -> bool {
        matches!(self, Self::Sent | Self::Sending)
    }
}

pub fn list_modems() -> Result<Vec<u32>> {
    let output = Command::new("mmcli")
        .arg("-L")
        .output()
        .context("Failed to run mmcli -L")?;

    if !output.status.success() {
        bail!(
            "mmcli -L failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let modems = stdout
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.contains("/org/freedesktop/ModemManager1/Modem/") {
                line.rsplit('/').next()?.split_whitespace().next()?.parse().ok()
            } else {
                None
            }
        })
        .collect();

    Ok(modems)
}

#[derive(Debug, Clone)]
pub struct ModemInfo {
    pub index: u32,
    pub manufacturer: String,
    pub model: String,
    pub own_number: String,
    pub state: String,
}

pub fn get_modem_info(modem_index: u32) -> Result<ModemInfo> {
    let output = Command::new("mmcli")
        .args(["-m", &modem_index.to_string()])
        .output()
        .context("Failed to get modem info")?;

    if !output.status.success() {
        bail!(
            "mmcli -m {} failed: {}",
            modem_index,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut manufacturer = String::new();
    let mut model = String::new();
    let mut own_number = String::new();
    let mut state = String::new();

    for line in stdout.lines() {
        let value_part = if let Some((_before, after)) = line.split_once('|') {
            after
        } else {
            line
        };
        let value_part = value_part.trim();

        if let Some(val) = value_part.strip_prefix("manufacturer:") {
            manufacturer = val.trim().to_string();
        } else if let Some(val) = value_part.strip_prefix("model:") {
            model = val.trim().to_string();
        } else if let Some(val) = value_part.strip_prefix("own:") {
            own_number = val.trim().to_string();
        } else if let Some(val) = value_part.strip_prefix("state:") {
            if state.is_empty() {
                state = val.trim().to_string();
            }
        }
    }

    Ok(ModemInfo {
        index: modem_index,
        manufacturer,
        model,
        own_number,
        state,
    })
}

pub fn list_sms(modem_index: u32) -> Result<Vec<(u32, String)>> {
    let output = Command::new("mmcli")
        .args(["-m", &modem_index.to_string(), "--messaging-list-sms"])
        .output()
        .context("Failed to list SMS")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("No sms messages were found") {
            return Ok(vec![]);
        }
        bail!("mmcli list SMS failed: {}", stderr);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let sms_entries = stdout
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.starts_with("/org/freedesktop/ModemManager1/SMS/") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                let index: u32 = parts[0].rsplit('/').next()?.parse().ok()?;
                let state = parts
                    .get(1)
                    .map(|s| s.trim_matches(|c| c == '(' || c == ')'))
                    .unwrap_or("unknown")
                    .to_string();
                Some((index, state))
            } else {
                None
            }
        })
        .collect();

    Ok(sms_entries)
}

pub fn get_sms(sms_index: u32) -> Result<SmsMessage> {
    let output = Command::new("mmcli")
        .args(["-s", &sms_index.to_string()])
        .output()
        .context("Failed to get SMS details")?;

    if !output.status.success() {
        bail!(
            "mmcli -s {} failed: {}",
            sms_index,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    let mut number = String::new();
    let mut text = String::new();
    let mut timestamp = String::new();
    let mut state = String::new();

    for line in stdout.lines() {
        let value_part = if let Some((_before, after)) = line.split_once('|') {
            after
        } else {
            line
        };

        let value_part = value_part.trim();

        if let Some(val) = value_part.strip_prefix("number:") {
            number = val.trim().trim_matches('\'').to_string();
        } else if let Some(val) = value_part.strip_prefix("text:") {
            text = val.trim().trim_matches('\'').to_string();
        } else if let Some(val) = value_part.strip_prefix("timestamp:") {
            timestamp = val.trim().trim_matches('\'').to_string();
        } else if let Some(val) = value_part.strip_prefix("state:") {
            state = val.trim().to_string();
        }
    }

    Ok(SmsMessage {
        index: sms_index,
        number,
        text,
        timestamp,
        state: SmsState::parse(&state),
    })
}

pub fn create_and_send_sms(modem_index: u32, number: &str, text: &str) -> Result<()> {
    let create_arg = format!("--messaging-create-sms=number='{}',text='{}'", number, text);
    let create_output = Command::new("mmcli")
        .args(["-m", &modem_index.to_string(), &create_arg])
        .output()
        .context("Failed to create SMS")?;

    if !create_output.status.success() {
        bail!(
            "Failed to create SMS: {}",
            String::from_utf8_lossy(&create_output.stderr)
        );
    }

    let stdout = String::from_utf8_lossy(&create_output.stdout);
    let sms_index = stdout
        .lines()
        .find_map(|line| {
            line.rsplit("/org/freedesktop/ModemManager1/SMS/")
                .next()
                .and_then(|idx| idx.trim().parse::<u32>().ok())
        })
        .context("Could not parse SMS index from create output")?;

    let send_output = Command::new("mmcli")
        .args(["-s", &sms_index.to_string(), "--send"])
        .output()
        .context("Failed to send SMS")?;

    if !send_output.status.success() {
        bail!(
            "Failed to send SMS: {}",
            String::from_utf8_lossy(&send_output.stderr)
        );
    }

    Ok(())
}

/// Check if we currently have sudo privileges (cached credentials or NOPASSWD).
pub fn has_sudo() -> bool {
    Command::new("sudo")
        .args(["-n", "true"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

pub fn delete_sms(modem_index: u32, sms_index: u32) -> Result<()> {
    if !has_sudo() {
        bail!("sudo privileges required to delete SMS. Run 'sudo -v' first or configure NOPASSWD for mmcli.");
    }

    let sms_path = format!("/org/freedesktop/ModemManager1/SMS/{}", sms_index);
    let output = Command::new("sudo")
        .args([
            "-n",
            "mmcli",
            "-m",
            &modem_index.to_string(),
            "--messaging-delete-sms",
            &sms_path,
        ])
        .output()
        .context("Failed to run sudo mmcli")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("Failed to delete SMS {}: {}", sms_index, stderr.trim());
    }

    Ok(())
}
