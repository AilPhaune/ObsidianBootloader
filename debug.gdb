target remote :1234
set architecture i8086
break *0x7c00
symbol-file build/bootloader_stage2.debug
layout asm
set disassembly-flavor intel
continue
