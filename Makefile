all: hello.bin echo.bin

hello.bin: hello.c
	riscv64-unknown-elf-gcc -S hello.c
	riscv64-unknown-elf-gcc -Wl,-Ttext=0x0 -nostdlib -o hello hello.s
	riscv64-unknown-elf-objcopy -O binary hello hello.bin

echo.bin: echo.c
	riscv64-unknown-elf-gcc -S echo.c
	riscv64-unknown-elf-gcc -Wl,-Ttext=0x0 -nostdlib -o echo echo.s
	riscv64-unknown-elf-objcopy -O binary echo echo.bin

clean:
	rm -f hello.s
	rm -f hello
	rm -f hello.bin
	rm -f echo.s
	rm -f echo
	rm -f echo.bin

