#!/bin/bash

set -e

readonly BFN_SUPPORTED_VERSIONS=('16' '17' '18')
readonly FILE_BFN_BUILDER_SCRIPT="${SSC_ARGV0:-${BASH_SOURCE[0]}}"
DIR_BFN_BUILDER_SCRIPT="$(dirname "${FILE_BFN_BUILDER_SCRIPT}")"
DIR_BFN_PROJECT="$(dirname "${DIR_BFN_BUILDER_SCRIPT}")"
readonly DIR_BFN_PROJECT

cd "${DIR_BFN_PROJECT}" || exit 1

# Create image for PGRX builder
build_pgrx_image(){
	if podman image exists "localhost/${BFN_CNT_BUILD_NAME}"; then
    	echo "Image '${BFN_BUILD_CNT_NAME}' exists."
    else
    	echo "Building '${BFN_BUILD_CNT_NAME}'..."
    	podman build --arch amd64 -f "Containerfile.pg${BFN_PG_V}.pgrx" -t "${BFN_CNT_BUILD_NAME}" .
    fi
}

# Start container of PGRX builder
start_pgrx_cnt(){
    podman start "${BFN_BUILD_CNT_NAME}"
    #podman exec --user root "${BFN_BUILD_CNT_NAME}" cat /etc/os-release
    echo -e "\nUse command below to access container"
    echo -e "\t podman exec -it ${BFN_BUILD_CNT_NAME} /bin/bash\n"
}

# Create container of PGRX builder
create_pgrx_container(){
	build_pgrx_image
	echo -e "Creating container \`${BFN_BUILD_CNT_NAME}\`..."
    podman create --name "${BFN_BUILD_CNT_NAME}" \
        --platform linux/amd64 \
        --workdir /root/pgrx-project \
        --entrypoint /bin/sh \
        "localhost/${BFN_CNT_BUILD_NAME}" \
        -c "sleep infinity"
}

# Stop container of PGRX builder
stop_pgrx_builder(){
	# Check if container runs
	CNT_RUNNING=$(podman inspect "${BFN_BUILD_CNT_NAME}" --format '{{.State.Running}}' 2>/dev/null)
	if [ "${CNT_RUNNING}" = "false" ]; then
		echo "Container '${BFN_BUILD_CNT_NAME}' is stopped."
	else
		podman stop "${BFN_BUILD_CNT_NAME}"
	fi
}

# Start PGRX builder
start_pgrx_builder(){
    # Check if the container exists
    if podman ps -a --format "{{.Names}}" | grep -qw "${BFN_BUILD_CNT_NAME}"; then
        echo -e "Container \`${BFN_BUILD_CNT_NAME}\` exists."
        # Check if container runs
        CNT_RUNNING=$(podman inspect "${BFN_BUILD_CNT_NAME}" --format '{{.State.Running}}' 2>/dev/null)
        if [ "${CNT_RUNNING}" = "false" ]; then
            echo "Container '${BFN_BUILD_CNT_NAME}' is stopped."
            start_pgrx_cnt
        fi
    else
        echo -e "Container \`${BFN_BUILD_CNT_NAME}\` does not exist."
        create_pgrx_container
        start_pgrx_cnt
    fi
}

# Prepares data for building extensions
build_bfn_prepare() {
	echo -e "\nPreparing data for build...\n"
	podman exec --user root "$BFN_BUILD_CNT_NAME" find /root/pgrx-project -mindepth 1 -maxdepth 1 -exec rm -rf {} +
	# Copy project into container
	podman cp "${DIR_BFN_PROJECT}/." "${BFN_BUILD_CNT_NAME}":/root/pgrx-project
	# Sanitize
	podman exec --user root "${BFN_BUILD_CNT_NAME}" rm -f ./Cargo.lock
	podman exec --user root "${BFN_BUILD_CNT_NAME}" rm -rf ./target/*
	podman exec --user root "${BFN_BUILD_CNT_NAME}" rm -rf ./build
	podman exec --user root "${BFN_BUILD_CNT_NAME}" rm -rf ./.git
	podman exec --user root "${BFN_BUILD_CNT_NAME}" rm -rf ./.idea
	podman exec --user root "${BFN_BUILD_CNT_NAME}" bash -c "bash ./scripts/set-pg-version.sh --set ${BFN_PG_V}"
	podman exec --user root "${BFN_BUILD_CNT_NAME}" sed -i "s/\[target.'cfg(target_os=\"macos\")'\]/[target.'cfg(target_os=\"linux\")']/g" ./.cargo/config.toml
}

# Builds extension
build_bfn_build(){
	echo -e "\nBuilding extension...\n"
	podman exec --user root "${BFN_BUILD_CNT_NAME}" bash -c 'export CARGO_PROFILE=release && cargo pgrx package --profile release'
}

# Builds debian install packages
build_bfn_pack(){
    podman exec --user root "${BFN_BUILD_CNT_NAME}" bash -c "bash /root/pgrx-project/dist/pack-pg.sh --pack ${BFN_PG_V}"
}

# Copies packages from container
copy_bfn_pack(){
	echo -e "\nCoping Debian install package..."
	mkdir -p "${DIR_BFN_BUILD}"
	podman cp -a "${BFN_BUILD_CNT_NAME}":"/root/pgrx-project/pack/pg${BFN_PG_V}/dist/." "${DIR_BFN_BUILD}/"
	ls -lh "${DIR_BFN_BUILD}"
}

# Builds bfn extension and creates distro packages
build_bfn_distro(){
	local v=$1
	[[ -z $v ]] && { echo "Invalid PG version" >&2; exit 1; }
	for x in "${BFN_SUPPORTED_VERSIONS[@]}"; do [[ $v == "$x" ]] && break; done
	[[ $v != "$x" ]] && { echo "Invalid PG version" >&2; exit 1; }
	export BFN_PG_V=$1
	export BFN_BUILD_CNT_NAME="pgrx_builder_pg${BFN_PG_V}"
	export BFN_CNT_BUILD_NAME="${BFN_BUILD_CNT_NAME}:latest"
    export DIR_BFN_BUILD="${DIR_BFN_PROJECT}/build/pg${BFN_PG_V}"
    if [ -d "${DIR_BFN_BUILD}" ]; then
    	rm -rf "${DIR_BFN_BUILD}"
    fi
	echo -e "\nBuilding distro for PostgreSQL ${BFN_PG_V} ...\n"
	# Handle build
	start_pgrx_builder
	build_bfn_prepare
	build_bfn_build
	build_bfn_pack
	copy_bfn_pack
	stop_pgrx_builder
}

# Check if --build argument is provided
# bash ./scripts/bfn-builder.sh --build
if [[ "$1" == "--build" ]]; then
    build_bfn_distro "$2"
fi