# Matrix: SMTP, первый админ и настройка клиентов

Эта инструкция описывает, что нужно сделать перед подключением форкнутых
клиентов Element Web, Android и iOS к homeserver.

Текущие параметры:

- Matrix `server_name`: `celesteai.ru`
- Homeserver API: `https://matrix.celesteai.ru`
- Web-клиент: `https://chat.celesteai.ru`
- Client discovery: `https://celesteai.ru/.well-known/matrix/client`
- Server discovery: `https://celesteai.ru/.well-known/matrix/server`
- Конфиг сервера: `deploy/continuwuity.toml`
- Docker Compose: `deploy/docker-compose.yml`
- Команды деплоя из корня: `make matrix-*`

Название мессенджера отдельно от домена. Его нужно выставить в форках клиентов,
в `deploy/element-config.json` поле `brand`, а также в SMTP display name:
`sender = "CHANGE_ME <noreply@celesteai.ru>"`.

Важно: после создания базы не менять `server_name = "celesteai.ru"`. Matrix ID
пользователей привязаны к этому имени сервера.

## 1. Заменить SMTP placeholder

Открыть файл:

```bash
deploy/continuwuity.toml
```

Найти блок:

```toml
[global.smtp]
connection_uri = "smtps://mailer%40celesteai.ru:CHANGE_ME@mail.celesteai.ru:465"
sender = "CHANGE_ME <noreply@celesteai.ru>"

require_email_for_registration = true
require_email_for_token_registration = true
```

Заменить:

- `CHANGE_ME` в `connection_uri` на настоящий SMTP пароль или app password.
- `CHANGE_ME` в `sender` на видимое имя мессенджера.

Пример:

```toml
connection_uri = "smtps://mailer%40celesteai.ru:APP_PASSWORD@mail.celesteai.ru:465"
sender = "Messenger Name <noreply@celesteai.ru>"
```

На что обратить внимание:

- Если SMTP логин является email-адресом, символ `@` должен быть записан как
  `%40`.
- Если пароль содержит спецсимволы URL, их тоже нужно URL-encode: `@`, `:`,
  `/`, `?`, `#`, `%`, пробелы и похожие символы.
- `sender` должен быть разрешен SMTP-провайдером. Некоторые провайдеры не дадут
  отправлять письма от адреса, который не совпадает с авторизованным ящиком или
  доменом.
- Пока `require_email_for_registration = true`, обычная регистрация клиентов не
  заработает без рабочего SMTP.
- Первый админ создается отдельно: на пустой базе через first-run registration
  token из логов сервера, а не через email.

## 2. Поднять или перезапустить контейнеры

На сервере перейти в корень репозитория:

```bash
cd ~/continuwuity
```

Проверить итоговый compose-конфиг:

```bash
make matrix-config
```

При свежем деплое или плановом обновлении подтянуть образы:

```bash
make matrix-pull
```

Запустить или обновить контейнеры:

```bash
make matrix-up
```

Если менялся только конфиг и контейнеры уже работают, перезапустить:

```bash
make matrix-restart
```

Проверить статус:

```bash
make matrix-ps
```

Смотреть логи homeserver:

```bash
make matrix-logs
```

После настройки SMTP в логах должен появиться успешный тест соединения:

```text
SMTP connection test successful
```

Если SMTP падает, сначала исправить `connection_uri`, пароль, порт или права
отправителя у провайдера, и только потом тестировать регистрацию.

## 3. Создать первого админа по first-run token

Этот шаг нужен только если база homeserver пустая.

Запустить сервер и открыть логи:

```bash
make matrix-logs
```

В логах будет first-run banner. Он печатает registration token и предлагает
создать первый аккаунт с этим токеном.

Обычный путь через клиент:

1. Открыть Matrix-клиент, который поддерживает registration token.
2. Выбрать сервер `celesteai.ru` или `https://matrix.celesteai.ru`.
3. Начать регистрацию.
4. Ввести username и password.
5. Ввести first-run registration token из логов.
6. Завершить регистрацию.

