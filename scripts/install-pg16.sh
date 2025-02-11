#!/usr/bin/env bash
# Installs distro in PostgreSQl environment
# This script is not suitable for any configurations.
readonly BFN_INSTALL_FILE="${BASH_SOURCE[0]}"
BFN_DISTRO_DIR="$(dirname "${BFN_INSTALL_FILE}")"
BFN_DISTRO_PKG="bfn-distro-pg16.tar.gz"

cd "${BFN_DISTRO_DIR}" || exit 1

if [ -f "${BFN_DISTRO_PKG}" ]; then
	## Check that psql exists
	if [[ -d "/usr/lib/postgresql/16" ]]; then
		echo "Deploying BFN from '${BFN_DISTRO_PKG}'...";
		mkdir -p tmp
		# Unpack
		tar -xzf "${BFN_DISTRO_PKG}" -C /
		# Fix privileges
		if id -u root > /dev/null 2>&1; then
			echo "Finalizing...";
			chown root:root /usr/lib/postgresql/16/lib/bfn.so
			chown root:root /usr/share/postgresql/16/extension/bfn.control
			chown root:root /usr/share/postgresql/16/extension/bfn-*.sql
		fi
		echo "Done.";
		return 0
    else
        echo "PostgreSQL 16 directory does not exist"
        return 1
    fi
else
    echo "BFN package '${BFN_DISTRO_PKG}' does not exist.";
    return 1
fi