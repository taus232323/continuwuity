# Celeste Matrix Deployment

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

## Deploy On Server

Copy this directory to the server, for example:

```bash
scp -r deploy/celesteai celeste:/root/
```

Then on the server:

```bash
cd /root/celesteai
docker compose pull
docker compose up -d
docker compose logs -f continuwuity
```

## Verify

After start, check:

```bash
curl -i https://matrix.celesteai.ru/_matrix/client/versions
curl -i https://celesteai.ru/.well-known/matrix/server
curl -i https://celesteai.ru/.well-known/matrix/client
curl -I https://chat.celesteai.ru
```

## Notes

- registration is left at the application default for the first bootstrap
- if you want open or token-based registration, extend `continuwuity.toml`
- if you want a support endpoint, add it either in Continuwuity or in `well-known/nginx.conf`
