#!/usr/bin/env bash
# -------------------------------------------------------------------
# Installs BFN into PostgreSQL environment
#
# Usage example (terminal, preloaded file):
# bash ./deploy-pg16.sh
#
# Usage example (terminal, from GitHub):
# curl -sL https://raw.githubusercontent.com/bpsbits-org/bfn/main/scripts/deploy-pg16.sh | bash
#
# Usage example (podman, form GitHub)
# podman run -d curl -sL https://raw.githubusercontent.com/bpsbits-org/bfn/main/scripts/deploy-pg16.sh | bash
#
# -------------------------------------------------------------------
readonly BFN_DEP_FILE="${BASH_SOURCE[0]}"

if [[ ! -d "/usr/share/postgresql/16" ]]; then
	echo -e "PostgreSQL 16 directory does not exist"
	exit 1
fi

## Make tmp dir for deployment usage
mkdir -p /var/lib/postgresql/tmp
cd /var/lib/postgresql/tmp || exit 1

## Download extension
curl -s https://api.github.com/repos/bpsbits-org/bfn/releases/latest \
| grep "browser_download_url.*bfn-distro-pg16.tar.gz" \
| cut -d '"' -f 4 \
| xargs curl -L -o bfn-distro-pg16.tar.gz

## Download installer
curl -s https://api.github.com/repos/bpsbits-org/bfn/releases/latest \
| grep "browser_download_url.*install-pg16.sh" \
| cut -d '"' -f 4 \
| xargs curl -L -o install-pg16.sh

if [[ -f "install-pg16.sh" && -f "bfn-distro-pg16.tar.gz" ]]; then
    bash ./install-pg16.sh
    rm -f bfn-distro-pg16.tar.gz
    rm -f install-pg16.sh
    rm -f "${BFN_DEP_FILE}"
else
    echo -e "One or both of deployment files are missing"
    exit 0
fi
