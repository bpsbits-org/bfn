#!/bin/bash

set -e

readonly BFN_SUPPORTED_VERSIONS=('16' '17' '18')
readonly FILE_BFN_BUILDER_SCRIPT="${SSC_ARGV0:-${BASH_SOURCE[0]}}"
DIR_BFN_BUILDER_SCRIPT="$(dirname "${FILE_BFN_BUILDER_SCRIPT}")"
DIR_BFN_PROJECT="$(dirname "${DIR_BFN_BUILDER_SCRIPT}")"
readonly DIR_BFN_PROJECT

set_toml_pg_version(){
	local FILE_TOM v
	v=$1
	[[ -z $v ]] && { echo "Please provide PG version" >&2; exit 1; }
	for x in "${BFN_SUPPORTED_VERSIONS[@]}"; do [[ $v == "$x" ]] && break; done
	[[ $v != "$x" ]] && { echo "Invalid PG version" >&2; exit 1; }
	export BFN_PG_V=$1
	FILE_TOM="${DIR_BFN_PROJECT}/Cargo.toml"
	if [ ! -f "${FILE_TOM}" ]; then
		exho "Cargo.toml file not found"
		exit 1
	fi
	echo "Altering Cargo.toml ..."
	sed -i.bak -E "
	  /^\[features\]/,/^(\[.*\]|$)/ {
		s/^([[:space:]]*default[[:space:]]*=[[:space:]]*\[\").*/\1pg${BFN_PG_V}\"]/
	  }
	" "${FILE_TOM}"
	echo "PG version changed to ${BFN_PG_V}"
}

# Check if --set argument is provided
# bash ./scripts/bfn-builder.sh --set
if [[ "$1" == "--set" ]]; then
    set_toml_pg_version "$2"
fi