.PHONY: matrix-help matrix-pull matrix-up matrix-down matrix-restart matrix-logs matrix-ps matrix-config

MATRIX_DEPLOY_DIR := deploy/celesteai

matrix-help:
	$(MAKE) -C $(MATRIX_DEPLOY_DIR) help

matrix-pull:
	$(MAKE) -C $(MATRIX_DEPLOY_DIR) pull

matrix-up:
	$(MAKE) -C $(MATRIX_DEPLOY_DIR) up

matrix-down:
	$(MAKE) -C $(MATRIX_DEPLOY_DIR) down

matrix-restart:
	$(MAKE) -C $(MATRIX_DEPLOY_DIR) restart

matrix-logs:
	$(MAKE) -C $(MATRIX_DEPLOY_DIR) logs

matrix-ps:
	$(MAKE) -C $(MATRIX_DEPLOY_DIR) ps

matrix-config:
	$(MAKE) -C $(MATRIX_DEPLOY_DIR) config
