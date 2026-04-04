.PHONY: homelab-up homelab-down

COMPOSE_FILES = -f docker-compose.homelab.yml -f docker-compose.homelab.nordvpn.yml

homelab-up:
	podman compose $(COMPOSE_FILES) up --build -d

homelab-down:
	podman compose $(COMPOSE_FILES) down
