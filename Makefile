.PHONY: matrix-help matrix-pull matrix-up matrix-down matrix-restart matrix-logs matrix-ps matrix-config

MATRIX_COMPOSE_FILE := deploy/celesteai/docker-compose.yml
MATRIX_COMPOSE := docker compose -f $(MATRIX_COMPOSE_FILE)

matrix-help:
	@printf "%s\n" \
		"Available targets:" \
		"  make matrix-pull    - pull latest images" \
		"  make matrix-up      - start or update containers in background" \
		"  make matrix-down    - stop and remove containers" \
		"  make matrix-restart - restart the deployment" \
		"  make matrix-logs    - follow homeserver logs" \
		"  make matrix-ps      - show container status" \
		"  make matrix-config  - validate rendered compose config"

matrix-pull:
	$(MATRIX_COMPOSE) pull

matrix-up:
	$(MATRIX_COMPOSE) up -d

matrix-down:
	$(MATRIX_COMPOSE) down

matrix-restart:
	$(MATRIX_COMPOSE) down
	$(MATRIX_COMPOSE) up -d

matrix-logs:
	$(MATRIX_COMPOSE) logs -f continuwuity

matrix-ps:
	$(MATRIX_COMPOSE) ps

matrix-config:
	$(MATRIX_COMPOSE) config
