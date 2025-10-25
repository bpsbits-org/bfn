#!/bin/bash

set -e
readonly BFN_SUPPORTED_VERSIONS=('16' '17' '18')
readonly FILE_BFN_PDM_SCRIPT="${SSC_ARGV0:-${BASH_SOURCE[0]}}"
DIR_BNF_PRJ=$(realpath "${FILE_BFN_PDM_SCRIPT}/../../../..")
readonly DIR_BNF_PRJ
readonly DIR_BFN_BUILD="${DIR_BNF_PRJ}/build"

create_pgrx_tester_cnt(){
	local SRV_PORT="644${BFN_PG_V}"
	echo -e "Creating container \`${BFN_TEST_CNT_NAME}\`..."
    podman create \
        --name "${BFN_TEST_CNT_NAME}" \
        --restart always \
        -p "${SRV_PORT}":5432 \
        -e POSTGRES_PASSWORD="12345678" \
        --platform linux/amd64 \
        "${BFN_TEST_CNT_IMAGE}"
    echo "Port of ${BFN_TEST_CNT_NAME}: ${SRV_PORT}"
}

stop_pgrx_tester_cnt(){
	# Check if container runs
	CNT_RUNNING=$(podman inspect "${BFN_TEST_CNT_NAME}" --format '{{.State.Running}}' 2>/dev/null)
	if [ "${CNT_RUNNING}" = "false" ]; then
		echo "Container '${BFN_TEST_CNT_NAME}' is stopped."
	else
		podman stop "${BFN_TEST_CNT_NAME}"
	fi
}

print_server_acc_command(){
	echo -e "\nUse command below to access container"
	echo -e "\t podman exec -it ${BFN_TEST_CNT_NAME} /bin/bash\n"
}

start_pgrx_tester_cnt(){
    podman start "${BFN_TEST_CNT_NAME}"
}

run_pgrx_tester_cnt(){
    # Check if the container exists
    if podman ps -a --format "{{.Names}}" | grep -qw "${BFN_TEST_CNT_NAME}"; then
        echo -e "Container \`${BFN_TEST_CNT_NAME}\` exists."
        # Check if container runs
        CNT_RUNNING=$(podman inspect "${BFN_TEST_CNT_NAME}" --format '{{.State.Running}}' 2>/dev/null)
        if [ "${CNT_RUNNING}" = "false" ]; then
            echo "Container '${BFN_TEST_CNT_NAME}' is stopped."
            start_pgrx_tester_cnt
        fi
    else
        echo -e "Container \`${BFN_TEST_CNT_NAME}\` does not exist."
        create_pgrx_tester_cnt
        start_pgrx_tester_cnt
    fi
    add_bfn_if_needed
}

add_bfn_if_needed(){
	local PG_DIR DEB_PACK_NAME INSTALLED_SO
	PG_DIR="${DIR_BFN_BUILD}/pg${BFN_PG_V}"
	sleep 5
	podman exec --user root "${BFN_TEST_CNT_NAME}" psql -U postgres -c "SELECT version();"
	podman exec --user root "${BFN_TEST_CNT_NAME}" psql -U postgres -c "\l"
	if [ -d "${PG_DIR}" ]; then
		DEB_PACK_NAME=$(find "${PG_DIR}" -type f -name '*.deb' -exec basename {} \; | head -n 1)
		INSTALLED_SO=$(podman exec --user root "${BFN_TEST_CNT_NAME}" find /usr/lib/postgresql -type f -name bfn.so)
		if [ -z "${INSTALLED_SO}" ]; then
			echo "Installing ${DEB_PACK_NAME}.."
			podman cp "${PG_DIR}/${DEB_PACK_NAME}" "${BFN_TEST_CNT_NAME}":"/tmp/${DEB_PACK_NAME}"
			podman exec --user root "${BFN_TEST_CNT_NAME}" dpkg -i "/tmp/${DEB_PACK_NAME}" || apt-get install -f -y
			podman exec --user root "${BFN_TEST_CNT_NAME}" createdb -U postgres bfn 2>/dev/null || true
			podman exec --user root "${BFN_TEST_CNT_NAME}" psql -U postgres -d bfn -c "CREATE SCHEMA IF NOT EXISTS bfn;" || true
			podman exec --user root "${BFN_TEST_CNT_NAME}" psql -U postgres -d bfn -c "CREATE EXTENSION IF NOT EXISTS bfn SCHEMA bfn;" || true
			podman exec --user root "${BFN_TEST_CNT_NAME}" psql -U postgres -d bfn -c "select bfn.version();" || true
			podman exec --user root "${BFN_TEST_CNT_NAME}" psql -U postgres -d bfn -c "select bfn.new_uuid();" || true
			podman exec --user root "${BFN_TEST_CNT_NAME}" psql -U postgres -d bfn -c "select bfn.last_day_of_month('2020-02-03');" || true
			podman exec --user root "${BFN_TEST_CNT_NAME}" psql -U postgres -d bfn -c "select bfn.last_day_of_month('2021-02-03');" || true
		else
			echo "BFN is already installed"
		fi
	else
		echo "BFN install package is not found"
	fi
}

run_test_srv_of_version (){
	local v=$1
	[[ -z $v ]] && { echo "Invalid PG version" >&2; exit 1; }
	for x in "${BFN_SUPPORTED_VERSIONS[@]}"; do [[ $v == "$x" ]] && break; done
	[[ $v != "$x" ]] && { echo "Invalid PG version" >&2; exit 1; }
	export BFN_PG_V=$1
	export BFN_TEST_CNT_NAME="pgrx_test_pg${BFN_PG_V}"
	export BFN_TEST_CNT_IMAGE="docker.io/library/postgres:${BFN_PG_V}"
	run_pgrx_tester_cnt
	print_server_acc_command
}

# Check if --run argument is provided
# bash ./scripts/bfn-builder.sh --run 16
if [[ "$1" == "--run" ]]; then
    run_test_srv_of_version "$2"
fi