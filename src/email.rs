use lettre::message::{header::ContentType, MultiPart, SinglePart};
use lettre::transport::smtp::AsyncSmtpTransport;
use lettre::{AsyncTransport, Message, Tokio1Executor};

use crate::config::Config;

/// `SMTP_HOST` yapılandırılmışsa o sunucu üzerinden (kimlik doğrulamasız düz
/// SMTP — ör. yerel bir Postfix aktarıcısı) gönderim yapar; VDS'te mikoshi-ai
/// ile aynı desen. Bu proje açık kaynak olduğu için varsayılan bir SMTP sunucusu
/// GÖMÜLMEZ — `SMTP_HOST` boşsa gönderim tamamen atlanır, çağıran taraf
/// sıfırlama bağlantısını sunucu logunda görünür bırakır.
pub async fn send_password_reset_email(config: &Config, to_email: &str, reset_url: &str) -> bool {
    let Some(smtp_host) = config.smtp_host.as_deref() else {
        tracing::debug!("SMTP_HOST tanımlı değil, şifre sıfırlama e-postası gönderilmeyecek.");
        return false;
    };

    let subject = "Commit Günlüğü — Şifre Sıfırlama Bağlantısı";

    let text_body = format!(
        "Şifre sıfırlama isteği\n\n\
        Commit Günlüğü hesabınız için bir şifre sıfırlama talebi aldık.\n\
        Yeni bir şifre belirlemek için şu bağlantıyı tarayıcınıza yapıştırın:\n\n\
        {reset_url}\n\n\
        Bu bağlantı 1 saat boyunca geçerlidir ve yalnızca bir kez kullanılabilir.\n\
        Bu talebi siz yapmadıysanız bu e-postayı yok sayabilirsiniz; hesabınızda bir değişiklik yapılmadı."
    );

    let html_body = format!(
        r##"<!doctype html>
<html lang="tr">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="color-scheme" content="dark light">
<title>{subject}</title>
</head>
<body style="margin:0; padding:0; background-color:#14110f; font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,Helvetica,Arial,sans-serif;">
<table role="presentation" width="100%" cellpadding="0" cellspacing="0" border="0" bgcolor="#14110f" style="background-color:#14110f;">
  <tr>
    <td align="center" style="padding:32px 16px;">
      <table role="presentation" width="100%" cellpadding="0" cellspacing="0" border="0" style="max-width:480px;">
        <tr>
          <td bgcolor="#1c1815" style="background-color:#1c1815; border:1px solid #332b24; border-radius:16px; padding:36px 32px;">
            <table role="presentation" width="100%" cellpadding="0" cellspacing="0" border="0">
              <tr>
                <td style="font-size:19px; font-weight:700; color:#f2ede3; letter-spacing:-.01em; padding-bottom:14px;">
                  Şifre sıfırlama isteği
                </td>
              </tr>
              <tr>
                <td style="font-size:14px; line-height:1.65; color:#a89e8f; padding-bottom:24px;">
                  Merhaba, Commit Günlüğü hesabınız için bir şifre sıfırlama talebi aldık. Aşağıdaki butona tıklayarak yeni bir şifre belirleyebilirsiniz.
                </td>
              </tr>
              <tr>
                <td align="center" style="padding-bottom:24px;">
                  <table role="presentation" cellpadding="0" cellspacing="0" border="0">
                    <tr>
                      <td bgcolor="#5b8a7a" style="background-color:#5b8a7a; border-radius:10px;">
                        <a href="{reset_url}" style="display:inline-block; padding:13px 28px; font-size:14px; font-weight:700; color:#0f1210; text-decoration:none; border-radius:10px;">Şifremi sıfırla</a>
                      </td>
                    </tr>
                  </table>
                </td>
              </tr>
              <tr>
                <td style="font-size:12.5px; line-height:1.6; color:#776b5c; padding-bottom:8px;">
                  Buton çalışmıyorsa şu bağlantıyı tarayıcınıza yapıştırın:
                </td>
              </tr>
              <tr>
                <td style="font-size:12.5px; word-break:break-all; padding-bottom:22px;">
                  <a href="{reset_url}" style="color:#5b8a7a; text-decoration:underline;">{reset_url}</a>
                </td>
              </tr>
              <tr>
                <td style="border-top:1px solid #332b24; padding-top:18px; font-size:12px; line-height:1.6; color:#776b5c;">
                  Bu bağlantı 1 saat boyunca geçerlidir ve yalnızca bir kez kullanılabilir. Bu talebi siz yapmadıysanız bu e-postayı yok sayabilirsiniz — hesabınızda bir değişiklik yapılmadı.
                </td>
              </tr>
            </table>
          </td>
        </tr>
        <tr>
          <td align="center" style="padding-top:22px; font-size:11.5px; color:#776b5c;">
            Bu e-postayı Commit Günlüğü hesabınız için bir şifre sıfırlama talebi olduğundan aldınız.
          </td>
        </tr>
      </table>
    </td>
  </tr>
</table>
</body>
</html>"##
    );

    let email = match Message::builder()
        .from(config.smtp_from.parse().unwrap_or_else(|_| {
            "no-reply@localhost".parse().expect("statik adres geçerli olmalı")
        }))
        .to(match to_email.parse() {
            Ok(addr) => addr,
            Err(e) => {
                tracing::error!("Geçersiz alıcı e-posta adresi ({}): {}", to_email, e);
                return false;
            }
        })
        .subject(subject)
        .multipart(
            MultiPart::alternative()
                .singlepart(
                    SinglePart::builder()
                        .header(ContentType::TEXT_PLAIN)
                        .body(text_body),
                )
                .singlepart(
                    SinglePart::builder()
                        .header(ContentType::TEXT_HTML)
                        .body(html_body),
                ),
        ) {
        Ok(m) => m,
        Err(e) => {
            tracing::error!("E-posta mesajı oluşturulamadı: {}", e);
            return false;
        }
    };

    let transport = AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(smtp_host)
        .port(config.smtp_port)
        .build();

    match transport.send(email).await {
        Ok(_) => {
            tracing::info!("Şifre sıfırlama e-postası gönderildi: {}", to_email);
            true
        }
        Err(e) => {
            // Yerel MTA yapılandırılmamışsa (ör. geliştirme ortamı) sessizce
            // düşme yerine uyar; bağlantı linki zaten çağıran tarafta loglanır.
            tracing::warn!("SMTP gönderimi başarısız ({}), reset linki loglanacak.", e);
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_send_email_disabled_when_no_host() {
        let mut config = Config::from_env();
        config.smtp_host = None;

        let res = send_password_reset_email(&config, "test@example.com", "https://commit.dixtuel.tr/reset-password?token=test1234").await;
        assert!(!res, "SMTP_HOST boşken e-posta gönderilmemeli");
    }
}


