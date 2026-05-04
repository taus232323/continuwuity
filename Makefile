.PHONY: help pull up down restart logs ps config

MATRIX_COMPOSE_FILE := deploy/docker-compose.yml
MATRIX_COMPOSE_PROJECT ?= celesteai
MATRIX_COMPOSE := docker compose -p $(MATRIX_COMPOSE_PROJECT) -f $(MATRIX_COMPOSE_FILE)

help:
	@printf "%s\n" \
		"Available targets:" \
		"  make pull    - pull latest images" \
		"  make up      - start or update containers in background" \
		"  make down    - stop and remove containers" \
		"  make restart - restart the deployment" \
		"  make logs    - follow homeserver logs" \
		"  make ps      - show container status" \
		"  make config  - validate rendered compose config"

pull:
	$(MATRIX_COMPOSE) pull

up:
	$(MATRIX_COMPOSE) up -d

matrix-down:
	$(MATRIX_COMPOSE) down

restart:
	$(MATRIX_COMPOSE) down
	$(MATRIX_COMPOSE) up -d

logs:
	$(MATRIX_COMPOSE) logs -f continuwuity

ps:
	$(MATRIX_COMPOSE) ps

config:
	$(MATRIX_COMPOSE) config
