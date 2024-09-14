PROJECT_NAME=skatebuddy
ELF=firmware/target/thumbv7em-none-eabihf/release/skatebuddy
DISABLE_DEBUG_SCRIPT=./disable_debug_interface.sh

all: ${ELF}

${ELF}:
	cd firmware; cargo build --release

.PHONY: flash ${ELF} simu

flash: ${ELF}
	probe-run --chip nRF52840_xxAA ${ELF} | tools/filter_output.py || ${DISABLE_DEBUG_SCRIPT}

reset: ${ELF}
	probe-run --chip nRF52840_xxAA ${ELF} --no-flash | tools/filter_output.py || ${DISABLE_DEBUG_SCRIPT}

simu:
	pushd firmware; cargo run --target=x86_64-unknown-linux-gnu; popd
