.PHONY: help pull up down restart logs ps config

COMPOSE := docker compose

help:
	@printf "%s\n" \
		"Available targets:" \
		"  make pull     - pull latest images" \
		"  make up       - start or update containers in background" \
		"  make down     - stop and remove containers" \
		"  make restart  - restart the deployment" \
		"  make logs     - follow homeserver logs" \
		"  make ps       - show container status" \
		"  make config   - validate rendered compose config"

pull:
	$(COMPOSE) pull

up:
	$(COMPOSE) up -d

down:
	$(COMPOSE) down

restart:
	$(COMPOSE) down
	$(COMPOSE) up -d

logs:
	$(COMPOSE) logs -f continuwuity

ps:
	$(COMPOSE) ps

config:
	$(COMPOSE) config
