#!/usr/bin/env bash

## Pull postgres 16 image
podman pull docker.io/library/postgres:16.6

## Make container for testing purposes
podman create \
	--name pg-16-test \
	--restart always \
	-p 5432:5432 \
	-e POSTGRES_PASSWORD="12345678" \
	docker.io/library/postgres:16.4

## Run container
podman start pg-16-test

## check that curl exists in test container
podman exec --user root -it pg-16-test curl --version

## Copy deploy script to container
podman cp deploy-pg16.sh pg-16-test:/

## Run deploy script in container
podman exec pg-16-test bash /deploy-pg16.sh

## Check linux version in container
podman exec pg-16-test cat /etc/os-release

## Install curl in container
podman exec --user root -it pg-16-test apt-get update
podman exec --user root -it pg-16-test apt-get install -y curl

## Example how to make a version upgrade file
cp /usr/share/postgresql/16/extension/bfn--2.0.1.sql /usr/share/postgresql/16/extension/bfn--2.0.0--2.0.1.sql
