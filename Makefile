SHELL := /bin/bash

.PHONY: it-local it-local-build it-local-up it-local-test it-local-down policy-build

policy-build:
	./scripts/build-policy-artifacts.sh

it-local-build: policy-build
	cargo build -p gateway-cp -p gateway-dp -p gateway-it

it-local-up:
	./scripts/it-local.sh up

it-local-test:
	./scripts/it-local.sh test

it-local-down:
	./scripts/it-local.sh down

it-local:
	./scripts/it-local.sh run