Созданный пользователь автоматически станет админом сервера.

Если UI клиента не показывает stage для registration token, использовать API
fallback.

Сначала создать UIAA-сессию:

```bash
curl -sS https://matrix.celesteai.ru/_matrix/client/v3/register \
  -H 'Content-Type: application/json' \
  -d '{
    "username": "admin",
    "password": "REPLACE_WITH_STRONG_PASSWORD",
    "initial_device_display_name": "bootstrap"
  }'
```

Из ответа скопировать `session`.

Затем завершить регистрацию с токеном из логов:

```bash
curl -sS https://matrix.celesteai.ru/_matrix/client/v3/register \
  -H 'Content-Type: application/json' \
  -d '{
    "username": "admin",
    "password": "REPLACE_WITH_STRONG_PASSWORD",
    "initial_device_display_name": "bootstrap",
    "auth": {
      "type": "m.login.registration_token",
      "token": "FIRST_RUN_TOKEN_FROM_LOGS",
      "session": "SESSION_FROM_PREVIOUS_RESPONSE"
    }
  }'
```

Успешный ответ содержит `user_id`, `device_id` и `access_token`. `access_token`
не логировать и никому не передавать.

После создания первого админа first-run режим выключается. Обычные пользователи
после этого регистрируются через email verification.

## 4. Проверить публичные endpoints

Проверки с любой машины, где есть доступ в интернет:

```bash
curl -i https://matrix.celesteai.ru/_matrix/client/versions
curl -i https://matrix.celesteai.ru/_matrix/client/v3/login
curl -i https://celesteai.ru/.well-known/matrix/client
curl -i https://celesteai.ru/.well-known/matrix/server
curl -I https://chat.celesteai.ru
```

Ожидаемо:

- `/_matrix/client/versions` отвечает HTTP 200 и JSON.
- `/_matrix/client/v3/login` показывает flow `password` + `email_code`.
- `/.well-known/matrix/client` указывает на `https://matrix.celesteai.ru`.
- `/.well-known/matrix/client` отдает `Access-Control-Allow-Origin: *`.
- `/.well-known/matrix/server` указывает на `matrix.celesteai.ru:443`.
- Web-клиент открывается по HTTPS.

Проверить, что email validation endpoint проксируется на homeserver:

```bash
curl -i https://matrix.celesteai.ru/_continuwuity/3pid/email/validate
```

Без query-параметров bad request или страница ошибки допустимы. Proxy 404
недопустим.

## 5. Проверить обычную регистрацию

После рабочего SMTP и создания первого админа:

1. Открыть клиент.
2. Начать регистрацию на `celesteai.ru`.
3. Запросить письмо регистрации.
4. Перейти по ссылке в письме.
5. Завершить регистрацию.
6. Выйти из аккаунта.
7. Зайти снова по Matrix ID/localpart или email, если клиент поддерживает email
   identifier.
8. Проверить password reset по email.

Ожидаемое поведение username:

- Если клиент отправляет `username`, он становится Matrix localpart.
- Если клиент не отправляет `username` после email verification, сервер берет
  localpart из проверенного email. Например, `alice@example.com` станет
  `@alice:celesteai.ru`, если имя свободно.
- Полный email-адрес не становится Matrix ID.

## 6. Общий промпт для работы с форками Element

Скопировать этот блок в чат с разработчиком или AI-ассистентом, который
настраивает форк Element Web, Android или iOS.

