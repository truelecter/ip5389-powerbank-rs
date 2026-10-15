MEMORY
{
  FLASH : ORIGIN = 0x08000000, LENGTH = 1024K
  /* The factory 96 KB. 224 KB needs unlock_ram first, and draws more power. */
  RAM   : ORIGIN = 0x20000000, LENGTH = 96K
}