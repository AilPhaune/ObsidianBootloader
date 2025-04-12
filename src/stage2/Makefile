SRC_DIR?=src
CARGO?=cargo

ASM?=nasm
ASM_FLAGS?=-f elf32 -F dwarf -g

LD = "ld.lld"

ifeq ($(MODE),debug)
	CARGO_CONFIG=--profile dev
	CARGO_BUILD_DIR=target/x86-unknown-bare_metal/debug
else
	CARGO_CONFIG=--release
	CARGO_BUILD_DIR=target/x86-unknown-bare_metal/release
endif

.PHONY: all stage2asm stage2 clean

all: stage2asm stage2

stage2: stage2asm
	$(CARGO) build $(CARGO_CONFIG)
	$(LD) -T linker.ld main.o $(CARGO_BUILD_DIR)/libstage2.a -o ../../build/stage2.o

	objcopy --only-keep-debug ../../build/stage2.o ../../build/bootloader_stage2.debug
	objcopy -O binary ../../build/stage2.o ../../build/bootloader_stage2.bin

stage2asm: main.o

main.o: $(shell find . -type f -name '*.asm')
	mkdir -p build
	$(ASM) $(ASM_FLAGS) -o main.o main.asm

clean:
	rm -rf target
	rm main.o