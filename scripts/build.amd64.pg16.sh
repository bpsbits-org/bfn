#!/usr/bin/env bash
# scripts/build.amd64.pg16.sh

# Define locations
MS_START=$(date +%s)
readonly CONTAINER_NAME="build-pgrx"
readonly PGRX_BUILDER_FILE="${BASH_SOURCE[0]}"
PGRX_BUILDER_DIR="$(dirname "${PGRX_BUILDER_FILE}")"
PGRX_PROJECT_DIR="$(dirname "${PGRX_BUILDER_DIR}")"
readonly MS_START
readonly PGRX_BUILDER_DIR
readonly PGRX_PROJECT_DIR

# Check if the container exists
if podman ps -a --format "{{.Names}}" | grep -qw "${CONTAINER_NAME}"; then
    echo -e "Old build container \`${CONTAINER_NAME}\` exists."
    # Stop the container if it's running
    if podman ps --format "{{.Names}}" | grep -qw "${CONTAINER_NAME}"; then
        echo -e "Stopping old build container \`${CONTAINER_NAME}\` ..."
        podman stop "${CONTAINER_NAME}"
    fi
    # Remove the container
    echo -e "Removing old build container \`${CONTAINER_NAME}\` ..."
    podman rm "${CONTAINER_NAME}"
else
    echo -e "Old build container \`${CONTAINER_NAME}\` does not exist."
fi

# Go to the project directory
cd "${PGRX_PROJECT_DIR}" || exit 1

# Clean build directory
mkdir -p ./build
rm -rf ./build/{*,.[!.]*,..?*}

# Build container
podman build --arch amd64 -f Containerfile.pg16 -t build-pgrx:snapshot .

# Run container that runs the builder
podman run -d --name "${CONTAINER_NAME}" localhost/build-pgrx:snapshot

# Copy build to ./build
podman cp "${CONTAINER_NAME}":/root/pgrx-build/packed/bfn-distro-pg16.tar.gz ./build

# Copy install script to ./build
cp "${PGRX_PROJECT_DIR}/scripts/install-pg16.sh" ./build

# Time spent
MSR_END=$(date +%s)
readonly MSR_END
TIME_SPENT=$(( MSR_END - MS_START ))
echo "Elapsed time: ${TIME_SPENT} seconds"

exit 0