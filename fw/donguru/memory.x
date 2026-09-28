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

  /* BKP_REGS (reg): TAMP backup registers, retained across resets (not power loss without VBAT) */
  BKP_REGS : ORIGIN = 0x4000B100, LENGTH = 0x14
}

SECTIONS
{
  .bkp_regs (NOLOAD) : ALIGN(4)
  {
    *(.bss._CNT_BKP_BUFFER .bss._CNT_BKP_BUFFER.*);
    . = ALIGN(4);
  } > BKP_REGS
}

/* Helper symbols: additional RAM banks (see init_ram.rs), flash partitions */
/* CONFIG: offsets relative to the start of flash (ORIGIN(FLASH)), as embassy-boot / flash drivers expect */
__config_start = ORIGIN(CONFIG) - ORIGIN(FLASH);
__config_end = ORIGIN(CONFIG) + LENGTH(CONFIG) - ORIGIN(FLASH);