```text
Ты помогаешь настроить форк Element для нашего мессенджера.

Целевой homeserver:
- Matrix server_name: celesteai.ru
- Homeserver base URL: https://matrix.celesteai.ru
- Web app URL: https://chat.celesteai.ru
- Client discovery URL: https://celesteai.ru/.well-known/matrix/client

Название продукта:
- Будет задано отдельно.
- Не хардкодь старое название.
- Используй плейсхолдер MESSENGER_NAME там, где нужно видимое имя продукта.

Цель:
Сделать так, чтобы форк Element корректно работал с Continuwuity homeserver для
регистрации, логина, логаута, email verification и password reset.

Общие требования к клиенту:
- По умолчанию выбирать homeserver celesteai.ru / https://matrix.celesteai.ru.
- Не использовать https://chat.celesteai.ru как homeserver API. Это только web
  app.
- Отключить guest login/registration в production UI.
- Оставить password login включенным.
- Оставить обычную Matrix registration включенной.
- Поддерживать UIAA flows, которые возвращает сервер.
- Поддерживать m.login.registration_token для первого админа/bootstrap flow,
  либо явно документировать API fallback.
- Поддерживать m.login.email.identity для обычной email-backed registration.
- Поддерживать request registration email, переход по validation link, final
  registration, login, logout и password reset email.
- Позволять логин по Matrix ID/localpart. Если платформа поддерживает Matrix
  email identifiers, также позволять логин по email.
- Не писать в логи, аналитику, crash reports или скриншоты access tokens,
  first-run registration tokens, passwords, email validation tokens и reset
  tokens.

Поведение сервера, которое нужно учитывать:
- Первый аккаунт на пустой базе создается через first-run registration token из
  логов сервера. Он автоматически становится админом.
- First-run admin creation не требует email verification.
- После создания первого админа обычная регистрация требует email verification.
- Если клиент не отправляет username при final registration после email
  verification, сервер использует localpart проверенного email как Matrix
  localpart. Пример: alice@example.com -> @alice:celesteai.ru, если имя
  свободно.
- Полный email-адрес не становится Matrix ID.
- Access tokens возвращаются Matrix login/registration и используются как Bearer
  tokens.

Element Web:
- Настроить config.json:
  - default_server_name: celesteai.ru
  - default_server_config.m.homeserver.base_url: https://matrix.celesteai.ru
  - default_server_config.m.homeserver.server_name: celesteai.ru
  - brand: MESSENGER_NAME
  - disable_custom_urls: true, кроме QA/debug сборок
  - disable_guests: true
- Проверить CORS и .well-known discovery в браузере.
- Проверить, что validation link из письма открывается:
  https://matrix.celesteai.ru/_continuwuity/3pid/email/validate

Element Android / iOS:
- Найти существующий механизм default homeserver/config в форке и выставить:
  - server name: celesteai.ru
  - base URL: https://matrix.celesteai.ru
  - brand/app name: MESSENGER_NAME
- В production скрыть или предзаполнить выбор custom homeserver.
- Оставить QA/debug вариант, где custom homeserver можно включить вручную.
- Убедиться, что universal links/app links не перехватывают email validation URL,
  если приложение не умеет завершить web validation корректно.
- Хранить access tokens только в secure storage, который уже используется
  Element на платформе.

Тестовый чеклист:
1. Fresh install открывается с целевым homeserver по умолчанию.
2. GET /_matrix/client/versions работает.
3. GET /_matrix/client/v3/login показывает flow password + email_code.
4. First admin registration работает через m.login.registration_token, либо
   API fallback описан и проверен.
5. Обычная регистрация отправляет email.
6. Email validation link помечает session как validated.
7. Final registration завершается успешно.
8. Login работает с созданным аккаунтом.
9. Logout инвалидирует текущую session.
10. Password reset email приходит и позволяет поставить новый пароль.
11. Секреты не попадают в логи, crash reports, analytics или screenshots.
12. Изменение server_name считается database-breaking server migration, а не
    настройкой клиента.

Особенно важно:
- Не путать server_name celesteai.ru и homeserver base URL
  https://matrix.celesteai.ru.
- Не использовать chat.celesteai.ru как homeserver API endpoint.
- Не предполагать open registration. После первого админа сервер требует UIAA и
  email verification.
- Не удалять поддержку registration-token UIAA. Она нужна для bootstrap.
- Не заставлять пользователя выбирать Matrix username, если целевой UX
  email-first. Сервер может вывести username из verified email localpart, если
  клиент не отправит username.
```


новый промпт:
Да. Вот готовый промпт для клиента, чтобы проверить и поправить UX без лишних догадок.

