MEMORY
{
  /* The last 2 KB page of the first 512 KB bank holds the settings, see src/store.rs */
  FLASH : ORIGIN = 0x08000000, LENGTH = 510K
  /* The factory 96 KB. 224 KB needs unlock_ram first, and draws more power. */
  RAM   : ORIGIN = 0x20000000, LENGTH = 96K
}
