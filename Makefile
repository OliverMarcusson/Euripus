.PHONY: homelab-up homelab-down

EURIPUS_ENABLE_NORDVPN ?= true

homelab-up:
	EURIPUS_ENABLE_NORDVPN=$(EURIPUS_ENABLE_NORDVPN) ./scripts/deploy.sh

homelab-down:
	@if [ "$(EURIPUS_ENABLE_NORDVPN)" = "true" ]; then \
		podman compose -f docker-compose.homelab.yml -f docker-compose.homelab.nordvpn.yml down; \
	else \
		podman compose -f docker-compose.homelab.yml down; \
	fi
