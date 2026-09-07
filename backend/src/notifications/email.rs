use std::time::Duration;

use async_trait::async_trait;
use lettre::{
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
    message::{Mailbox, MultiPart},
};

use crate::config::{SmtpConfig, SmtpSecurity};

const PAYMENT_REMINDER_TEMPLATE: &str = include_str!("payment_reminder.html");

pub struct PaymentReminderEmail {
    pub user_id: String,
    pub display_name: String,
    pub email: String,
    pub amount_cents: i64,
    pub payment_link: String,
}

#[derive(Debug, PartialEq, Eq)]
pub struct EmailDeliveryResult {
    pub sent_count: usize,
    pub failed_user_ids: Vec<String>,
}

#[async_trait]
trait EmailSender: Sync {
    async fn send(&self, message: Message) -> Result<(), String>;
}

struct SmtpEmailSender {
    transport: AsyncSmtpTransport<Tokio1Executor>,
}

impl SmtpEmailSender {
    fn new(config: &SmtpConfig) -> anyhow::Result<Self> {
        let builder = match config.security {
            SmtpSecurity::None => {
                AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&config.host)
            }
            SmtpSecurity::Starttls => {
                AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&config.host)?
            }
        };
        let transport = builder
            .port(config.port)
            .timeout(Some(Duration::from_secs(20)))
            .build();
        Ok(Self { transport })
    }
}

#[async_trait]
impl EmailSender for SmtpEmailSender {
    async fn send(&self, message: Message) -> Result<(), String> {
        self.transport
            .send(message)
            .await
            .map(|_| ())
            .map_err(|error| error.to_string())
    }
}

pub fn email_address_available(email: Option<&str>) -> bool {
    email
        .map(str::trim)
        .filter(|email| !email.is_empty())
        .and_then(|email| email.parse::<lettre::Address>().ok())
        .is_some()
}

pub async fn send_payment_reminder_emails(
    config: &SmtpConfig,
    app_name: &str,
    frontend_base_url: &str,
    reminders: &[PaymentReminderEmail],
) -> anyhow::Result<EmailDeliveryResult> {
    let sender = SmtpEmailSender::new(config)?;
    deliver_payment_reminder_emails(
        &sender,
        &config.from,
        app_name,
        frontend_base_url,
        reminders,
    )
    .await
}

async fn deliver_payment_reminder_emails(
    sender: &dyn EmailSender,
    from: &str,
    app_name: &str,
    frontend_base_url: &str,
    reminders: &[PaymentReminderEmail],
) -> anyhow::Result<EmailDeliveryResult> {
    let from = from.parse::<Mailbox>()?;
    let mut sent_count = 0;
    let mut failed_user_ids = Vec::new();

    for reminder in reminders {
        let message =
            build_payment_reminder_message(from.clone(), app_name, frontend_base_url, reminder);
        let result = match message {
            Ok(message) => sender.send(message).await,
            Err(error) => Err(error.to_string()),
        };
        match result {
            Ok(()) => {
                sent_count += 1;
                tracing::info!(user_id = %reminder.user_id, "payment reminder email accepted by SMTP server");
            }
            Err(error) => {
                failed_user_ids.push(reminder.user_id.clone());
                tracing::warn!(user_id = %reminder.user_id, %error, "payment reminder email delivery failed");
            }
        }
    }

    Ok(EmailDeliveryResult {
        sent_count,
        failed_user_ids,
    })
}

fn build_payment_reminder_message(
    from: Mailbox,
    app_name: &str,
    frontend_base_url: &str,
    reminder: &PaymentReminderEmail,
) -> anyhow::Result<Message> {
    let address = reminder.email.trim().parse::<lettre::Address>()?;
    let recipient = Mailbox::new(Some(reminder.display_name.clone()), address);
    let amount = format_euros(reminder.amount_cents);
    let html = render_html(
        app_name,
        frontend_base_url,
        &reminder.display_name,
        &amount,
        &reminder.payment_link,
    );
    let plain = format!(
        "Hi {},\n\nYour {} account currently has an outstanding balance of {}.\n\nPay now: {}\n\nALREADY PAID?\nPlease mark your debt as paid in the app: {}\n\n{}",
        reminder.display_name, app_name, amount, reminder.payment_link, frontend_base_url, app_name,
    );

    Ok(Message::builder()
        .from(from)
        .to(recipient)
        .subject(format!("{app_name} payment reminder: {amount} due"))
        .multipart(MultiPart::alternative_plain_html(plain, html))?)
}

