#!/bin/bash
# ./dist/pack-pg.sh
set -e
readonly BFN_SUPPORTED_VERSIONS=('16' '17' '18')
readonly FILE_BFN_PACK_SCRIPT="${SSC_ARGV0:-${BASH_SOURCE[0]}}"

function bfn_creat_deb_pack(){
	local X_DIR B_DIR F_INST BUILD_DIR base file path dir v TAR_PACK_NAME
	v=$1
	[[ -z $v ]] && { echo "Please provide PG version" >&2; exit 1; }
	for x in "${BFN_SUPPORTED_VERSIONS[@]}"; do [[ $v == "$x" ]] && break; done
	[[ $v != "$x" ]] && { echo "Invalid PG version" >&2; exit 1; }
	export BFN_PG_VERSION=$1
	X_DIR="$(dirname "${FILE_BFN_PACK_SCRIPT}")"
    B_DIR="$(dirname "${X_DIR}")"
    BUILD_DIR="${B_DIR}/pack/pg${BFN_PG_VERSION}/dist"
    SOURCE_DIR="${B_DIR}/target/release/bfn-pg${BFN_PG_VERSION}"
    F_INST="${BUILD_DIR}/bfn/debian/bfn.install"

    # Make Dirs
    if [ -d "${BUILD_DIR}" ]; then
    	rm -rf "${BUILD_DIR}"
    fi
    mkdir -p "${BUILD_DIR}"
    cp -a "${B_DIR}/dist/." "${BUILD_DIR}/"

    cd "${SOURCE_DIR}" || exit 1

    # Display tree of files being added to the install package
    echo -e "\nFiles to add into install pack:"
    tree ./

	# Copy files and update install file
	echo '' > "${F_INST}"
    while IFS= read -r -d '' file; do
      base=$(basename "${file}")
      path=${file#./}
      dir="$(dirname "${path}")"
      printf "%s %s\n" "${base}" "/${dir}" >> "${F_INST}"
      cp "${file}" "${BUILD_DIR}/bfn/"
    done < <(find . -type f -not -name ".*" -print0)

    echo -e "\nInstall file:"
    cat "${F_INST}"

    # Build debian package
    cd "${BUILD_DIR}/bfn" || exit
    echo -e "\nCreating .deb package:\n"
    dpkg-buildpackage -rfakeroot -us -uc -b
    # Cleanup
    rm -rf "${BUILD_DIR}/bfn"
	find "${BUILD_DIR}" -maxdepth 1 -type f -name '*dbgsym*' -delete
	find "${BUILD_DIR}" -maxdepth 1 -type f '!' -name '*.deb' -delete
	# Rename
	find "${BUILD_DIR}" -type f -name "*.deb" | while IFS= read -r file; do
		base=$(basename "${file}" .deb)
		new_file="${BUILD_DIR}/pg${BFN_PG_VERSION}_${base}.deb"
		mv -v "${file}" "${new_file}"
	done
	# Make compressed package
	TAR_PACK_NAME=$(find "${BUILD_DIR}" -maxdepth 1 -type f -name '*.deb' -printf '%f\n' -quit)
	TAR_PACK_NAME=${TAR_PACK_NAME%.deb}
	if [ -n "${TAR_PACK_NAME}" ]; then
        echo -e "\nCreating .gz package:\n"
        find "${SOURCE_DIR}" -exec touch -m -t 197001010000 {} + 2>/dev/null
        if [ -f "${BUILD_DIR}/${TAR_PACK_NAME}.tar.gz" ]; then
        	rm -f "${BUILD_DIR}/${TAR_PACK_NAME}.tar.gz"
        fi
        tar -czf "${BUILD_DIR}/${TAR_PACK_NAME}.tar.gz" -C "${SOURCE_DIR}" --transform='s,^\./,/,' .
    fi
	tree "${BUILD_DIR}"
}

# Check if --pack argument is provided
# bash ./scripts/pack-pg.sh --pack 18
if [[ "$1" == "--pack" ]]; then
	echo -e "\nCreating Debian install package..."
    bfn_creat_deb_pack "$2"
fi