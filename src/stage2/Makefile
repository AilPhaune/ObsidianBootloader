SRC_DIR?=src
CARGO?=cargo

ASM?=nasm
ASM_FLAGS?=-f elf32 -F dwarf -g

LD = "ld.lld"

ifeq ($(MODE),debug)
	CARGO_CONFIG=--profile dev
	CARGO_BUILD_DIR=target/x86-unknown-bare_metal/debug/deps
else
	CARGO_CONFIG=--release
	CARGO_BUILD_DIR=target/x86-unknown-bare_metal/release/deps
endif

.PHONY: all stage2asm stage2 clean

all: stage2asm stage2

stage2: stage2asm
	$(CARGO) rustc $(CARGO_CONFIG) -- -C link-args=-Tlinker.ld --emit obj
	$(LD) -T linker.ld ../../build/main.o $(CARGO_BUILD_DIR)/stage2-*.o -o ../../build/stage2.o

	objcopy -O binary ../../build/stage2.o ../../build/bootloader_stage2.bin
	objcopy --only-keep-debug ../../build/stage2.o ../../build/bootloader_stage2.debug

stage2asm: ../../build/main.o

../../build/main.o: $(shell find . -type f -name '*.asm')
	mkdir -p build
	$(ASM) $(ASM_FLAGS) -o ../../build/main.o main.asm

clean:
	rm -rf target
