#*****************************************************************************
#* ACHTUNG:         Ein falsch angegebenes Laufwerk kann dazu fuehren, dass  *
#*                  Daten auf dem spezifizierten Laufwerk verloren gehen!    *
#*                  Nicht mit root-Rechten ausfuehren!                       *
#*                                                                           *
#* Autor:           Michael Schoettner, HHU, 12.03.2023                      *
#*****************************************************************************/
TARGET := hhu_tosr
RUST_OBJ := target/$(TARGET)/debug/lib$(TARGET).a

ASM = nasm
ASMOBJFORMAT = elf64
ASMFLAGS := -w+error=label-redef-late
OBJDIR = build

SYSTEM := build/$(TARGET).bin
LINKER_SCRIPT := src/link.ld

GRUB-ISO := hhuTOSr-grub.iso
LIMINE-ISO := hhuTOSr-limine.iso
TOWBOOT-IMG := hhuTOSr-towboot.img

.PHONY: default
default: limine

# -------------------------------------------------------------------------
# Namen der Unterverzeichnisse mit den Assembler-Quelltexten
VPATH = $(sort $(dir $(ASM_SOURCES)))

# --------------------------------------------------------------------------
# Liste der Assembler-Quelltexte/-Objektdateien
ASM_SOURCES = $(shell find ./src -name "*.asm")
ASM_OBJECTS = $(patsubst %.asm,_%.o, $(notdir $(ASM_SOURCES)))
OBJPRE = $(addprefix $(OBJDIR)/,$(ASM_OBJECTS))

VERBOSE = @

.PHONY: all clean run iso

all: $(GRUB-ISO)

clean:
	@rm -r build
	@cargo clean
	
# --------------------------------------------------------------------------
# Regeln zur Erzeugung der Assembler-Objektdateien
$(OBJDIR)/_%.o : %.asm
	@echo "ASM		$@"
	@if test \( ! \( -d $(@D) \) \) ;then mkdir -p $(@D);fi
	$(VERBOSE) $(ASM) -f $(ASMOBJFORMAT) $(ASMFLAGS) -o $@ $<

# --------------------------------------------------------------------------
# Regeln zum Compilieren der Rust-Dateien 
rust_objs:
	@RUST_TARGET_PATH=$(shell pwd) cargo build -Z build-std=std,panic_abort --target $(TARGET)

# --------------------------------------------------------------------------
# System binden
$(SYSTEM): rust_objs $(OBJPRE) $(LINKER_SCRIPT)
	@ld -n -T $(LINKER_SCRIPT) -o $(SYSTEM) $(OBJPRE) $(RUST_OBJ)

# --------------------------------------------------------------------------
# GRUB ISO erstellen
$(GRUB-ISO): $(SYSTEM)
	@cp $(SYSTEM) loader/grub/boot/hhuTOSr.bin
	@grub-mkrescue -o $(GRUB-ISO) loader/grub

grub: $(GRUB-ISO)

# --------------------------------------------------------------------------
# LIMINE ISO erstellen
$(LIMINE-ISO): $(SYSTEM)
	@cp $(SYSTEM) loader/limine/iso/hhuTOSr.bin
	cd loader/limine && ./build.sh && cd ../..
	@mv loader/limine/hhuTOSr-limine.iso .

limine: $(LIMINE-ISO)

# --------------------------------------------------------------------------
# TOWBOOT IMG erstellen
$(TOWBOOT-IMG): $(SYSTEM)
	@cp $(SYSTEM) loader/towboot/hhuTOSr.bin
	cd loader/towboot && ./build.sh && cd ../..
	@mv loader/towboot/hhuTOSr-towboot.img .

towboot: $(TOWBOOT-IMG)