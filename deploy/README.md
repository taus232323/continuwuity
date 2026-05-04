# Matrix Deployment

This directory contains a deploy-ready setup for:

- `matrix.celesteai.ru` -> Continuwuity homeserver
- `chat.celesteai.ru` -> Element Web
- `celesteai.ru/.well-known/matrix/*` -> static Matrix discovery

The setup is designed for the current server layout on `celeste`:

- Traefik runs as a system service
- Traefik forwards to localhost services
- the following local ports are already reserved in Traefik:
  - `6167` for Continuwuity
  - `3300` for Element Web
  - `3310` for `.well-known`

## Files

- `docker-compose.yml` - starts the three containers
- `continuwuity.toml` - homeserver config
- `element-config.json` - Element Web config
- `well-known/nginx.conf` - static `.well-known` endpoints
- `continuwuity-resolv.conf` - avoids Docker DNS federation issues

## Important

- `server_name` is set to `celesteai.ru`
- changing `server_name` later requires wiping the homeserver database
- user IDs will look like `@user:celesteai.ru`
- email registration is supported by the homeserver once SMTP is configured
- if the client omits `username` during registration, Continuwuity will use the
  verified email localpart as the initial Matrix username

## Registration Model

This deployment is now prepared for email-backed registration:

- registration is enabled
- email is required during registration
- email must be validated before registration completes
- password reset works through email
- users can log in with email if the client sends an email identifier at login

This is a good baseline if you want "email-first" onboarding without MAS.

What it does not do by itself:

- it does not make the Matrix user ID equal to the full email address
- it does not stop all bot signups on its own

In practice, if a user verifies `alice@example.com` and the client does not send
an explicit `username`, the created Matrix ID will default to something like
`@alice:celesteai.ru`.

## SMTP Setup

SMTP credentials are now supplied through the local `.env` file in the
repository root, not committed to git.

1. Copy [`.env.example`](</Users/taus/Projects/continuwuity/.env.example>) to
   `../.env` from this directory.
2. Fill in your real Yandex app password.
3. Keep `deploy/continuwuity.toml` unchanged unless you want to change the
   registration policy.

Required variables:

- `CONTINUWUITY_SMTP__CONNECTION_URI`
- `CONTINUWUITY_SMTP__SENDER`
- `CONTINUWUITY_SMTP__REQUIRE_EMAIL_FOR_REGISTRATION`
- `CONTINUWUITY_SMTP__REQUIRE_EMAIL_FOR_TOKEN_REGISTRATION`

Also replace visible product placeholders:

- `"brand": "CHANGE_ME"` in `element-config.json`

The Docker Compose project is pinned by the root `Makefile` as `celesteai` to
keep the existing deployment volume after moving files from `deploy/celesteai/`
to `deploy/`. This is infrastructure naming, not the public messenger name.

## Recommended Hardening

Email verification is better than open registration, but it is still not strong
anti-bot protection by itself. If abuse starts, add one of these:

- enable `suspend_on_register = true` and review new users manually
- add reCAPTCHA in `continuwuity.toml`
- switch back to registration tokens for closed onboarding

## Deploy On Server

Copy this directory to the server, for example:

```bash
scp -r deploy celeste:/root/continuwuity-deploy
```

Then on the server:

```bash
cd ~/continuwuity
make matrix-pull
make matrix-up
make matrix-logs
```

## Verify

After start, check:

```bash
curl -i https://matrix.celesteai.ru/_matrix/client/versions
curl -i https://celesteai.ru/.well-known/matrix/server
curl -i https://celesteai.ru/.well-known/matrix/client
curl -I https://chat.celesteai.ru
```

For email flows, also verify:

```bash
curl -i https://matrix.celesteai.ru/_continuwuity/3pid/email/validate
```

Then test from a client:

- request registration email
- click the verification link from the mailbox
- complete registration without sending a separate `username`
- log in using the email address as identifier
- request a password reset email

## Notes

- if you want a support endpoint, add it either in Continuwuity or in `well-known/nginx.conf`
- if you want tighter anti-abuse controls later, combine email verification with
  reCAPTCHA or admin approval
