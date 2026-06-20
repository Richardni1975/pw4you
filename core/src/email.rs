//! Email-based password recovery module.
//!
//! Uses the `lettre` crate to send verification codes via SMTP.
//! The user's bound email and SMTP settings are stored in the app config.
//!
//! ## Auto-detection of TLS mode
//! - Port **465** → SSL/TLS (direct TLS, used by 网易/126/163/QQ)
//! - Port **587** → STARTTLS (used by Gmail/Outlook)
//! - Other ports default to STARTTLS
//!
//! # SMTP Configuration Guide
//!
//! ## Gmail
//! 1. Enable 2-Factor Authentication at: https://myaccount.google.com/security
//! 2. Generate an App Password at: https://myaccount.google.com/apppasswords
//!    (Select "Mail" as the app, "Other" as the device, then copy the 16-char password)
//! 3. Use: Server: smtp.gmail.com, Port: 587
//!
//! ## QQ Mail (QQ邮箱)
//! 1. Settings → Account → POP3/IMAP/SMTP Service → Enable SMTP
//! 2. Get the authorization code. Use: Server: smtp.qq.com, Port: 587
//!
//! ## 126 Mail (网易126邮箱) ⭐
//! 1. Log in at https://mail.126.com
//! 2. Settings → POP3/SMTP/IMAP → Enable "SMTP Service"
//! 3. NetEase will generate an **authorization code** (授权码)
//! 4. Use: Server: smtp.126.com, Port: 465
//! 5. Password = the authorization code (NOT your login password!)
//!
//! ## 163 Mail (网易163邮箱)
//! 1. Settings → POP3/SMTP/IMAP → Enable SMTP service
//! 2. Server: smtp.163.com, Port: 465
//! 3. Password = the authorization code
//!
//! ## Outlook / Hotmail
//! 1. Server: smtp-mail.outlook.com, Port: 587

use rand::Rng;

use lettre::Transport;

use crate::config::SmtpConfig;
use crate::error::{Pw4Error, Pw4Result};

/// Generate a random 6-digit numeric verification code.
pub fn generate_verification_code() -> String {
    let mut rng = rand::thread_rng();
    format!("{:06}", rng.gen_range(0..1_000_000))
}

/// Verification code expiry time in seconds (10 minutes).
pub const CODE_EXPIRY_SECONDS: i64 = 600;

/// Send a verification code to the user's email via SMTP.
///
/// # Arguments
/// * `smtp_config` — SMTP server configuration from app config.
/// * `to_email` — The recipient email address (the bound security email).
/// * `code` — The 6-digit verification code to send.
///
/// # Returns
/// Ok(()) on success, or an error describing what went wrong.
pub fn send_verification_email(
    smtp_config: &SmtpConfig,
    to_email: &str,
    code: &str,
) -> Pw4Result<()> {
    // Build the email message
    let email = lettre::Message::builder()
        .from(
            format!("pw4you <{}>", smtp_config.username)
                .parse()
                .map_err(|e| Pw4Error::SmtpError(format!("Invalid from address: {}", e)))?,
        )
        .to(to_email
            .parse()
            .map_err(|e| Pw4Error::SmtpError(format!("Invalid to address: {}", e)))?)
        .subject("pw4you — 密码恢复验证码 / Password Recovery Code")
        .body(format!(
            "您好，\n\n\
             您正在为 pw4you 加密文件夹请求密码恢复。\n\n\
             您的验证码是：{}\n\n\
             ⚠️ 该验证码将在 10 分钟内过期。\n\
             ⚠️ 如果您没有请求此操作，请忽略此邮件。\n\n\
             ---\n\n\
             Hello,\n\n\
             You are requesting password recovery for your pw4you encrypted folder.\n\n\
             Your verification code is: {}\n\n\
             ⚠️ This code will expire in 10 minutes.\n\
             ⚠️ If you did not request this, please ignore this email.\n\n\
             — pw4you",
            code, code
        ))
        .map_err(|e| Pw4Error::SmtpError(format!("Failed to build email: {}", e)))?;

    // Build SMTP transport — auto-detect SSL vs STARTTLS based on port
    let creds = lettre::transport::smtp::authentication::Credentials::new(
        smtp_config.username.clone(),
        smtp_config.password.clone(),
    );

    let mailer = if smtp_config.port == 465 {
        // SSL/TLS direct (used by 126.com, 163.com, QQ)
        lettre::SmtpTransport::relay(&smtp_config.server)
            .map_err(|e| Pw4Error::SmtpError(format!(
                "Failed to connect to SMTP server '{}': {}. \
                 Check that the server address is correct.",
                smtp_config.server, e
            )))?
            .credentials(creds)
            .port(smtp_config.port)
            .build()
    } else {
        // STARTTLS (used by Gmail, Outlook on port 587)
        lettre::SmtpTransport::starttls_relay(&smtp_config.server)
            .map_err(|e| Pw4Error::SmtpError(format!(
                "Failed to connect to SMTP server '{}': {}. \
                 Check that the server address is correct.",
                smtp_config.server, e
            )))?
            .credentials(creds)
            .port(smtp_config.port)
            .build()
    };

    // Send the email
    match mailer.send(&email) {
        Ok(_) => {
            log::info!(
                "Verification code sent to {} (masked: {}***@{})",
                to_email,
                &to_email[..1],
                to_email.split('@').nth(1).unwrap_or("unknown")
            );
            Ok(())
        }
        Err(e) => Err(Pw4Error::SmtpError(format!(
            "Failed to send email: {}. \
             For Gmail: make sure you're using an App Password (not your account password). \
             See the SMTP Configuration Guide in the Settings page.",
            e
        ))),
    }
}

/// Verify a user-submitted code against the stored pending code.
///
/// Returns Ok(()) if the code matches and hasn't expired.
pub fn verify_code(
    submitted_code: &str,
    pending_code: &str,
    expiry_timestamp: i64,
) -> Pw4Result<()> {
    let now = chrono::Utc::now().timestamp();

    if now > expiry_timestamp {
        return Err(Pw4Error::VerificationCodeExpired);
    }

    if submitted_code.trim() != pending_code {
        return Err(Pw4Error::InvalidVerificationCode);
    }

    Ok(())
}

/// Get a masked version of an email for display.
/// e.g., "example@gmail.com" → "e***@gmail.com"
pub fn mask_email(email: &str) -> String {
    if let Some(at_pos) = email.find('@') {
        let first_char = &email[..1];
        let domain = &email[at_pos..];
        format!("{}***{}", first_char, domain)
    } else {
        "***".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_verification_code() {
        let code = generate_verification_code();
        assert_eq!(code.len(), 6);
        assert!(code.chars().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn test_verify_code_correct() {
        let now = chrono::Utc::now().timestamp();
        let result = verify_code("123456", "123456", now + 300);
        assert!(result.is_ok());
    }

    #[test]
    fn test_verify_code_wrong() {
        let now = chrono::Utc::now().timestamp();
        let result = verify_code("654321", "123456", now + 300);
        assert!(matches!(
            result.unwrap_err(),
            Pw4Error::InvalidVerificationCode
        ));
    }

    #[test]
    fn test_verify_code_expired() {
        let now = chrono::Utc::now().timestamp();
        let result = verify_code("123456", "123456", now - 1);
        assert!(matches!(
            result.unwrap_err(),
            Pw4Error::VerificationCodeExpired
        ));
    }

    #[test]
    fn test_mask_email() {
        let masked = mask_email("example@gmail.com");
        assert_eq!(masked, "e***@gmail.com");
    }
}
