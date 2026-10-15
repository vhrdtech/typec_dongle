MEMORY
{
  /* FLASH and RAM are the regions cortex-m-rt / link.x expects */

  /* FLASH (flash): Full FLASH range, for reference only (dual bank, BANK_2 starts at 0x8040000) */
  /* FLASH : ORIGIN = 0x08000000, LENGTH = 512K */

  /* FLASH (flash): Application */
  FLASH : ORIGIN = 0x08000000, LENGTH = 510K

  /* CONFIG (flash): Persistent application configuration (one erase sector) */
  CONFIG : ORIGIN = 0x0807F800, LENGTH = 2K

  /* SRAM (ram): Main RAM (also exported as RAM) */
  /* SRAM : ORIGIN = 0x20000000, LENGTH = 144K */
  RAM : ORIGIN = 0x20000000, LENGTH = 144K

  /* BKPSRAM (reg): TAMP backup registers, retained across resets (not power loss without VBAT); cnt.x places the BKP counters here */
  BKPSRAM : ORIGIN = 0x4000B100, LENGTH = 0x14
}

/* Helper symbols: additional RAM banks (see init_ram.rs), flash partitions */
/* CONFIG: offsets relative to the start of flash (ORIGIN(FLASH)), as embassy-boot / flash drivers expect */
__config_start = ORIGIN(CONFIG) - ORIGIN(FLASH);
__config_end = ORIGIN(CONFIG) + LENGTH(CONFIG) - ORIGIN(FLASH);

/* GNU build id (--build-id=sha1 in build.rs): the firmware's identity, a host finds the ELF by it. Kept in flash;
   ww_firmware_info::build_id() reads it through the two symbols. */
SECTIONS
{
  .note.gnu.build-id : ALIGN(4)
  {
    __build_id_start = .;
    KEEP(*(.note.gnu.build-id))
    __build_id_end = .;
  } > FLASH
} INSERT AFTER .rodata;
