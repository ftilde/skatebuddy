PROJECT_NAME=skatebuddy
ELF=firmware/target/thumbv7em-none-eabihf/release/skatebuddy
DISABLE_DEBUG_SCRIPT=./disable_debug_interface.sh

all: ${ELF}

${ELF}:
	pushd firmware; cargo build --release; popd

.PHONY: flash ${ELF}

flash: ${ELF}
	probe-run --chip nRF52840_xxAA ${ELF} || ${DISABLE_DEBUG_SCRIPT}

reset: ${ELF}
	probe-run --chip nRF52840_xxAA ${ELF} --no-flash || ${DISABLE_DEBUG_SCRIPT}

simu: ${ELF}
	pushd firmware; cargo run --target=x86_64-unknown-linux-gnu; popd
