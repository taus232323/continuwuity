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


Если полезно, могу ещё сразу написать короткий вариант этого же промпта в формате для issue/PR description.
```

Если хочешь, я могу ещё сделать:
1. короткий вариант для GitHub issue,
2. более технический вариант для разработчика клиента,
3. вариант именно под Element/Matrix-клиент.