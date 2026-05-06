# Client Auth Flow Prompt

Нужно полностью заменить текущий auth flow на новый двухшаговый сценарий.

## Что нужно сделать

### Регистрация

1. Экран регистрации начинается только с поля `email`.
2. После отправки email клиент вызывает:
   - `POST /_matrix/client/v3/register/email/requestToken`
3. Сервер отправляет код на почту и возвращает `sid`.
4. Клиент показывает экран ввода кода.
5. После успешного ввода кода клиент вызывает:
   - `POST /_matrix/client/v3/register/email/submitToken`
6. Только после подтверждения email клиент показывает форму:
   - `username`
   - `password`
7. После отправки username/password клиент вызывает:
   - `POST /_matrix/client/v3/register`
8. Если регистрация успешна, сервер возвращает сессию / access token, и пользователь входит в аккаунт.

### Вход

1. Экран входа принимает:
   - `login` или `email`
   - `password`
2. После отправки логина и пароля клиент вызывает:
   - `POST /_matrix/client/v3/login`
3. Если пароль верный, сервер отправляет код на email этого аккаунта и возвращает `sid`.
4. Клиент показывает экран ввода кода.
5. После ввода кода клиент вызывает:
   - `POST /_matrix/client/v3/login`
6. Если код верный, сервер возвращает:
   - `user_id`
   - `access_token`
   - `device_id`
   - `home_server`
   - `refresh_token` если есть
7. Только после этого пользователь считается вошедшим.

Первый запрос:

```json
{
  "client_secret": "random-client-secret",
  "login": "alice_or_email",
  "password": "secret-password",
  "send_attempt": 1
}
```

Ответ первого запроса:

```json
{
  "sid": "session-id",
  "email": "alice@example.com"
}
```

Второй запрос:

```json
{
  "client_secret": "random-client-secret",
  "sid": "session-id",
  "token": "123456",
  "device_id": "optional-device-id",
  "initial_device_display_name": "optional device name"
}
```

Ответ второго запроса:

```json
{
  "user_id": "@alice:server",
  "access_token": "token",
  "device_id": "DEVICEID",
  "home_server": "server",
  "refresh_token": null
}
```

## Контракт, который клиент должен хранить

### Общие поля

- `client_secret` нужно сохранять между шагами одного flow.
- `sid` нужно хранить до завершения соответствующего flow.
- `send_attempt` нужно увеличивать при повторной отправке кода.

### Registration requestToken

Request:

```json
{
  "client_secret": "random-client-secret",
  "email": "alice@example.com",
  "send_attempt": 1
}
```

Response:

```json
{
  "sid": "session-id"
}
```

### Registration submitToken

Request:

```json
{
  "client_secret": "random-client-secret",
  "sid": "session-id",
  "token": "123456"
}
```

Response:

```json
{
  "sid": "session-id"
}
```

### Final registration

Request:

```json
{
  "email": "alice@example.com",
  "client_secret": "random-client-secret",
  "sid": "session-id",
  "password": "secret-password",
  "username": "alice",
  "device_id": "optional-device-id",
  "initial_device_display_name": "optional device name",
  "inhibit_login": false
}
```

Response:

```json
{
  "user_id": "@alice:server",
  "access_token": "token",
  "device_id": "DEVICEID",
  "home_server": "server",
  "refresh_token": null,
  "expires_in": null
}
```

## Ошибки

Клиент должен различать эти случаи:

- неверный пароль:
  - показать ошибку invalid credentials
  - не переходить к шагу ввода кода
- неверный код:
  - показать ошибку invalid verification code
  - остаться на экране ввода кода
- повторная отправка кода слишком рано:
  - показать rate limit / retry later
  - использовать `retry_after_ms`, если сервер его вернул
- отсутствует email у аккаунта на login:
  - показать, что вход через email verification невозможен

## UX требования

- Не показывать старый UIAA login flow.
- Не пытаться завершать login сразу после пароля.
- Не пытаться завершать registration сразу после email-кода.
- Хранить промежуточное состояние между экранами.
- Если пользователь вернулся назад, не терять `client_secret` и `sid`, пока flow не завершён.

## Важное

- Это не дополнение к старому flow, а полная замена.
- После успешного password check в login надо обязательно ждать email code.
- После успешного email code в registration надо обязательно показать форму username/password.
- Только после финального шага сервер должен выдавать session/access token.