```text
Нужно проверить и привести клиентский auth flow к текущему поведению сервера.

Контекст:
- Сервер использует кастомный двухшаговый сценарий.
- Регистрация:
  1) email
  2) код подтверждения на почту
  3) username
  4) password
  5) confirm password нужна только на стороне клиента как UX-проверка, на сервер не отправляется отдельно
- Авторизация:
  1) login или email
  2) password
  3) код подтверждения на почту, привязанную к аккаунту

Что проверить в клиенте:

1. Экран регистрации
- Первый шаг должен запрашивать только `email`.
- После ввода email клиент должен вызвать:
  - `POST /_matrix/client/v3/register/email/requestToken`
- Тело запроса:
```json
{
  "client_secret": "random-client-secret",
  "email": "alice@example.com",
  "send_attempt": 1
}
```
- Ответ:
```json
{
  "sid": "session-id"
}
```
- После этого клиент должен показать экран ввода кода.
- После успешного ввода кода вызвать:
  - `POST /_matrix/client/v3/register/email/submitToken`
- Тело запроса:
```json
{
  "client_secret": "random-client-secret",
  "sid": "session-id",
  "token": "123456"
}
```
- После успешного подтверждения email клиент должен показать форму:
  - `username`
  - `password`
  - `confirm password`
- `confirm password` должен сравниваться только локально в клиенте.
- Затем вызвать:
  - `POST /_matrix/client/v3/register`
- Тело запроса должно содержать уже подтвержденный email и пароль.

2. Экран логина
- Экран входа должен запрашивать:
  - `login` или `email`
  - `password`
- После ввода этих данных клиент должен вызвать:
  - `POST /_matrix/client/v3/login`
- Тело первого запроса:
```json
{
  "client_secret": "random-client-secret",
  "login": "alice_or_email",
  "password": "secret-password",
  "send_attempt": 1
}
```
- Если пароль верный, сервер вернет `sid` и `email`.
- Клиент должен показать экран ввода кода.
- После ввода кода клиент должен повторно вызвать:
  - `POST /_matrix/client/v3/login`
- Тело второго запроса:
```json
{
  "client_secret": "random-client-secret",
  "sid": "session-id",
  "token": "123456",
  "device_id": "optional-device-id",
  "initial_device_display_name": "optional device name"
}
```
- После этого сервер вернет:
  - `user_id`
  - `access_token`
  - `device_id`
  - `home_server`
  - `refresh_token` может быть `null`

3. Что именно проверить в логике клиента
- Клиент не должен ожидать Matrix-standard login discovery с `flows[].type`.
- Клиент должен понимать кастомный flow со стадиями:
  - `password`
  - `email_code`
- Клиент не должен показывать ошибку `unsupported login flow`, если сервер отвечает этим кастомным flow.
- Если клиент сейчас парсит `/login` как стандартный Matrix login discovery, это нужно заменить.
- Если клиент сейчас пытается логиниться только одним запросом с паролем, это нужно заменить на двухшаговый сценарий.
- Если клиент сейчас ожидает регистрацию без подтверждения email, это тоже нужно заменить.

4. Что проверить в отображении ошибок
- Неверный пароль должен показываться как обычная ошибка входа.
- Неверный код должен показываться как ошибка подтверждения кода.
- Повторная отправка кода должна увеличивать `send_attempt`.
- После успешной проверки кода клиент должен продолжать flow, а не сбрасываться на старт.

5. Критерий готовности
- Регистрация проходит по схеме `email -> code -> username/password`.
- Логин проходит по схеме `login/password -> code -> session`.
- Клиент больше не пишет `unsupported login flow`.
- Не ломается повторная отправка кода.
- `confirm password` работает только как локальная проверка совпадения.

Если полезно, могу ещё сразу написать короткий вариант этого же промпта в формате для issue/PR description.
```

Если хочешь, я могу ещё сделать:
1. короткий вариант для GitHub issue,
2. более технический вариант для разработчика клиента,
3. вариант именно под Element/Matrix-клиент.