fn render_html(
    app_name: &str,
    frontend_base_url: &str,
    display_name: &str,
    amount: &str,
    payment_link: &str,
) -> String {
    PAYMENT_REMINDER_TEMPLATE
        .replace("{{app_name}}", &escape_html(app_name))
        .replace("{{frontend_base_url}}", &escape_html(frontend_base_url))
        .replace("{{display_name}}", &escape_html(display_name))
        .replace("{{amount}}", &escape_html(amount))
        .replace("{{payment_link}}", &escape_html(payment_link))
}

fn escape_html(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            _ => escaped.push(character),
        }
    }
    escaped
}

fn format_euros(amount_cents: i64) -> String {
    format!("{}.{:02} EUR", amount_cents / 100, amount_cents.abs() % 100)
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    struct SequencedSender {
        results: Mutex<Vec<Result<(), String>>>,
    }

    #[async_trait]
    impl EmailSender for SequencedSender {
        async fn send(&self, _message: Message) -> Result<(), String> {
            self.results.lock().unwrap().remove(0)
        }
    }

    fn reminder(user_id: &str, display_name: &str) -> PaymentReminderEmail {
        PaymentReminderEmail {
            user_id: user_id.into(),
            display_name: display_name.into(),
            email: format!("{user_id}@example.org"),
            amount_cents: 1220,
            payment_link: "https://paypal.example/pay/12.20?x=1&y=2".into(),
        }
    }

    #[test]
    fn email_addresses_must_be_non_blank_and_parseable() {
        assert!(email_address_available(Some("ada@example.org")));
        assert!(!email_address_available(None));
        assert!(!email_address_available(Some("  ")));
        assert!(!email_address_available(Some("not an email")));
    }

    #[test]
    fn reminder_message_contains_plain_and_escaped_html_parts() {
        let message = build_payment_reminder_message(
            "Coffee Tracker <coffee@example.org>".parse().unwrap(),
            "Coffee Tracker",
            "https://coffee.example.org",
            &reminder("ada", "Ada <Admin>"),
        )
        .unwrap();
        let formatted = String::from_utf8(message.formatted()).unwrap();
        let html = render_html(
            "Coffee Tracker",
            "https://coffee.example.org",
            "Ada <Admin>",
            "12.20 EUR",
            "https://paypal.example/pay/12.20?x=1&y=2",
        );

        assert!(formatted.contains("Coffee Tracker payment reminder: 12.20 EUR due"));
        assert!(formatted.contains("12.20 EUR"));
        assert!(formatted.contains("multipart/alternative"));
        assert!(html.contains("Ada &lt;Admin&gt;"));
        assert!(html.contains("https://paypal.example/pay/12.20?x=1&amp;y=2"));
        assert!(html.contains("Already paid?"));
        assert!(html.contains("Please mark your debt as"));
    }

    #[tokio::test]
    async fn delivery_continues_after_a_recipient_failure() {
        let sender = SequencedSender {
            results: Mutex::new(vec![Ok(()), Err("rejected".into()), Ok(())]),
        };
        let reminders = vec![
            reminder("ada", "Ada"),
            reminder("grace", "Grace"),
            reminder("linus", "Linus"),
        ];

        let result = deliver_payment_reminder_emails(
            &sender,
            "Coffee Tracker <coffee@example.org>",
            "Coffee Tracker",
            "https://coffee.example.org",
            &reminders,
        )
        .await
        .unwrap();

        assert_eq!(result.sent_count, 2);
        assert_eq!(result.failed_user_ids, vec!["grace"]);
    }
}
