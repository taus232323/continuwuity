use askama::Template;
use ruma::UserId;

pub trait MessageTemplate: Template {
	fn subject(&self) -> String;

	fn html_body(&self) -> Option<String> { None }
}

#[derive(Template)]
#[template(path = "mail/change_email.txt")]
pub struct ChangeEmail<'a> {
	pub user_id: Option<&'a UserId>,
	pub verification_link: String,
}

impl MessageTemplate for ChangeEmail<'_> {
	fn subject(&self) -> String { "Подтверждение адреса электронной почты".to_owned() }
}

#[derive(Template)]
#[template(path = "mail/new_account.txt")]
pub struct NewAccount {
	pub verification_code: String,
}

impl MessageTemplate for NewAccount {
	fn subject(&self) -> String { "Подтверждение регистрации".to_owned() }

	fn html_body(&self) -> Option<String> {
		NewAccountHtml {
			verification_code: self.verification_code.clone(),
		}
		.render()
		.ok()
	}
}

#[derive(Template)]
#[template(path = "mail/new_account.html")]
pub struct NewAccountHtml {
	pub verification_code: String,
}

#[derive(Template)]
#[template(path = "mail/new_account_code.txt")]
pub struct NewAccountCode {
	pub verification_code: String,
}

impl MessageTemplate for NewAccountCode {
	fn subject(&self) -> String { "Подтверждение регистрации".to_owned() }

	fn html_body(&self) -> Option<String> {
		NewAccountCodeHtml {
			verification_code: self.verification_code.clone(),
		}
		.render()
		.ok()
	}
}

#[derive(Template)]
#[template(path = "mail/new_account_code.html")]
pub struct NewAccountCodeHtml {
	pub verification_code: String,
}

#[derive(Template)]
#[template(path = "mail/login_code.txt")]
pub struct LoginCode<'a> {
	pub user_id: &'a UserId,
	pub verification_code: String,
}

impl MessageTemplate for LoginCode<'_> {
	fn subject(&self) -> String { "Код входа".to_owned() }

	fn html_body(&self) -> Option<String> {
		LoginCodeHtml {
			verification_code: self.verification_code.clone(),
		}
		.render()
		.ok()
	}
}

#[derive(Template)]
#[template(path = "mail/login_code.html")]
pub struct LoginCodeHtml {
	pub verification_code: String,
}

#[derive(Template)]
#[template(path = "mail/password_reset.txt")]
pub struct PasswordReset<'a> {
	pub display_name: Option<&'a str>,
	pub user_id: &'a UserId,
	pub verification_code: String,
}

impl MessageTemplate for PasswordReset<'_> {
	fn subject(&self) -> String { format!("Запрос на сброс пароля для {}", &self.user_id) }

	fn html_body(&self) -> Option<String> {
		PasswordResetHtml {
			display_name: self.display_name,
			user_id: self.user_id,
			verification_code: self.verification_code.clone(),
		}
		.render()
		.ok()
	}
}

#[derive(Template)]
#[template(path = "mail/password_reset.html")]
pub struct PasswordResetHtml<'a> {
	pub display_name: Option<&'a str>,
	pub user_id: &'a UserId,
	pub verification_code: String,
}

#[derive(Template)]
#[template(path = "mail/test.txt")]
pub struct Test;

impl MessageTemplate for Test {
	fn subject(&self) -> String { "Test message".to_owned() }
}
